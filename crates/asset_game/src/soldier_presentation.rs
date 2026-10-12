use std::collections::HashMap;
use std::sync::Arc;

use asset_anim::{
    CompiledAnimTreeDefinition, ParsedPlayerAnimScript, PlayerAnimSources, XAnimCatalog,
};
use asset_core::{FamilyId, FpvMeshIndex};
use asset_model::{BodyMeshCatalog, BodyMeshEntry, FpvHands, FpvMeshCatalog, SoldierKit};
use bevy::prelude::Resource;
use xmodel_runtime::{AnimClip, DObj};

#[derive(Debug)]
pub struct SoldierBodyAnimation {
    bodies: Arc<BodyMeshCatalog>,
    body_name: String,
    tree: Arc<CompiledAnimTreeDefinition>,
    script: Arc<ParsedPlayerAnimScript>,
    catalog: Arc<XAnimCatalog>,
    rig: DObj,
    runtime: Result<Arc<xmodel_runtime::XAnimTreeDefinition>, String>,
}

impl SoldierBodyAnimation {
    fn prepare(
        bodies: &Arc<BodyMeshCatalog>,
        kit: &SoldierKit,
        sources: &PlayerAnimSources,
        catalog: &Arc<XAnimCatalog>,
    ) -> Result<Arc<Self>, String> {
        if !sources.decode_errors().is_empty() {
            return Err("character animation source decode failed".into());
        }
        Self::bind(
            bodies,
            kit,
            sources.family(),
            sources.compiled().and_then(|tree| tree.as_ref().ok()),
            sources
                .parsed_script()
                .and_then(|script| script.as_ref().ok()),
            catalog,
        )
    }

    fn bind(
        bodies: &Arc<BodyMeshCatalog>,
        kit: &SoldierKit,
        family: Option<FamilyId>,
        tree: Option<&Arc<CompiledAnimTreeDefinition>>,
        script: Option<&Arc<ParsedPlayerAnimScript>>,
        catalog: &Arc<XAnimCatalog>,
    ) -> Result<Arc<Self>, String> {
        let body = bodies.get(&kit.body).ok_or("character body missing")?;
        if family != Some(body.namespace) {
            return Err(format!(
                "{} character requires its own animation tree and script; supplied family {:?}",
                body.namespace.as_str(),
                family
            ));
        }
        let tree = tree.ok_or("character tree is not compiled")?;
        let script = script.ok_or("character script is not parsed")?;
        let pose = body
            .skel
            .pose
            .as_ref()
            .ok_or("character body pose missing")?;
        let rig = DObj::build(&[(pose, None)]).map_err(|error| error.to_string())?;
        for name in ["legs", "torso"] {
            let index = tree
                .index_of(name)
                .ok_or_else(|| format!("player tree {name} branch missing"))?;
            if tree.node(index).is_none_or(|node| node.child_count == 0) {
                return Err(format!("player tree {name} is not a branch"));
            }
        }
        let runtime = tree
            .to_runtime_definition(|_, name| catalog.clip(body.namespace, name))
            .map_err(|error| error.to_string());
        Ok(Arc::new(Self {
            runtime,
            bodies: Arc::clone(bodies),
            body_name: kit.body.clone(),
            tree: Arc::clone(tree),
            script: Arc::clone(script),
            catalog: Arc::clone(catalog),
            rig,
        }))
    }

    pub fn body(&self) -> &BodyMeshEntry {
        self.bodies
            .get(&self.body_name)
            .expect("bound character body")
    }

    pub fn tree(&self) -> &Arc<CompiledAnimTreeDefinition> {
        &self.tree
    }

    pub fn script(&self) -> &Arc<ParsedPlayerAnimScript> {
        &self.script
    }

    pub fn runtime(&self) -> Result<&Arc<xmodel_runtime::XAnimTreeDefinition>, &str> {
        self.runtime.as_ref().map_err(String::as_str)
    }

    pub fn decode_leaf(&self, index: u16) -> Result<Arc<AnimClip>, String> {
        let leaf = self
            .tree
            .node(index)
            .filter(|node| node.child_count == 0)
            .ok_or_else(|| format!("packed index {index} is not a character animation leaf"))?;
        let clip = self
            .catalog
            .clip(self.body().namespace, &leaf.name)
            .ok_or_else(|| format!("decode failed for character clip `{}`", leaf.name))?;
        if !self.rig.tracks_for(&clip).iter().any(Option::is_some) {
            return Err(format!(
                "character clip `{}` has no tracks for body `{}`",
                leaf.name, self.body_name
            ));
        }
        Ok(clip)
    }
}

#[derive(Clone, Copy, Debug)]
struct HandMounts {
    posed: bool,
    weapon: bool,
    secondary: bool,
}

