//! Actors in the client's prediction world.
//!
//! The prediction world copies the authority's collision rows once, when it is
//! armed, and afterwards only moves rows it already has. Actors (survival
//! enemies) are spawned later, so without this their bodies were missing and a
//! predicted shot went through them into the wall. Each snapshot marks actor
//! movers (`ScriptMoverGentity::actor`); prediction mirrors those as hit-volume
//! rows built from the snapshot's DObj state, re-poses them when the authority's
//! anim state changes, and drops them when the mover leaves the snapshot.
//! Unmarked movers are left alone, so MP prediction is unchanged.

use super::SimState;
use crate::bullet_collision::{AuthorityDObjState, EntityCollisionCapabilities};
use crate::gentity::{apos_from_entity_state, pos_from_entity_state};
use crate::identities::ScriptModelId;
use crate::match_state::SnapshotMeta;
use entity_iw4::evaluate_trajectory;
use std::collections::BTreeMap;

/// Actor rows prediction owns, with the authority DObj state each was last
/// built from. `None` marks a row inherited from the armed copy (a late join
/// mid-wave) that has not been rebuilt from a snapshot yet.
pub(super) type PredictionActors =
    BTreeMap<ScriptModelId, Option<xmodel_runtime::DObjSemanticState>>;

impl SimState {
    /// Rows the armed copy already holds for actors are prediction's to keep
    /// current and to remove, like the ones it creates.
    pub(super) fn track_inherited_prediction_actors(&mut self) {
        self.prediction_actors = self
            .entity_collision_capabilities
            .iter()
            .filter(|row| row.dobj.as_ref().is_some_and(|dobj| dobj.hit_volumes))
            .filter_map(|row| Some((row.owner.script_model()?, None)))
            .collect();
    }

    pub(super) fn adopt_prediction_actors(&mut self, meta: &SnapshotMeta, time_ms: i32) {
        let wanted: BTreeMap<
            ScriptModelId,
            (
                &crate::ScriptMoverGentity,
                &xmodel_runtime::DObjSemanticState,
            ),
        > = meta
            .script_movers
            .iter()
            .filter(|mover| mover.actor)
            .filter_map(|mover| {
                let state = meta
                    .entity_dobjs
                    .iter()
                    .find(|(owner, _)| owner.script_model() == Some(mover.id))
                    .map(|(_, state)| state)?;
                Some((mover.id, (mover, state)))
            })
            .collect();

        let gone: Vec<ScriptModelId> = self
            .prediction_actors
            .keys()
            .filter(|id| !wanted.contains_key(id))
            .copied()
            .collect();
        for id in gone {
            self.prediction_actors.remove(&id);
            self.remove_collision_owner(id);
            diag::info!(Sim, "prediction: actor row removed mover={}", id.to_wire());
        }

        for (id, (mover, state)) in wanted {
            if self
                .prediction_actors
                .get(&id)
                .is_some_and(|have| have.as_ref() == Some(state))
            {
                continue;
            }
            let pose = (
                evaluate_trajectory(&pos_from_entity_state(&mover.state), time_ms),
                evaluate_trajectory(&apos_from_entity_state(&mover.state), time_ms),
            );
            let Some(dobj) = self.prediction_actor_dobj(state, pose) else {
                self.prediction_actors.remove(&id);
                self.remove_collision_owner(id);
                continue;
            };
            let mut row = EntityCollisionCapabilities::current_tick(
                crate::AuthorityModelOwner::ScriptModel(id),
                Some(dobj),
                Vec::new(),
            );
            row.solid = !mover.nonsolid;
            row.hidden = mover.state.e_flags & entity_iw4::CG_SCRIPT_MOVER_NODRAW != 0;
            row.followed_pose = Some(pose);
            if !self.prediction_actors.contains_key(&id) {
                diag::info!(
                    Sim,
                    "prediction: actor row added mover={} entity={} model={} capability={}",
                    id.to_wire(),
                    mover.state.number,
                    row.dobj
                        .as_ref()
                        .map_or("", |dobj| dobj.current_model.as_str()),
                    row.dobj
                        .as_ref()
                        .is_some_and(|dobj| dobj.capability.is_some()),
                );
            }
            self.insert_collision_owner(row);
            self.prediction_actors.insert(id, Some(state.clone()));
        }
    }

