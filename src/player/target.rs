use bevy::prelude::*;

use crate::agent::AgentId;
use crate::conf::map::TILE_SIZE;
use crate::conf::target::{SQUARE_COLOR, SQUARE_THICKNESS};
use crate::map::{Map, TARGET_SQUARE_LOCAL_Z};
use crate::network::events::TargetLost;

/// The agent this player is attacking, as a session-local `AgentId`.
///
/// Applied **optimistically**: the click writes it before the server answers.
/// This is a correctness requirement, not a responsiveness preference — because
/// the click toggles, an ack-gated resource lets a lagging player clear the
/// target by clicking twice.
///
/// `seq` numbers each `SetTarget` the client sends. The server stores it beside
/// the target and echoes it on `TargetLost`, so a loss that crossed a newer click
/// on the wire arrives with an older seq and is ignored.
#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct CombatTarget {
    pub target: Option<AgentId>,
    seq: u32,
}

impl CombatTarget {
    /// What clicking `agent_id` should produce: clicking the current target
    /// clears it, clicking anything else selects it.
    pub fn next_for_click(&self, agent_id: AgentId) -> Option<AgentId> {
        if self.target == Some(agent_id) {
            None
        } else {
            Some(agent_id)
        }
    }

    /// Applies a click optimistically and returns what to tell the server.
    ///
    /// One call, so a caller cannot obtain a value to send without having already
    /// applied it: "optimistic before send" is a property of this signature rather
    /// than of statement order at the call site.
    pub fn apply_click(&mut self, agent_id: AgentId) -> (Option<AgentId>, u32) {
        let next = self.next_for_click(agent_id);
        self.target = next;
        self.seq += 1;
        (next, self.seq)
    }

    /// Drops the target locally and returns the seq to send with the clear.
    pub fn clear_locally(&mut self) -> u32 {
        self.target = None;
        self.seq += 1;
        self.seq
    }
}

/// A loss the player has already moved past is not a loss.
pub fn on_target_lost(
    event: On<TargetLost>,
    mut commands: Commands,
    mut target: ResMut<CombatTarget>,
    map: Res<Map>,
    square_q: Query<Entity, With<TargetSquare>>,
) {
    if event.seq != target.seq {
        return;
    }
    target.target = None;
    refresh_target_square(&mut commands, &target, &map, &square_q);
}

#[derive(Component)]
pub struct TargetSquare;

/// `to_world()` is the tile's top-left corner; the square must sit on the tile
/// centre. Y is negative because world Y grows upward while tile Y grows down.
pub fn square_centre_offset() -> Vec2 {
    Vec2::new(TILE_SIZE / 2.0, -TILE_SIZE / 2.0)
}

/// The four edges of the outline as `(size, centre_offset_from_tile_centre)`.
fn square_bars() -> [(Vec2, Vec2); 4] {
    let half = TILE_SIZE / 2.0;
    let inset = half - SQUARE_THICKNESS / 2.0;
    [
        (
            Vec2::new(TILE_SIZE, SQUARE_THICKNESS),
            Vec2::new(0.0, inset),
        ),
        (
            Vec2::new(TILE_SIZE, SQUARE_THICKNESS),
            Vec2::new(0.0, -inset),
        ),
        (
            Vec2::new(SQUARE_THICKNESS, TILE_SIZE),
            Vec2::new(-inset, 0.0),
        ),
        (
            Vec2::new(SQUARE_THICKNESS, TILE_SIZE),
            Vec2::new(inset, 0.0),
        ),
    ]
}

