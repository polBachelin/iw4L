//! Actors: characters the server simulates without a client slot. An actor
//! is a script mover with a team, health and a weapon; scripts give it its
//! character (`setcharacter`), its anim tree and its behavior.

use super::super::args::{arg, float, int, optional, string, vector};
use super::super::entities::EntityKind;
use super::super::entity_damage::EntityHit;
use super::actor_path::{self, Nav};
use crate::frame::FrameWorld;
use crate::script::Namespace::{Function, Method};
use crate::script::runtime::raise;
use crate::script::{Arc, NativeRegistry, Runtime, Value};
use bevy_ecs::prelude::{Resource, World};

const SPAWN_HEALTH: i32 = 100;

/// `actor hold on`: every actor stops walking its route and its `shoot`
/// fires nothing, so a test can place enemies and keep them there. Absent
/// means not held.
#[derive(Resource, Clone, Copy, Debug, Default)]
struct ActorHold(bool);

/// One live actor as the `actor list` console verb shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ActorRow {
    pub number: i32,
    pub archetype: String,
    pub team: String,
    pub origin: [f32; 3],
    pub health: i32,
}

/// Which live actors `actor tp` moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ActorTarget {
    All,
    /// The live actor spawned first, so a recipe can bring enemies one by
    /// one without knowing their entity numbers.
    First,
    Number(i32),
}

fn held(world: &World) -> bool {
    world.get_resource::<ActorHold>().is_some_and(|hold| hold.0)
}

#[derive(Clone, Debug)]
pub(crate) struct Actor {
    team: Arc<str>,
    weapon: Option<u32>,
    worn: Vec<(Arc<str>, Arc<str>)>,
    /// The weapon's world model on the body's weapon tag, as attached.
    gun: Option<(Arc<str>, Arc<str>)>,
    pub(super) nav: Option<Nav>,
}

fn actor(world: &World, receiver: &Value) -> Result<u64, String> {
    match receiver {
        Value::Object(id) if world.resource::<Runtime>().actors.contains_key(id) => Ok(*id),
        _ => Err("receiver is not an actor".into()),
    }
}

fn health(runtime: &mut Runtime, object: u64) -> i32 {
    match runtime.object_field(object, "health") {
        Value::Int(health) => health,
        Value::Float(health) => health as i32,
        _ => 0,
    }
}

pub(crate) fn alive(world: &mut World, object: u64) -> bool {
    let mut runtime = world.resource_mut::<Runtime>();
    runtime.live(&object) && health(&mut runtime, object) > 0
}

