use bevy::prelude::*;

use crate::agent::Agent;
use crate::map::{Map, Position};

/// The server's stack index for the `Position` inserted with it. Consumed by
/// [`sync_tile_agents`].
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackIndex(pub u8);

/// The agent's place in its tile's agent list, which is its draw slot.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StackSlot(pub u32);

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
    mut commands: Commands,
    mut map: ResMut<Map>,
    moved: Query<(Entity, &Agent, &Position, Option<&StackIndex>), Changed<Position>>,
) {
    for (entity, agent, pos, stack) in moved.iter() {
        map.index_agent(agent.agent_id, pos, stack.map(|s| s.0));
        if stack.is_some() {
            commands.entity(entity).try_remove::<StackIndex>();
        }
    }
}

pub fn restack_tile_agents(mut map: ResMut<Map>, mut slots: Query<&mut StackSlot>) {
    for pos in map.take_restacked() {
        for (index, id) in map.agents_on(&pos).iter().enumerate() {
            if let Some(entity) = map.get_agent(*id)
                && let Ok(mut slot) = slots.get_mut(entity)
            {
                slot.set_if_neq(StackSlot(index as u32));
            }
        }
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

    fn run_sync(world: &mut World) {
        world.run_system_once(sync_tile_agents).unwrap();
        world.run_system_once(restack_tile_agents).unwrap();
    }

    fn spawn_indexed(world: &mut World, id: u16, pos: Position, stack: Option<u8>) -> Entity {
        let mut entity = world.spawn((
            Agent {
                agent_id: AgentId(id),
                ..Default::default()
            },
            pos,
        ));
        if let Some(stack) = stack {
            entity.insert(StackIndex(stack));
        }
        let entity = entity.id();
        world.resource_mut::<Map>().add_agent(AgentId(id), entity);
        entity
    }

    #[test]
    fn a_stack_index_places_the_agent_and_is_consumed() {
        let mut world = world_with_map();
        spawn_indexed(&mut world, 1, at(10, 10), None);
        run_sync(&mut world);

        let below = spawn_indexed(&mut world, 2, at(10, 10), Some(0));
        run_sync(&mut world);

        assert_eq!(
            world.resource::<Map>().agents_on(&at(10, 10)),
            &[AgentId(2), AgentId(1)]
        );
        assert!(world.get::<StackIndex>(below).is_none());
    }

    #[test]
    fn each_agent_on_a_tile_gets_its_place_as_its_slot() {
        let mut world = world_with_map();
        let bottom = spawn_indexed(&mut world, 1, at(10, 10), None);
        let top = spawn_indexed(&mut world, 2, at(10, 10), None);
        run_sync(&mut world);

        assert_eq!(*world.get::<StackSlot>(bottom).unwrap(), StackSlot(0));
        assert_eq!(*world.get::<StackSlot>(top).unwrap(), StackSlot(1));
    }

    #[test]
    fn a_departure_shifts_the_agents_above_it_down() {
        let mut world = world_with_map();
        let bottom = spawn_indexed(&mut world, 1, at(10, 10), None);
        let top = spawn_indexed(&mut world, 2, at(10, 10), None);
        run_sync(&mut world);

        world.entity_mut(bottom).insert(at(11, 10));
        run_sync(&mut world);

        assert_eq!(*world.get::<StackSlot>(top).unwrap(), StackSlot(0));
        assert_eq!(*world.get::<StackSlot>(bottom).unwrap(), StackSlot(0));
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