#[derive(Clone, Debug)]
pub struct SoldierHands {
    choice: FpvHands,
    model: FpvMeshIndex,
    mounts: HandMounts,
}

impl SoldierHands {
    pub fn choice(&self) -> &FpvHands {
        &self.choice
    }

    pub fn model(&self) -> FpvMeshIndex {
        self.model
    }
}

#[derive(Debug)]
pub struct SoldierHead {
    bodies: Arc<BodyMeshCatalog>,
    name: String,
    tag: String,
}

impl SoldierHead {
    /// A head or hat model mounted on the kit body's head tag.
    fn prepare(
        bodies: &Arc<BodyMeshCatalog>,
        kit: &SoldierKit,
        part: Option<&String>,
        family: FamilyId,
    ) -> Result<Option<Self>, String> {
        let Some(name) = part else {
            return Ok(None);
        };
        let body = bodies.get(&kit.body).ok_or("soldier body missing")?;
        let entry = bodies
            .get(name)
            .ok_or_else(|| format!("soldier model `{name}` missing"))?;
        if entry.namespace != family {
            return Err(format!("soldier model `{name}` differs from body family"));
        }
        let body_pose = body
            .skel
            .pose
            .as_ref()
            .ok_or_else(|| format!("soldier body pose missing for `{name}`"))?;
        let head_pose = entry
            .skel
            .pose
            .as_ref()
            .ok_or_else(|| format!("soldier model `{name}` pose missing"))?;
        let tag = xmodel_runtime::tp_head_attach_tag(&body.skel.bone_names)
            .ok_or("soldier head mount missing")?;
        DObj::build(&[
            (body_pose, None),
            (
                head_pose,
                Some(xmodel_runtime::Attach {
                    parent_model: 0,
                    tag: tag.into(),
                }),
            ),
        ])
        .map_err(|error| error.to_string())?;
        Ok(Some(Self {
            bodies: Arc::clone(bodies),
            name: name.clone(),
            tag: tag.into(),
        }))
    }

    pub fn entry(&self) -> &BodyMeshEntry {
        self.bodies.get(&self.name).expect("prepared soldier head")
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }
}

#[derive(Debug)]
pub struct SoldierPresentation {
    kit: SoldierKit,
    family: FamilyId,
    bodies: Arc<BodyMeshCatalog>,
    meshes: Arc<FpvMeshCatalog>,
    mounts: Arc<[HandMounts]>,
    kit_hands: Result<Option<SoldierHands>, String>,
    head: Result<Option<SoldierHead>, String>,
    hat: Result<Option<SoldierHead>, String>,
    animation: Result<Arc<SoldierBodyAnimation>, String>,
}

impl SoldierPresentation {
    fn prepare(
        axis: bool,
        bodies: &Arc<BodyMeshCatalog>,
        meshes: &Arc<FpvMeshCatalog>,
        mode_hands: Option<&str>,
        mounts: &Arc<[HandMounts]>,
        sources: &PlayerAnimSources,
        catalog: &Arc<XAnimCatalog>,
    ) -> Result<Arc<Self>, String> {
        if meshes.identity() == 0 {
            return Err("soldier meshes are not published".into());
        }
        let family = meshes.map_namespace.ok_or("soldier map family missing")?;
        let kit = bodies.kits().kit(axis).ok_or("no map soldier kit")?.clone();
        let body = bodies.get(&kit.body).ok_or("soldier body missing")?;
        if body.namespace != family {
            return Err("soldier body differs from map family".into());
        }
        let choice = if family == FamilyId::T6 {
            kit.arms
                .as_ref()
                .map(|name| FpvHands::FromKit {
                    namespace: family,
                    name: name.clone(),
                })
                .map(Some)
                .ok_or_else(|| "T6 soldier kit hands missing".to_owned())
        } else {
            let choice = FpvHands::resolve(meshes, family, mode_hands, Some(&kit), None, family);
            Ok(choice.key().is_some().then_some(choice))
        };
        let mut presentation = Self {
            head: SoldierHead::prepare(bodies, &kit, kit.head.as_ref(), family),
            hat: SoldierHead::prepare(bodies, &kit, kit.hat.as_ref(), family),
            animation: SoldierBodyAnimation::prepare(bodies, &kit, sources, catalog),
            kit,
            family,
            bodies: Arc::clone(bodies),
            meshes: Arc::clone(meshes),
            mounts: Arc::clone(mounts),
            kit_hands: Ok(None),
        };
        presentation.kit_hands = choice.and_then(|choice| {
            choice
                .map(|choice| presentation.bind_hands(choice))
                .transpose()
        });
        Ok(Arc::new(presentation))
    }