pub(crate) fn register(registry: &mut NativeRegistry) {
    actor_path::register(registry);
    registry.register(Method, "finishactordamage", |world, receiver, args| {
        let object = actor(world, receiver)?;
        let amount = int(args, 0)?;
        let attacker = arg(args, 1)?.clone();
        let direction = vector(args, 2)?;
        let point = vector(args, 3)?;
        let means = string(args, 4)?;
        let tag = string(args, 5)?;
        let location = string(args, 6)?;
        let flags = int(args, 7)?;
        let weapon = string(args, 8)?;
        finish_damage(
            world, object, amount, attacker, direction, point, &means, &tag, &location, flags,
            &weapon,
        );
        Ok(Value::Undefined)
    });
    registry.register(Function, "spawnactor", |world, _, args| {
        let origin = vector(args, 0)?;
        let angles = vector(args, 1)?;
        let team = string(args, 2)?;
        let presence = super::super::presence::spawn_presence(world, origin)?;
        let number = FrameWorld::from_world(world).gentity_number(presence);
        let mut runtime = world.resource_mut::<Runtime>();
        let id = runtime.create_entity(EntityKind::Actor, "actor")?;
        runtime.set_object_field(id, "origin", Value::Vector(origin));
        runtime.set_object_field(id, "angles", Value::Vector(angles));
        runtime.set_object_field(id, "team", Value::string(&team));
        runtime.set_object_field(id, "health", Value::Int(SPAWN_HEALTH));
        runtime.set_object_field(id, "maxhealth", Value::Int(SPAWN_HEALTH));
        let entity = runtime.entities.get_mut(&id).unwrap();
        entity.presence = Some(presence);
        entity.can_damage = true;
        if let Some(number) = number {
            entity.number = number;
        }
        runtime.actors.insert(
            id,
            Actor {
                team: team.into(),
                weapon: None,
                worn: Vec::new(),
                gun: None,
                nav: None,
            },
        );
        Ok(Value::Object(id))
    });
    registry.register(Method, "setgoalpos", |world, receiver, args| {
        let id = actor(world, receiver)?;
        let goal = vector(args, 0)?;
        set_goal(world, id, goal, None);
        Ok(Value::Undefined)
    });
    registry.register(Method, "setgoalnode", |world, receiver, args| {
        let id = actor(world, receiver)?;
        let node = actor_path::node_arg(world, args, 0)?;
        let goal = FrameWorld::from_world(world).path_graph().nodes[node as usize].origin;
        set_goal(world, id, goal, Some(node));
        Ok(Value::Undefined)
    });
    registry.register(Method, "setgoalradius", |world, receiver, args| {
        let id = actor(world, receiver)?;
        let radius = float(args, 0)?;
        world
            .resource_mut::<Runtime>()
            .set_object_field(id, "goalradius", Value::Float(radius));
        Ok(Value::Undefined)
    });
    registry.register(Method, "setweapon", |world, receiver, args| {
        let id = actor(world, receiver)?;
        let name = string(args, 0)?;
        let weapon = crate::script_player::weapon_named(&FrameWorld::from_world(world), &name)?;
        world
            .resource_mut::<Runtime>()
            .actors
            .get_mut(&id)
            .unwrap()
            .weapon = (weapon != 0).then_some(weapon);
        hold_gun(world, id);
        if let Some((model, tag)) = world.resource::<Runtime>().actors[&id].gun.clone() {
            let number = world.resource::<Runtime>().entities[&id].number;
            diag::info!(
                Sim,
                "actor gun: actor={number} weapon={name} gun={model}@{tag}"
            );
        }
        Ok(Value::Undefined)
    });
    registry.register(Method, "shoot", shoot);
    registry.register(Method, "dropweapon", drop_weapon);
    registry.register(Method, "isalive", |world, receiver, _| {
        let id = actor(world, receiver)?;
        Ok(Value::Int(alive(world, id).into()))
    });
    registry.register(Method, "getteam", |world, receiver, _| {
        let id = actor(world, receiver)?;
        Ok(Value::string(&world.resource::<Runtime>().actors[&id].team))
    });
}

fn set_goal(world: &mut World, id: u64, goal: [f32; 3], node: Option<u32>) {
    let nav = actor_path::set_goal(world, id, goal, node);
    world
        .resource_mut::<Runtime>()
        .actors
        .get_mut(&id)
        .unwrap()
        .nav = Some(nav);
}

/// Walks every actor with a goal one frame along its route, unless actors
/// are held, then turns each toward where it goes.
pub(crate) fn advance(world: &mut World) {
    let held = held(world);
    if !held {
        actor_path::advance_all(world, alive);
    }
    turn(world, held);
}

fn number_field(runtime: &mut Runtime, id: u64, name: &str) -> Option<f32> {
    match runtime.object_field(id, name) {
        Value::Int(v) => Some(v as f32),
        Value::Float(v) => Some(v),
        _ => None,
    }
}

