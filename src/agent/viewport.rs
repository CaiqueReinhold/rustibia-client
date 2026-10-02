use bevy::prelude::*;

use crate::agent::Agent;
use crate::map::{Position, ViewportCenter, viewport::is_kept};
use crate::player::components::Player;

/// `Inherited` rather than `Visible`, so a floor hidden by occlusion still hides its agents.
pub fn hide_agents_outside_viewport(
    center: Res<ViewportCenter>,
    player: Query<&Position, With<Player>>,
    mut agents: Query<(&Position, &mut Visibility), With<Agent>>,
) {
    let Some(center) = &center.0 else {
        return;
    };
    let player = player.single().ok();
    for (position, mut visibility) in &mut agents {
        let wanted = if is_kept(center, player, position) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        visibility.set_if_neq(wanted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn a_world_centred_on(center: Option<Position>) -> World {
        let mut world = World::new();
        world.insert_resource(ViewportCenter(center));
        world
    }

    fn an_agent_at(world: &mut World, position: Position) -> Entity {
        world
            .spawn((Agent::default(), position, Visibility::Inherited))
            .id()
    }

    fn visibility(world: &World, agent: Entity) -> Visibility {
        *world.get::<Visibility>(agent).unwrap()
    }

    #[test]
    fn an_agent_outside_the_window_is_hidden() {
        let mut world = a_world_centred_on(Some(Position::new(100, 100, 7)));
        let agent = an_agent_at(&mut world, Position::new(110, 100, 7));

        world.run_system_once(hide_agents_outside_viewport).unwrap();

        assert_eq!(visibility(&world, agent), Visibility::Hidden);
    }

    #[test]
    fn an_agent_that_walks_back_in_inherits_its_floors_visibility_again() {
        let mut world = a_world_centred_on(Some(Position::new(100, 100, 7)));
        let agent = an_agent_at(&mut world, Position::new(110, 100, 7));
        world.run_system_once(hide_agents_outside_viewport).unwrap();

        world.entity_mut(agent).insert(Position::new(109, 100, 7));
        world.run_system_once(hide_agents_outside_viewport).unwrap();

        assert_eq!(visibility(&world, agent), Visibility::Inherited);
    }

    #[test]
    fn before_the_first_description_nothing_is_hidden() {
        let mut world = a_world_centred_on(None);
        let agent = an_agent_at(&mut world, Position::new(110, 100, 7));

        world.run_system_once(hide_agents_outside_viewport).unwrap();

        assert_eq!(visibility(&world, agent), Visibility::Inherited);
    }

    #[test]
    fn an_agent_on_the_floor_still_on_screen_stays_through_a_floor_change() {
        let mut world = a_world_centred_on(Some(Position::new(100, 100, 8)));
        world.spawn((
            crate::player::components::Player {
                agent_id: crate::agent::AgentId(1),
            },
            Position::new(100, 100, 7),
        ));
        let agent = an_agent_at(&mut world, Position::new(101, 100, 7));

        world.run_system_once(hide_agents_outside_viewport).unwrap();

        assert_eq!(visibility(&world, agent), Visibility::Inherited);
    }
}