    /// A script-chosen body, head and hat drawn over `team`'s hands; any
    /// model it names that cannot be drawn refuses the whole character.
    fn dress(
        team: &Self,
        kit: &SoldierKit,
        animation: Result<Arc<SoldierBodyAnimation>, String>,
    ) -> Result<Arc<Self>, String> {
        let body = team
            .bodies
            .get(&kit.body)
            .ok_or_else(|| format!("character body `{}` missing", kit.body))?;
        if body.namespace != team.family {
            return Err(format!(
                "character body `{}` differs from map family",
                kit.body
            ));
        }
        let head = SoldierHead::prepare(&team.bodies, kit, kit.head.as_ref(), team.family)?;
        let hat = SoldierHead::prepare(&team.bodies, kit, kit.hat.as_ref(), team.family)?;
        Ok(Arc::new(Self {
            kit: kit.clone(),
            family: team.family,
            bodies: Arc::clone(&team.bodies),
            meshes: Arc::clone(&team.meshes),
            mounts: Arc::clone(&team.mounts),
            kit_hands: team.kit_hands.clone(),
            head: Ok(head),
            hat: Ok(hat),
            animation: Ok(animation?),
        }))
    }

    fn bind_hands(&self, choice: FpvHands) -> Result<SoldierHands, String> {
        let (family, name) = choice.key().ok_or("soldier hands unresolved")?;
        if family != self.family {
            return Err("hands differ from soldier family".into());
        }
        let index = self
            .meshes
            .index_by_name(family, name)
            .ok_or_else(|| format!("soldier hands `{name}` missing"))?;
        let mounts = *self
            .mounts
            .get(index)
            .ok_or("soldier hand mounts missing")?;
        if !mounts.posed || !mounts.weapon {
            return Err(format!(
                "soldier hands `{name}` require a pose and tag_weapon"
            ));
        }
        Ok(SoldierHands {
            choice,
            model: FpvMeshIndex::from_order(index),
            mounts,
        })
    }

    pub fn kit(&self) -> &SoldierKit {
        &self.kit
    }

    pub fn family(&self) -> FamilyId {
        self.family
    }

    pub fn body(&self) -> &BodyMeshEntry {
        self.bodies
            .get(&self.kit.body)
            .expect("prepared soldier body")
    }

    pub fn head(&self) -> Result<Option<&SoldierHead>, &str> {
        self.head
            .as_ref()
            .map(Option::as_ref)
            .map_err(String::as_str)
    }

    pub fn hat(&self) -> Result<Option<&SoldierHead>, &str> {
        self.hat
            .as_ref()
            .map(Option::as_ref)
            .map_err(String::as_str)
    }

    pub fn owns_bodies(&self, bodies: &Arc<BodyMeshCatalog>) -> bool {
        Arc::ptr_eq(&self.bodies, bodies)
    }

    pub fn animation(&self) -> Result<&Arc<SoldierBodyAnimation>, &str> {
        self.animation.as_ref().map_err(String::as_str)
    }

    pub fn connect_weapon(
        self: &Arc<Self>,
        hands: Option<(FamilyId, &str)>,
        secondary: bool,
    ) -> Result<SoldierFpvPresentation, String> {
        let hands = match &self.kit_hands {
            Err(error) => return Err(error.clone()),
            Ok(Some(hands)) => hands.clone(),
            Ok(None) => {
                let (family, name) = hands.ok_or("soldier weapon hands missing")?;
                self.bind_hands(FpvHands::FromWeaponDef {
                    namespace: family,
                    name: name.to_owned(),
                })?
            }
        };
        if secondary && !hands.mounts.secondary {
            return Err("soldier hands require tag_weapon1 for a secondary gun".into());
        }
        Ok(SoldierFpvPresentation {
            soldier: Arc::clone(self),
            hands,
        })
    }
}

#[derive(Clone, Debug)]
pub struct SoldierFpvPresentation {
    soldier: Arc<SoldierPresentation>,
    hands: SoldierHands,
}

impl SoldierFpvPresentation {
    pub fn soldier(&self) -> &Arc<SoldierPresentation> {
        &self.soldier
    }

    pub fn hands(&self) -> &SoldierHands {
        &self.hands
    }

    pub fn mesh_identity(&self) -> u64 {
        self.soldier.meshes.identity()
    }
}

#[derive(Clone, Debug, Resource)]
pub struct SoldierPresentations {
    sides: [Result<Arc<SoldierPresentation>, String>; 2],
    characters: HashMap<SoldierKit, Result<Arc<SoldierPresentation>, String>>,
    sources_decoded: bool,
    bodies: Arc<BodyMeshCatalog>,
    meshes: Arc<FpvMeshCatalog>,
    catalog: Arc<XAnimCatalog>,
    animation_family: Option<FamilyId>,
    tree: Option<Arc<CompiledAnimTreeDefinition>>,
    script: Option<Arc<ParsedPlayerAnimScript>>,
}