/// Turns every live actor toward the way it walked this frame, else toward
/// the yaw its script faces it at (`goalyaw`), by at most `turnrate` degrees
/// a second. An actor whose script sets no `turnrate` turns at once.
fn turn(world: &mut World, held: bool) {
    for (id, _) in living(world) {
        let mut runtime = world.resource_mut::<Runtime>();
        let heading = runtime.actors[&id]
            .nav
            .as_ref()
            .and_then(|nav| nav.heading)
            .filter(|_| !held);
        let Some(goal) = heading.or_else(|| number_field(&mut runtime, id, "goalyaw")) else {
            continue;
        };
        let mut angles = match runtime.object_field(id, "angles") {
            Value::Vector(angles) => angles,
            _ => [0.0; 3],
        };
        let most = number_field(&mut runtime, id, "turnrate")
            .map_or(f32::INFINITY, |rate| rate.max(0.0) * actor_path::TICK_S);
        let yaw = actor_path::turn_toward(angles[1], goal, most);
        if yaw != angles[1] {
            angles[1] = yaw;
            runtime.set_object_field(id, "angles", Value::Vector(angles));
        }
    }
}

/// The live actors' entity numbers, in spawn order.
fn living(world: &mut World) -> Vec<(u64, i32)> {
    let ids: Vec<u64> = world.resource::<Runtime>().actors.keys().copied().collect();
    let living: Vec<u64> = ids.into_iter().filter(|id| alive(world, *id)).collect();
    let runtime = world.resource::<Runtime>();
    living
        .into_iter()
        .map(|id| (id, runtime.entities[&id].number))
        .collect()
}

/// The console's view of actors: listing, placing and holding them. They
/// stand outside the client action path, so the console reaches them here.
impl crate::SimWorld {
    pub fn actor_rows(&mut self) -> Vec<ActorRow> {
        let mut frame = self.frame();
        let world = frame.ecs();
        living(world)
            .into_iter()
            .map(|(id, number)| {
                let team = world.resource::<Runtime>().actors[&id].team.to_string();
                let mut runtime = world.resource_mut::<Runtime>();
                let archetype = match runtime.object_field(id, "archetype") {
                    Value::String(name) => name.to_string(),
                    _ => String::new(),
                };
                let origin = match runtime.object_field(id, "origin") {
                    Value::Vector(origin) => origin,
                    _ => [0.0; 3],
                };
                ActorRow {
                    number,
                    archetype,
                    team,
                    origin,
                    health: health(&mut runtime, id),
                }
            })
            .collect()
    }

    /// Puts the `target` live actors at `origin`, facing `yaw` when given,
    /// and drops their route; the script plans a new one on its next move.
    /// Returns the entity numbers moved.
    pub fn actor_tp(
        &mut self,
        target: ActorTarget,
        origin: [f32; 3],
        yaw: Option<f32>,
    ) -> Vec<i32> {
        let mut frame = self.frame();
        let world = frame.ecs();
        let mut moved = living(world);
        match target {
            ActorTarget::All => {}
            ActorTarget::First => moved.truncate(1),
            ActorTarget::Number(want) => moved.retain(|(_, n)| *n == want),
        }
        let mut runtime = world.resource_mut::<Runtime>();
        for (id, _) in &moved {
            runtime.set_object_field(*id, "origin", Value::Vector(origin));
            if let Some(yaw) = yaw {
                runtime.set_object_field(*id, "angles", Value::Vector([0.0, yaw, 0.0]));
            }
            if let Some(actor) = runtime.actors.get_mut(id) {
                actor.nav = None;
            }
        }
        moved.into_iter().map(|(_, n)| n).collect()
    }

    pub fn set_actor_hold(&mut self, on: bool) {
        self.frame().ecs().insert_resource(ActorHold(on));
    }
}

