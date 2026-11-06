use bevy::prelude::*;

use crate::agent::Agent;
use crate::map::{Position, ViewportCenter, viewport::is_kept};
use crate::player::components::Player;

/// An agent that left the window. The server sends none of its moves while it is out, so its
/// `Position` is stale until a step or a teleport from the server lands, and both remove this.
#[derive(Component, Debug)]
pub struct OutOfSight;

/// `Inherited` rather than `Visible`, so a floor hidden by occlusion still hides its agents.
pub fn hide_agents_outside_viewport(
    mut commands: Commands,
    center: Res<ViewportCenter>,
    player: Query<&Position, With<Player>>,
    mut agents: Query<(Entity, &Position, &mut Visibility, Has<OutOfSight>), With<Agent>>,
) {
    let Some(center) = &center.0 else {
        return;
    };
    let player = player.single().ok();
    for (entity, position, mut visibility, out_of_sight) in &mut agents {
        let kept = is_kept(center, player, position);
        if !kept && !out_of_sight {
            commands.entity(entity).insert(OutOfSight);
        }
        let wanted = if kept && !out_of_sight {
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
    fn an_agent_the_server_places_back_in_view_inherits_its_floors_visibility_again() {
        let mut world = a_world_centred_on(Some(Position::new(100, 100, 7)));
        let agent = an_agent_at(&mut world, Position::new(110, 100, 7));
        world.run_system_once(hide_agents_outside_viewport).unwrap();

        world
            .entity_mut(agent)
            .insert(Position::new(109, 100, 7))
            .remove::<OutOfSight>();
        world.run_system_once(hide_agents_outside_viewport).unwrap();

        assert_eq!(visibility(&world, agent), Visibility::Inherited);
    }

    /// The agent may have walked anywhere since it was last seen, so the tile it left from
    /// coming back into view says nothing about where it is.
    #[test]
    fn an_agent_whose_last_seen_tile_comes_back_into_view_stays_hidden() {
        let mut world = a_world_centred_on(Some(Position::new(100, 100, 7)));
        let agent = an_agent_at(&mut world, Position::new(110, 100, 7));
        world.run_system_once(hide_agents_outside_viewport).unwrap();

        world.resource_mut::<ViewportCenter>().0 = Some(Position::new(105, 100, 7));
        world.run_system_once(hide_agents_outside_viewport).unwrap();

        assert_eq!(visibility(&world, agent), Visibility::Hidden);
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
