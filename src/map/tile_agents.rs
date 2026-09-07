use bevy::prelude::*;

use crate::agent::Agent;
use crate::map::{Map, Position};

/// Keeps `MapTile::agents` in step with agents' `Position` components.
///
/// Derived from `Changed<Position>` rather than hooked into the spawn / move /
/// teleport events, because `Position` is written from more places than those
/// three and a missed site would silently corrupt the index. Nothing holds
/// `&mut Position` for bookkeeping, so this `Changed` filter really does gate.
///
/// `Map` is the single source of truth for where each agent is currently
/// indexed (`Map::index_agent` tracks it internally), so this system only
/// forwards the new position — no local "where was it last" component needed,
/// and nothing here can go stale relative to what `Map` actually did.
pub fn sync_tile_agents(
    mut map: ResMut<Map>,
    moved: Query<(&Agent, &Position), Changed<Position>>,
) {
    for (agent, pos) in moved.iter() {
        map.index_agent(agent.agent_id, pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentId;
    use bevy::ecs::system::RunSystemOnce;

    fn at(x: u16, y: u16) -> Position {
        Position { x, y, z: 7 }
    }

    fn world_with_map() -> World {
        let mut world = World::new();
        world.insert_resource(Map::default());
        world
    }

    #[test]
    fn an_agent_is_indexed_onto_its_tile() {
        let mut world = world_with_map();
        world.spawn((
            Agent {
                agent_id: AgentId(5),
                ..Default::default()
            },
            at(10, 10),
        ));

        world.run_system_once(sync_tile_agents).unwrap();

        assert_eq!(
            world.resource::<Map>().agents_on(&at(10, 10)),
            &[AgentId(5)]
        );
    }

    #[test]
    fn moving_an_agent_reindexes_it() {
        let mut world = world_with_map();
        let e = world
            .spawn((
                Agent {
                    agent_id: AgentId(5),
                    ..Default::default()
                },
                at(10, 10),
            ))
            .id();
        world.run_system_once(sync_tile_agents).unwrap();

        world.entity_mut(e).insert(at(11, 10));
        world.run_system_once(sync_tile_agents).unwrap();

        assert!(world.resource::<Map>().agents_on(&at(10, 10)).is_empty());
        assert_eq!(
            world.resource::<Map>().agents_on(&at(11, 10)),
            &[AgentId(5)]
        );
    }

    /// Topmost is the last entry, mirroring `Map::peek_item`. Push order is
    /// arrival order, so the most recently arrived agent is on top.
    #[test]
    fn co_located_agents_keep_arrival_order() {
        let mut world = world_with_map();
        world.spawn((
            Agent {
                agent_id: AgentId(1),
                ..Default::default()
            },
            at(10, 10),
        ));
        world.run_system_once(sync_tile_agents).unwrap();

        world.spawn((
            Agent {
                agent_id: AgentId(2),
                ..Default::default()
            },
            at(10, 10),
        ));
        world.run_system_once(sync_tile_agents).unwrap();

        assert_eq!(
            world.resource::<Map>().agents_on(&at(10, 10)),
            &[AgentId(1), AgentId(2)]
        );
    }

    /// A tile nobody stands on answers empty rather than panicking — the gesture
    /// asks about arbitrary hovered tiles.
    #[test]
    fn an_empty_tile_has_no_agents() {
        let world = world_with_map();
        assert!(world.resource::<Map>().agents_on(&at(1, 1)).is_empty());
    }
}