/// An actor wears its character as its model: the body, with the head and
/// hat on the body's head tag, so presence poses and draws them as one DObj.
/// Models attached by script stay on.
pub(crate) fn dress(world: &mut World, id: u64, selection: &crate::CharacterSelection) {
    if !world.resource::<Runtime>().actors.contains_key(&id) {
        return;
    }
    let head_tag = FrameWorld::from_world(world)
        .model_capability(&selection.body)
        .flatten()
        .and_then(|capability| xmodel_runtime::tp_head_attach_tag(&capability.pose.bone_names));
    let worn: Vec<(Arc<str>, Arc<str>)> = head_tag
        .into_iter()
        .flat_map(|tag| {
            [&selection.head, &selection.hat]
                .into_iter()
                .flatten()
                .map(move |model| (Arc::from(model.as_str()), Arc::from(tag)))
        })
        .collect();
    let mut runtime = world.resource_mut::<Runtime>();
    runtime.set_object_field(id, "model", Value::string(&selection.body));
    let runtime = runtime.into_inner();
    let actor = runtime.actors.get_mut(&id).unwrap();
    let entity = runtime.entities.get_mut(&id).unwrap();
    for model in actor.worn.drain(..) {
        if let Some(at) = entity.attachments.iter().position(|have| *have == model) {
            entity.attachments.remove(at);
        }
    }
    entity.attachments.extend(worn.iter().cloned());
    actor.worn = worn;
    // A new body may name its weapon tag differently.
    hold_gun(world, id);
}

/// Puts the actor's weapon's world model on its body's weapon tag (the first
/// of `TP_WEAPON_ATTACH_TAGS` the body has), replacing the gun it held. No
/// weapon, no body or no weapon tag leaves its hands empty.
fn hold_gun(world: &mut World, id: u64) {
    let body = match world.resource_mut::<Runtime>().object_field(id, "model") {
        Value::String(body) => body.to_string(),
        _ => String::new(),
    };
    let weapon = world.resource::<Runtime>().actors[&id].weapon;
    let frame = FrameWorld::from_world(world);
    let tag = frame
        .model_capability(&body)
        .flatten()
        .and_then(|capability| xmodel_runtime::tp_weapon_attach_tag(&capability.pose.bone_names));
    let model = weapon
        .and_then(|weapon| frame.weapon_world_model(weapon))
        .map(|(model, _)| model.to_owned())
        .filter(|model| !model.is_empty());
    if let Some(model) = &model
        && frame.model_capability(model).flatten().is_none()
    {
        static WARNED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
        let mut warned = WARNED.lock().unwrap_or_else(|e| e.into_inner());
        if !warned.contains(model) {
            warned.push(model.clone());
            diag::warn!(
                Sim,
                "actor gun gap: weapon model `{model}` is not in the loaded models; actors hold it undrawn"
            );
        }
    }
    let gun = model
        .zip(tag)
        .map(|(model, tag)| (Arc::<str>::from(model), Arc::<str>::from(tag)));
    let runtime = world.resource_mut::<Runtime>().into_inner();
    let actor = runtime.actors.get_mut(&id).unwrap();
    let entity = runtime.entities.get_mut(&id).unwrap();
    if let Some(old) = actor.gun.take()
        && let Some(at) = entity.attachments.iter().position(|have| *have == old)
    {
        entity.attachments.remove(at);
    }
    entity.attachments.extend(gun.iter().cloned());
    actor.gun = gun;
}

/// `self shoot( [point] )`: one bullet of the actor's weapon from its muzzle,
/// at `point` or along its facing.
fn shoot(world: &mut World, receiver: &Value, args: &[Value]) -> Result<Value, String> {
    let id = actor(world, receiver)?;
    let weapon = world.resource::<Runtime>().actors[&id]
        .weapon
        .ok_or("actor has no weapon")?;
    if !alive(world, id) {
        return Err("actor is dead".into());
    }
    // A held actor fires nothing; an error here would fault the script
    // thread mid-test.
    if held(world) {
        return Ok(Value::Undefined);
    }
    let from = muzzle(world, id);
    let (angles, tolerance) = {
        let mut runtime = world.resource_mut::<Runtime>();
        let angles = match runtime.object_field(id, "angles") {
            Value::Vector(angles) => angles,
            _ => [0.0; 3],
        };
        (angles, number_field(&mut runtime, id, "aimtolerance"))
    };
    let dir = match optional(args, 0, vector)? {
        Some(point) => glam::Vec3::from_array(std::array::from_fn(|i| point[i] - from[i]))
            .try_normalize()
            .ok_or("shoot point is the muzzle")?
            .to_array(),
        None => math_iw4::angle_vectors(angles).0,
    };
    // Still turning toward the point: the shot waits until the body faces
    // it within `aimtolerance` degrees.
    if let Some(tolerance) = tolerance
        && math_iw4::angle_subtract(math_iw4::vect_to_angles(dir)[1], angles[1]).abs() > tolerance
    {
        return Ok(Value::Undefined);
    }
    // The event a client's shot pushes: muzzle flash, fire sound and the
    // compass's fire ping for the actor's entity.
    let number = world.resource::<Runtime>().entities[&id].number;
    let tick = world.resource::<crate::step::StepRequest>().tick;
    let mut frame = FrameWorld::from_world(world);
    let correlation = frame.alloc_shot_id().0;
    frame.push_entity_event(
        tick,
        crate::EventAudience::All,
        entity_iw4::predicted_weapon_fire_event(0, false),
        crate::EntityEventPayload {
            number,
            weapon,
            correlation,
            origin: from,
            direction: math_iw4::vect_to_angles(dir),
            ..Default::default()
        },
    );
    super::super::turrets::fire_bullet(world, id, from, dir, weapon, None);
    Ok(Value::Undefined)
}

