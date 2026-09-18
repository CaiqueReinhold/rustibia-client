use bevy::prelude::*;

use crate::conf::map::TILE_SIZE;
use crate::conf::target::{MARK_COLOR, MARK_DURATION, MARK_INSET};
use crate::map::Map;
use crate::network::events::PlayerDamagedBy;
use crate::player::square::spawn_square;

#[derive(Component)]
pub struct AttackerSquare {
    timer: Timer,
}

pub fn mark_size() -> f32 {
    TILE_SIZE - 2.0 * MARK_INSET
}

pub fn on_player_damaged_by(
    event: On<PlayerDamagedBy>,
    mut commands: Commands,
    map: Res<Map>,
    mut existing_q: Query<(&ChildOf, &mut AttackerSquare)>,
) {
    let Some(agent) = map.get_agent(event.agent_id) else {
        return;
    };
    for (child_of, mut square) in existing_q.iter_mut() {
        if child_of.parent() == agent {
            square.timer.reset();
            return;
        }
    }
    spawn_square(
        &mut commands,
        agent,
        AttackerSquare {
            timer: Timer::new(MARK_DURATION, TimerMode::Once),
        },
        MARK_COLOR,
        mark_size(),
    );
}

pub fn expire_attacker_squares(
    mut commands: Commands,
    time: Res<Time>,
    mut square_q: Query<(Entity, &mut AttackerSquare)>,
) {
    for (entity, mut square) in square_q.iter_mut() {
        square.timer.tick(time.delta());
        if square.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentId;
    use bevy::ecs::system::RunSystemOnce;

    fn world_with_agent() -> (World, Entity, AgentId) {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let mut map = Map::default();
        let agent = world.spawn(Transform::from_xyz(0.0, 0.0, 0.0)).id();
        map.add_agent(AgentId(7), agent);
        world.insert_resource(map);
        world.add_observer(on_player_damaged_by);
        (world, agent, AgentId(7))
    }

    fn squares(world: &mut World) -> Vec<Entity> {
        world
            .query_filtered::<Entity, With<AttackerSquare>>()
            .iter(world)
            .collect()
    }

    #[test]
    fn a_hit_marks_its_attacker() {
        let (mut world, agent, id) = world_with_agent();

        world.trigger(PlayerDamagedBy { agent_id: id });
        world.flush();

        let marks = squares(&mut world);
        assert_eq!(marks.len(), 1, "exactly one square");
        assert_eq!(
            world.get::<ChildOf>(marks[0]).unwrap().parent(),
            agent,
            "parented to the attacker"
        );
    }

    /// An attacker outside the viewport has no entity to hang a square on. The
    /// server can still name it: the id is minted on damage, the despawn that
    /// drops it arrives separately.
    #[test]
    fn an_attacker_the_map_does_not_hold_marks_nothing() {
        let (mut world, _agent, _id) = world_with_agent();

        world.trigger(PlayerDamagedBy {
            agent_id: AgentId(99),
        });
        world.flush();

        assert!(squares(&mut world).is_empty());
    }

    #[test]
    fn a_square_outlives_half_its_second_and_not_the_whole_one() {
        let (mut world, _agent, id) = world_with_agent();

        world.trigger(PlayerDamagedBy { agent_id: id });
        world.flush();

        world
            .resource_mut::<Time<()>>()
            .advance_by(MARK_DURATION / 2);
        world.run_system_once(expire_attacker_squares).unwrap();
        world.flush();
        assert_eq!(squares(&mut world).len(), 1, "still inside the second");

        world
            .resource_mut::<Time<()>>()
            .advance_by(MARK_DURATION / 2 + std::time::Duration::from_millis(1));
        world.run_system_once(expire_attacker_squares).unwrap();
        world.flush();
        assert!(squares(&mut world).is_empty(), "the second is up");
    }

    #[test]
    fn two_attackers_expire_independently() {
        let (mut world, _first, first_id) = world_with_agent();
        let second = world.spawn(Transform::from_xyz(32.0, 0.0, 0.0)).id();
        world.resource_mut::<Map>().add_agent(AgentId(9), second);

        world.trigger(PlayerDamagedBy { agent_id: first_id });
        world.flush();
        world
            .resource_mut::<Time<()>>()
            .advance_by(MARK_DURATION / 2 + std::time::Duration::from_millis(1));
        world.run_system_once(expire_attacker_squares).unwrap();
        world.flush();

        world.trigger(PlayerDamagedBy {
            agent_id: AgentId(9),
        });
        world.flush();
        world
            .resource_mut::<Time<()>>()
            .advance_by(MARK_DURATION / 2);
        world.run_system_once(expire_attacker_squares).unwrap();
        world.flush();

        let marks = squares(&mut world);
        assert_eq!(marks.len(), 1, "the first expired, the second did not");
        assert_eq!(world.get::<ChildOf>(marks[0]).unwrap().parent(), second);
    }

    #[test]
    fn a_second_hit_resets_one_square_rather_than_stacking() {
        let (mut world, agent, id) = world_with_agent();

        world.trigger(PlayerDamagedBy { agent_id: id });
        world.flush();
        world
            .resource_mut::<Time<()>>()
            .advance_by(MARK_DURATION / 2);
        world.run_system_once(expire_attacker_squares).unwrap();
        world.flush();

        world.trigger(PlayerDamagedBy { agent_id: id });
        world.flush();

        let marks = squares(&mut world);
        assert_eq!(marks.len(), 1, "still exactly one square");
        assert_eq!(world.get::<ChildOf>(marks[0]).unwrap().parent(), agent);
        assert_eq!(
            world
                .get::<AttackerSquare>(marks[0])
                .unwrap()
                .timer
                .elapsed(),
            std::time::Duration::ZERO,
            "the second hit restarts the second"
        );
    }

    /// Both squares hang at the same local z, which is only safe because their
    /// bars never overlap. This reduces to `MARK_INSET >= SQUARE_THICKNESS`;
    /// below that the two rings collide and the shared z becomes a z-fight.
    #[test]
    fn the_mark_nests_inside_the_attack_square_without_touching_it() {
        use crate::conf::target::SQUARE_THICKNESS;

        let mark_half = mark_size() / 2.0;
        let attack_inner_face = TILE_SIZE / 2.0 - SQUARE_THICKNESS;

        assert!(
            mark_half <= attack_inner_face,
            "the mark reaches {mark_half} and the attack square's inner face is at {attack_inner_face}"
        );
    }

    #[test]
    fn an_agent_can_carry_both_squares_at_once() {
        use crate::conf::target::SQUARE_COLOR;
        use crate::player::target::TargetSquare;

        let (mut world, agent, id) = world_with_agent();
        world
            .run_system_once(move |mut commands: Commands| {
                spawn_square(&mut commands, agent, TargetSquare, SQUARE_COLOR, TILE_SIZE);
            })
            .unwrap();
        world.flush();

        world.trigger(PlayerDamagedBy { agent_id: id });
        world.flush();

        assert_eq!(squares(&mut world).len(), 1, "one mark");
        assert_eq!(
            world
                .query_filtered::<Entity, With<TargetSquare>>()
                .iter(&world)
                .count(),
            1,
            "and the attack square is untouched"
        );
    }
}