    /// The authority's actor DObj (`present_model`): body model, attachments,
    /// hidden parts and anim tree, colliding by its posed hit volumes.
    fn prediction_actor_dobj(
        &self,
        state: &xmodel_runtime::DObjSemanticState,
        (origin, angles): ([f32; 3], [f32; 3]),
    ) -> Option<AuthorityDObjState> {
        let body = state.composition.models.first()?;
        let capability = self.model_capability(&body.model).flatten();
        let mut dobj = AuthorityDObjState::at_pose(&body.model, capability, origin, angles);
        dobj.hit_volumes = true;
        let attachments: Vec<(&str, &str)> = state
            .composition
            .models
            .iter()
            .skip(1)
            .filter_map(|model| Some((model.model.as_str(), model.attach_tag.as_deref()?)))
            .collect();
        dobj.set_attachments(&attachments);
        dobj.semantic_state.hide_part_bits = state.hide_part_bits;
        dobj.pose_request.hide_part_bits = state.hide_part_bits;
        let clips = self.content.script_anim_clips();
        dobj.set_script_anim_tree(state.tree.clone(), |name| clips.clip(name));
        Some(dobj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor_mover(id: u32, origin: [f32; 3]) -> crate::ScriptMoverGentity {
        let mut mover = crate::ScriptMoverGentity {
            id: ScriptModelId::from_wire(id),
            actor: true,
            ..Default::default()
        };
        mover.state.tr_base = origin;
        mover
    }

    fn dobj_row(
        id: u32,
    ) -> (
        crate::AuthorityModelOwner,
        xmodel_runtime::DObjSemanticState,
    ) {
        (
            crate::AuthorityModelOwner::ScriptModel(ScriptModelId::from_wire(id)),
            xmodel_runtime::DObjSemanticState::bind_pose("body_enemy".to_owned(), 1, 1),
        )
    }

    fn row(world: &SimState, id: u32) -> Option<&EntityCollisionCapabilities> {
        world
            .entity_collision_capabilities()
            .iter()
            .find(|row| row.owner.script_model() == Some(ScriptModelId::from_wire(id)))
    }

    #[test]
    fn actor_rows_follow_the_snapshot() {
        let mut world = SimState::new();
        let mut meta = SnapshotMeta {
            script_movers: vec![
                actor_mover(40, [10.0, 20.0, 30.0]),
                crate::ScriptMoverGentity {
                    id: ScriptModelId::from_wire(41),
                    ..Default::default()
                },
            ],
            entity_dobjs: vec![dobj_row(40), dobj_row(41)],
            ..Default::default()
        };

        world.adopt_prediction_actors(&meta, 0);
        let actor = row(&world, 40).expect("actor row created");
        let dobj = actor.dobj.as_ref().unwrap();
        assert!(dobj.hit_volumes);
        assert_eq!(dobj.current_model, "body_enemy");
        assert!(actor.trace_geom().flesh);
        assert_eq!(actor.followed_pose, Some(([10.0, 20.0, 30.0], [0.0; 3])));
        assert!(row(&world, 41).is_none(), "unmarked movers get no row");

        meta.script_movers[0].nonsolid = true;
        world.adopt_prediction_actors(&meta, 0);
        crate::presence::follow_movers(
            &mut world.entity_collision_capabilities,
            &meta.script_movers,
            0,
        );
        assert!(
            !row(&world, 40).unwrap().solid,
            "a dead actor stops colliding"
        );

        meta.script_movers.remove(0);
        world.adopt_prediction_actors(&meta, 0);
        assert!(
            row(&world, 40).is_none(),
            "actor row removed with its mover"
        );
        assert!(world.prediction_actors.is_empty());
    }

    #[test]
    fn inherited_actor_rows_are_removed_when_gone() {
        let mut world = SimState::new();
        let mut dobj = AuthorityDObjState::at_pose("body_enemy", None, [0.0; 3], [0.0; 3]);
        dobj.hit_volumes = true;
        world.insert_collision_owner(EntityCollisionCapabilities::current_tick(
            crate::AuthorityModelOwner::ScriptModel(ScriptModelId::from_wire(50)),
            Some(dobj),
            Vec::new(),
        ));
        world.insert_collision_owner(EntityCollisionCapabilities::current_tick(
            crate::AuthorityModelOwner::ScriptModel(ScriptModelId::from_wire(51)),
            None,
            Vec::new(),
        ));
        world.track_inherited_prediction_actors();

        world.adopt_prediction_actors(&SnapshotMeta::default(), 0);
        assert!(row(&world, 50).is_none());
        assert!(row(&world, 51).is_some(), "map rows are never removed");
    }
}