/// `self dropweapon()`: the actor's weapon, out of its hands, as an item a
/// player picks up, holding a fresh weapon's clip and stock. It falls from
/// the gun's tag, else the actor's origin. Returns the item, or undefined
/// when the actor holds no weapon or no entity is free.
fn drop_weapon(world: &mut World, receiver: &Value, _: &[Value]) -> Result<Value, String> {
    let id = actor(world, receiver)?;
    let Some(weapon) = world.resource::<Runtime>().actors[&id].weapon else {
        return Ok(Value::Undefined);
    };
    let gun_tag = world.resource::<Runtime>().actors[&id]
        .gun
        .as_ref()
        .map(|(_, tag)| Arc::clone(tag));
    let at_tag = gun_tag.and_then(|tag| super::super::presence::tag_world(world, id, &tag));
    let (origin, yaw) = {
        let mut runtime = world.resource_mut::<Runtime>();
        let origin = match runtime.object_field(id, "origin") {
            Value::Vector(origin) => origin,
            _ => [0.0; 3],
        };
        let yaw = match runtime.object_field(id, "angles") {
            Value::Vector(angles) => angles[1],
            _ => 0.0,
        };
        (at_tag.map_or(origin, |(at, _)| at), yaw)
    };
    // The weapon leaves the hands with the drop.
    {
        let runtime = world.resource_mut::<Runtime>().into_inner();
        let actor = runtime.actors.get_mut(&id).unwrap();
        actor.weapon = None;
        if let (Some(gun), Some(entity)) = (actor.gun.take(), runtime.entities.get_mut(&id))
            && let Some(at) = entity.attachments.iter().position(|have| *have == gun)
        {
            entity.attachments.remove(at);
        }
    }
    let tick = world.resource::<crate::step::StepRequest>().tick;
    let mut frame = FrameWorld::from_world(world);
    let (clip_r, clip_l, stock) = frame
        .combat_facts_for(weapon)
        .map_or((0, 0, 0), |facts| weapon_iw4::spawn_clip_stock(&facts, 0));
    let number = crate::item::drop_weapon_at(
        &mut frame,
        tick,
        origin,
        yaw,
        playerstate_iw4::ENTITYNUM_NONE,
        weapon,
        (clip_r, clip_l, stock),
    );
    if number == playerstate_iw4::ENTITYNUM_NONE {
        return Ok(Value::Undefined);
    }
    let actor_number = world.resource::<Runtime>().entities[&id].number;
    diag::info!(
        Sim,
        "actor drop: actor={actor_number} weapon={} clip={clip_r} stock={stock} item={number} at ({:.0} {:.0} {:.0})",
        crate::script_player::weapon_name(&FrameWorld::from_world(world), weapon),
        origin[0],
        origin[1],
        origin[2]
    );
    super::player::new_item_entity(world, number, "weapon_item")
}