/// Despawns any existing square and, if there is a target, spawns a new one as a
/// **child of the target agent's entity** — which gives walk-offset tracking for
/// free, with no per-frame sync system.
pub fn refresh_target_square(
    commands: &mut Commands,
    target: &CombatTarget,
    map: &Map,
    square_q: &Query<Entity, With<TargetSquare>>,
) {
    for existing in square_q.iter() {
        commands.entity(existing).despawn();
    }

    let Some(agent_id) = target.target else {
        return;
    };
    let Some(agent_entity) = map.get_agent(agent_id) else {
        return;
    };

    let centre = square_centre_offset();
    commands.entity(agent_entity).with_children(|parent| {
        let mut root = parent.spawn((
            TargetSquare,
            Transform::from_xyz(centre.x, centre.y, TARGET_SQUARE_LOCAL_Z),
            Visibility::default(),
        ));
        root.with_children(|frame| {
            for (size, offset) in square_bars() {
                frame.spawn((
                    Sprite {
                        color: SQUARE_COLOR,
                        custom_size: Some(size),
                        ..Default::default()
                    },
                    Transform::from_xyz(offset.x, offset.y, 0.0),
                ));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn combat_target_starts_empty() {
        assert_eq!(CombatTarget::default().target, None);
    }

    /// The toggle is what makes optimistic application necessary: with ack-gating,
    /// a lagging player clicks twice and the second click clears the target they
    /// were trying to set.
    #[test]
    fn next_for_click_toggles_the_current_target_off() {
        let mut target = CombatTarget::default();
        target.apply_click(7);
        assert_eq!(target.next_for_click(7), None);
    }

    #[test]
    fn next_for_click_switches_to_a_different_agent() {
        let mut target = CombatTarget::default();
        target.apply_click(7);
        assert_eq!(target.next_for_click(9), Some(9));
    }

    #[test]
    fn next_for_click_sets_when_nothing_is_targeted() {
        let target = CombatTarget::default();
        assert_eq!(target.next_for_click(7), Some(7));
    }

    /// The optimistic apply is the point: `apply_click` cannot hand you something
    /// to send without having already applied it, so the ordering cannot be got
    /// wrong at the call site. The counter starts at 1, so 0 can never name a
    /// target a client set.
    #[test]
    fn apply_click_applies_before_returning_what_to_send() {
        let mut target = CombatTarget::default();
        let (to_send, seq) = target.apply_click(7);
        assert_eq!(to_send, Some(7));
        assert_eq!(seq, 1);
        assert_eq!(target.target, Some(7), "applied before the caller can send");

        let (to_send, seq) = target.apply_click(7);
        assert_eq!(to_send, None);
        assert_eq!(seq, 2);
        assert_eq!(target.target, None);
    }

    fn refresh_square_system(
        mut commands: Commands,
        target: Res<CombatTarget>,
        map: Res<Map>,
        square_q: Query<Entity, With<TargetSquare>>,
    ) {
        refresh_target_square(&mut commands, &target, &map, &square_q);
    }

    fn world_with_observer() -> World {
        let mut world = World::new();
        world.init_resource::<CombatTarget>();
        world.insert_resource(Map::default());
        world.add_observer(on_target_lost);
        world
    }

    #[test]
    fn clear_locally_mints_a_seq_too() {
        let mut target = CombatTarget::default();
        target.apply_click(7);

        assert_eq!(target.clear_locally(), 2);
        assert_eq!(target.target, None);
    }

    #[test]
    fn a_loss_for_the_current_seq_clears_the_target() {
        let mut world = world_with_observer();
        let seq = world.resource_mut::<CombatTarget>().apply_click(7).1;

        world.trigger(TargetLost { seq });
        world.flush();

        assert_eq!(world.resource::<CombatTarget>().target, None);
    }

    /// The whole point of the seq: a loss that crossed a newer click on the wire
    /// must not clear the target that click set.
    #[test]
    fn a_loss_for_a_stale_seq_is_ignored() {
        let mut world = world_with_observer();
        let stale = world.resource_mut::<CombatTarget>().apply_click(7).1;
        world.resource_mut::<CombatTarget>().apply_click(9);

        world.trigger(TargetLost { seq: stale });
        world.flush();

        assert_eq!(world.resource::<CombatTarget>().target, Some(9));
    }

    #[test]
    fn the_square_is_a_full_tile_outline() {
        // Four bars, each spanning a full tile edge, at the configured thickness.
        let bars = square_bars();
        assert_eq!(bars.len(), 4);
        for (size, _) in &bars {
            assert!(
                (size.x - TILE_SIZE).abs() < f32::EPSILON
                    || (size.y - TILE_SIZE).abs() < f32::EPSILON,
                "every bar spans one full tile edge, got {size:?}"
            );
            assert!(
                (size.x - SQUARE_THICKNESS).abs() < f32::EPSILON
                    || (size.y - SQUARE_THICKNESS).abs() < f32::EPSILON,
                "every bar is one thickness deep, got {size:?}"
            );
        }
    }

    /// `to_world()` returns the tile's TOP-LEFT corner and the convention is
    /// size-dependent: 64px sprites centre on it, 32px things do not. This is the
    /// counterpart of OTClient's `- getDisplacement()`; both put the square on the
    /// tile rather than on the artwork. Getting this wrong shipped a visible bug
    /// last session.
    #[test]
    fn the_square_is_offset_to_the_tile_centre() {
        assert_eq!(
            square_centre_offset(),
            Vec2::new(TILE_SIZE / 2.0, -TILE_SIZE / 2.0)
        );
    }

    /// The square is a child of the agent, and transform hierarchies compose
    /// additively, so the target layer's own offset as the local z would put the
    /// square in FRONT of the creature.
    ///
    /// This drives the real `refresh_target_square` and reads back the local z it
    /// spawned with. Checking the two constants against each other instead would
    /// be a tautology, true by construction whichever one the call site uses.
    #[test]
    fn the_square_composes_to_just_under_the_agent() {
        use crate::map::{DrawLayer, DrawOrder, DrawOrigin, DrawRank, Position};

        let tile = Position::new(1000, 1000, 7);
        let origin = DrawOrigin::around(&tile);
        let agent_z =
            DrawOrder::new(tile.clone(), DrawRank::Standing, DrawLayer::Creature, 0).key(&origin);

        let mut world = World::new();
        world.init_resource::<CombatTarget>();
        let mut map = Map::default();
        let agent_entity = world.spawn(Transform::from_xyz(0.0, 0.0, agent_z)).id();
        map.add_agent(7, agent_entity);
        world.insert_resource(map);

        world.resource_mut::<CombatTarget>().apply_click(7);
        world.run_system_once(refresh_square_system).unwrap();
        world.flush();

        let square_entity = world
            .query_filtered::<Entity, With<TargetSquare>>()
            .single(&world)
            .expect("a target square was spawned");
        let square_local_z = world.get::<Transform>(square_entity).unwrap().translation.z;
        let composed = agent_z + square_local_z;

        assert_eq!(
            composed,
            DrawOrder::new(tile.clone(), DrawRank::Standing, DrawLayer::Target, 0).key(&origin),
            "composed to the target layer, got local z {square_local_z}"
        );
        assert!(composed < agent_z, "the square draws under the creature");
        assert!(
            composed < DrawOrder::new(tile, DrawRank::Standing, DrawLayer::Top, 0).key(&origin),
            "and under top-layer items"
        );
    }

    /// `refresh_target_square` despawns any existing square before spawning a new
    /// one. Without that despawn, switching targets leaves the old square behind
    /// as an orphaned child of the previous target — this drives the real
    /// refresh across two targets in a row and pins that only the newest square
    /// survives, parented under the newest target.
    #[test]
    fn switching_targets_leaves_exactly_one_square() {
        let mut world = World::new();
        world.init_resource::<CombatTarget>();
        let mut map = Map::default();
        let first_agent = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();
        let second_agent = world.spawn(Transform::from_xyz(32.0, 0.0, 0.0)).id();
        map.add_agent(7, first_agent);
        map.add_agent(9, second_agent);
        world.insert_resource(map);

        world.resource_mut::<CombatTarget>().apply_click(7);
        world.run_system_once(refresh_square_system).unwrap();
        world.flush();
        world.resource_mut::<CombatTarget>().apply_click(9);
        world.run_system_once(refresh_square_system).unwrap();
        world.flush();

        let squares: Vec<Entity> = world
            .query_filtered::<Entity, With<TargetSquare>>()
            .iter(&world)
            .collect();
        assert_eq!(squares.len(), 1, "exactly one square should exist");

        let parent = world.get::<ChildOf>(squares[0]).unwrap().parent();
        assert_eq!(
            parent, second_agent,
            "the surviving square should be parented under the newest target"
        );
    }
}
