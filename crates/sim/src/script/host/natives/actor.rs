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
use bevy_ecs::prelude::World;

const SPAWN_HEALTH: i32 = 100;

#[derive(Clone, Debug)]
pub(crate) struct Actor {
    team: Arc<str>,
    weapon: Option<u32>,
    worn: Vec<(Arc<str>, Arc<str>)>,
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
        Ok(Value::Undefined)
    });
    registry.register(Method, "shoot", shoot);
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

/// Walks every actor with a goal one frame along its route.
pub(crate) fn advance(world: &mut World) {
    actor_path::advance_all(world, alive);
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
    let from = muzzle(world, id);
    let dir = match optional(args, 0, vector)? {
        Some(point) => glam::Vec3::from_array(std::array::from_fn(|i| point[i] - from[i]))
            .try_normalize()
            .ok_or("shoot point is the muzzle")?
            .to_array(),
        None => {
            let angles = match world.resource_mut::<Runtime>().object_field(id, "angles") {
                Value::Vector(angles) => angles,
                _ => [0.0; 3],
            };
            math_iw4::angle_vectors(angles).0
        }
    };
    super::super::turrets::fire_bullet(world, id, from, dir, weapon, None);
    Ok(Value::Undefined)
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