fn muzzle(world: &mut World, id: u64) -> [f32; 3] {
    if let Some((origin, _)) = super::super::presence::tag_world(world, id, "tag_flash") {
        return origin;
    }
    let origin = match world.resource_mut::<Runtime>().object_field(id, "origin") {
        Value::Vector(origin) => origin,
        _ => [0.0; 3],
    };
    let eye = movement_iw4::view_height::STAND as f32;
    [origin[0], origin[1], origin[2] + eye]
}

/// Script damage to an actor, scaled by hit location as a player's is: the
/// bone a bullet hit names the location, a head hit is a head shot.
pub(crate) fn damage(world: &mut World, object: u64, hit: &EntityHit, attacker: Value, tag: &str) {
    if !alive(world, object) {
        return;
    }
    let presence = world.resource::<Runtime>().entities[&object].presence;
    let frame = FrameWorld::from_world(world);
    let hitloc = hit
        .bone
        .zip(presence)
        .and_then(|(bone, presence)| {
            frame
                .entity_collision_capabilities()
                .iter()
                .find(|row| row.owner.script_model() == Some(presence))?
                .dobj
                .as_ref()
                .map(|dobj| dobj.bone_hitloc(bone))
        })
        .unwrap_or(0);
    let mut amount = hit.amount;
    let mut means = hit.means;
    if hit.bone.is_some() && means != "MOD_MELEE" {
        let scale = frame
            .combat_facts_for(hit.weapon)
            .map_or(1.0, |facts| facts.location_scale(hitloc));
        amount = (amount as f32 * scale) as i32;
        if hud_iw4::obituary_is_headshot(hitloc) {
            means = "MOD_HEAD_SHOT";
        }
    }
    let weapon = crate::script_player::weapon_name(&frame, hit.weapon);
    let location = weapon_iw4::HITLOC_NAMES
        .get(usize::from(hitloc))
        .copied()
        .unwrap_or("none");
    let callback = "maps/mp/gametypes/_callbacksetup::codecallback_actordamage";
    if world
        .resource::<Runtime>()
        .program
        .as_ref()
        .is_some_and(|p| p.names.contains_key(callback))
    {
        let now = super::super::players::now_ms(world);
        let _ = crate::script::runtime::run_now(
            world,
            callback,
            Value::Object(object),
            vec![
                Value::Int(amount),
                attacker,
                Value::Vector(hit.dir),
                Value::Vector(hit.point),
                Value::string(means),
                Value::string(tag),
                Value::string(location),
                Value::Int(hit.flags),
                Value::string(&weapon),
            ],
            now,
        );
    } else {
        finish_damage(
            world, object, amount, attacker, hit.dir, hit.point, means, tag, location, hit.flags,
            &weapon,
        );
    }
}

fn finish_damage(
    world: &mut World,
    object: u64,
    amount: i32,
    attacker: Value,
    direction: [f32; 3],
    point: [f32; 3],
    means: &str,
    tag: &str,
    location: &str,
    flags: i32,
    weapon: &str,
) {
    if amount <= 0 || !alive(world, object) {
        return;
    }
    let (model, after) = {
        let mut runtime = world.resource_mut::<Runtime>();
        let after = health(&mut runtime, object).saturating_sub(amount);
        runtime.set_object_field(object, "health", Value::Int(after));
        (runtime.object_field(object, "model"), after)
    };
    let receiver = Value::Object(object);
    raise(
        world,
        receiver.clone(),
        "damage",
        vec![
            Value::Int(amount),
            attacker.clone(),
            Value::Vector(direction),
            Value::Vector(point),
            Value::string(means),
            model,
            Value::string(tag),
            Value::string(location),
            Value::Int(flags),
            Value::string(weapon),
        ],
    );
    if after <= 0 {
        if let Some(entity) = world.resource_mut::<Runtime>().entities.get_mut(&object) {
            entity.can_damage = false;
        }
        raise(
            world,
            receiver,
            "death",
            vec![attacker, Value::string(means), Value::string(weapon)],
        );
    }
}