impl SoldierPresentations {
    /// `mode_hands` names the viewhands a mode gives its players; when the
    /// meshes hold no such model, the kit's are used.
    pub fn prepare(
        bodies: &Arc<BodyMeshCatalog>,
        meshes: &Arc<FpvMeshCatalog>,
        mode_hands: Option<&str>,
        sources: &PlayerAnimSources,
        catalog: &Arc<XAnimCatalog>,
    ) -> Self {
        let mounts: Arc<[HandMounts]> = (0..meshes.len())
            .map(|index| meshes.get_at(index).expect("published mesh row"))
            .map(|entry| HandMounts {
                posed: entry.skel.pose.is_some(),
                weapon: entry
                    .skel
                    .bone_names
                    .iter()
                    .any(|bone| bone == "tag_weapon"),
                secondary: entry
                    .skel
                    .bone_names
                    .iter()
                    .any(|bone| bone == "tag_weapon1"),
            })
            .collect();
        let allies = SoldierPresentation::prepare(
            false, bodies, meshes, mode_hands, &mounts, sources, catalog,
        );
        let axis = if bodies.kits().kit(false) == bodies.kits().kit(true) {
            allies.clone()
        } else {
            SoldierPresentation::prepare(
                true, bodies, meshes, mode_hands, &mounts, sources, catalog,
            )
        };
        Self {
            sides: [allies, axis],
            characters: HashMap::new(),
            sources_decoded: sources.decode_errors().is_empty(),
            bodies: Arc::clone(bodies),
            meshes: Arc::clone(meshes),
            catalog: Arc::clone(catalog),
            animation_family: sources.family(),
            tree: sources
                .compiled()
                .and_then(|tree| tree.as_ref().ok())
                .cloned(),
            script: sources
                .parsed_script()
                .and_then(|script| script.as_ref().ok())
                .cloned(),
        }
    }

    pub fn owned_by(
        &self,
        bodies: &Arc<BodyMeshCatalog>,
        meshes: &Arc<FpvMeshCatalog>,
        catalog: &Arc<XAnimCatalog>,
        sources: &PlayerAnimSources,
    ) -> bool {
        Arc::ptr_eq(&self.bodies, bodies)
            && Arc::ptr_eq(&self.meshes, meshes)
            && Arc::ptr_eq(&self.catalog, catalog)
            && self.animation_family == sources.family()
            && match (
                self.tree.as_ref(),
                sources.compiled().and_then(|tree| tree.as_ref().ok()),
            ) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
            && match (
                self.script.as_ref(),
                sources
                    .parsed_script()
                    .and_then(|script| script.as_ref().ok()),
            ) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
    }

    pub fn mesh_identity(&self) -> u64 {
        self.meshes.identity()
    }

    pub fn side(&self, axis: bool) -> Result<&Arc<SoldierPresentation>, &str> {
        self.sides[usize::from(axis)]
            .as_ref()
            .map_err(String::as_str)
    }

    /// `character`'s presentation once [`Self::dress`] prepared it, else the
    /// team kit's.
    pub fn soldier(
        &self,
        axis: bool,
        character: Option<&SoldierKit>,
    ) -> Result<&Arc<SoldierPresentation>, &str> {
        match character.and_then(|kit| self.characters.get(kit)) {
            Some(Ok(dressed)) => Ok(dressed),
            _ => self.side(axis),
        }
    }

    /// Prepares `character` for [`Self::soldier`]. A character that cannot be
    /// drawn is reported once and left to the team kit.
    pub fn dress(&mut self, character: &SoldierKit) {
        if self.characters.contains_key(character) {
            return;
        }
        let team = self.sides.iter().find_map(|side| side.as_ref().ok());
        let dressed = match team {
            None => Err("no team kit to dress".to_owned()),
            Some(team) => {
                let animation = if self.sources_decoded {
                    SoldierBodyAnimation::bind(
                        &self.bodies,
                        character,
                        self.animation_family,
                        self.tree.as_ref(),
                        self.script.as_ref(),
                        &self.catalog,
                    )
                } else {
                    Err("character animation source decode failed".into())
                };
                SoldierPresentation::dress(team, character, animation)
            }
        };
        if let Err(error) = &dressed {
            diag::warn!(
                World,
                "character body={} head={:?} hat={:?} drawn in its team kit: {error}",
                character.body,
                character.head,
                character.hat
            );
        }
        self.characters.insert(character.clone(), dressed);
    }
}
