use bevy::prelude::*;

use crate::agent::Hovered;
use crate::conf::hover::{OUTLINE_COLOR, OUTLINE_THICKNESS};
use crate::conf::map::TILE_SIZE;
use crate::map::{DrawLayer, DrawOrder, DrawRank, Map, Position};
use crate::player::MouseHoverState;
use crate::player::square::{square_bar_sprites, square_centre_offset};

#[derive(Component)]
pub struct HoverOutline;

pub fn sync_hover_outline(
    mut commands: Commands,
    hover: Res<MouseHoverState>,
    mut outline_q: Query<(&mut Transform, &mut DrawOrder, &mut Visibility), With<HoverOutline>>,
) {
    let Ok((mut transform, mut order, mut visibility)) = outline_q.single_mut() else {
        spawn_outline(&mut commands);
        return;
    };
    let Some(tile) = &hover.tile_position else {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    };
    let centre = tile.to_world().truncate() + square_centre_offset();
    if transform.translation.truncate() != centre {
        transform.translation.x = centre.x;
        transform.translation.y = centre.y;
    }
    order.set_if_neq(DrawOrder::new(
        tile.clone(),
        DrawRank::Standing,
        DrawLayer::Hover,
        0,
    ));
    visibility.set_if_neq(Visibility::Inherited);
}

fn spawn_outline(commands: &mut Commands) {
    commands
        .spawn((
            HoverOutline,
            DrawOrder::new(
                Position::new(0, 0, 7),
                DrawRank::Standing,
                DrawLayer::Hover,
                0,
            ),
            Transform::default(),
            Visibility::Hidden,
        ))
        .with_children(|frame| {
            for bar in square_bar_sprites(OUTLINE_COLOR, TILE_SIZE, OUTLINE_THICKNESS) {
                frame.spawn(bar);
            }
        });
}

pub fn sync_hovered_marker(
    mut commands: Commands,
    hover: Res<MouseHoverState>,
    map: Res<Map>,
    marked: Query<Entity, With<Hovered>>,
) {
    let wanted = hover.agent.and_then(|id| map.get_agent(id));
    for entity in &marked {
        if Some(entity) != wanted {
            commands.entity(entity).remove::<Hovered>();
        }
    }
    if let Some(entity) = wanted
        && !marked.contains(entity)
    {
        commands.entity(entity).try_insert(Hovered);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentId;
    use bevy::ecs::system::RunSystemOnce;

    fn hovering(tile: Option<Position>, agent: Option<AgentId>) -> MouseHoverState {
        MouseHoverState {
            tile_position: tile,
            agent,
            ..Default::default()
        }
    }

    fn outline(world: &mut World) -> (Transform, DrawOrder, Visibility) {
        let mut q =
            world.query_filtered::<(&Transform, &DrawOrder, &Visibility), With<HoverOutline>>();
        let (transform, order, visibility) = q.single(world).unwrap();
        (*transform, order.clone(), *visibility)
    }

    #[test]
    fn the_outline_sits_on_the_hovered_tile_in_the_hover_layer() {
        let mut world = World::new();
        let tile = Position::new(100, 200, 7);
        world.insert_resource(hovering(Some(tile.clone()), None));

        world.run_system_once(sync_hover_outline).unwrap();
        world.run_system_once(sync_hover_outline).unwrap();

        let (transform, order, visibility) = outline(&mut world);
        let centre = tile.to_world().truncate() + square_centre_offset();
        assert_eq!(transform.translation.truncate(), centre);
        assert_eq!(
            order,
            DrawOrder::new(tile, DrawRank::Standing, DrawLayer::Hover, 0)
        );
        assert_eq!(visibility, Visibility::Inherited);
    }

    #[test]
    fn the_outline_hides_when_nothing_is_hovered() {
        let mut world = World::new();
        world.insert_resource(hovering(Some(Position::new(100, 200, 7)), None));
        world.run_system_once(sync_hover_outline).unwrap();
        world.run_system_once(sync_hover_outline).unwrap();

        world.insert_resource(hovering(None, None));
        world.run_system_once(sync_hover_outline).unwrap();

        assert_eq!(outline(&mut world).2, Visibility::Hidden);
    }

    #[test]
    fn only_one_outline_is_ever_spawned() {
        let mut world = World::new();
        world.insert_resource(hovering(None, None));

        for _ in 0..3 {
            world.run_system_once(sync_hover_outline).unwrap();
        }

        let count = world
            .query_filtered::<(), With<HoverOutline>>()
            .iter(&world)
            .count();
        assert_eq!(count, 1);
    }

    fn world_with_agents() -> (World, Entity, Entity) {
        let mut world = World::new();
        let mut map = Map::default();
        let a = world.spawn_empty().id();
        let b = world.spawn_empty().id();
        map.add_agent(AgentId(1), a);
        map.add_agent(AgentId(2), b);
        world.insert_resource(map);
        (world, a, b)
    }

    fn hovered(world: &mut World) -> Vec<Entity> {
        world
            .query_filtered::<Entity, With<Hovered>>()
            .iter(world)
            .collect()
    }

    #[test]
    fn the_marker_moves_with_the_hovered_agent() {
        let (mut world, a, b) = world_with_agents();

        world.insert_resource(hovering(None, Some(AgentId(1))));
        world.run_system_once(sync_hovered_marker).unwrap();
        assert_eq!(hovered(&mut world), vec![a]);

        world.insert_resource(hovering(None, Some(AgentId(2))));
        world.run_system_once(sync_hovered_marker).unwrap();
        assert_eq!(hovered(&mut world), vec![b]);

        world.insert_resource(hovering(None, None));
        world.run_system_once(sync_hovered_marker).unwrap();
        assert!(hovered(&mut world).is_empty());
    }

    #[test]
    fn an_agent_unknown_to_the_map_is_not_marked() {
        let (mut world, _, _) = world_with_agents();

        world.insert_resource(hovering(None, Some(AgentId(9))));
        world.run_system_once(sync_hovered_marker).unwrap();

        assert!(hovered(&mut world).is_empty());
    }
}
