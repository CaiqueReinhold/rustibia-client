use std::ops::RangeInclusive;

use bevy::prelude::*;

use crate::conf::map::{
    BASE_FLOOR, MAX_FLOOR, MIN_FLOOR, UNDERGROUND_REACH, VIEW_BOTTOM, VIEW_LEFT, VIEW_RIGHT,
    VIEW_TOP,
};
use crate::items::ChangedTileQueue;
use crate::map::{Map, Position};
use crate::player::components::Player;

/// Inclusive on both ends. Says nothing about `z`; callers pair it with a floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRect {
    pub min_x: u16,
    pub min_y: u16,
    pub max_x: u16,
    pub max_y: u16,
}

impl TileRect {
    pub fn contains(&self, pos: &Position) -> bool {
        (self.min_x..=self.max_x).contains(&pos.x) && (self.min_y..=self.max_y).contains(&pos.y)
    }
}

/// The tiles `floor` contributes to the viewport around `center`. Must match the server's
/// `map_query::floor_viewport_rect`, which builds every `DescribeMap` and walk strip.
pub fn floor_viewport_rect(center: &Position, floor: u8) -> TileRect {
    let floor_offset = center.z as i32 - floor as i32;
    let cx = center.x as i32 + floor_offset;
    let cy = center.y as i32 + floor_offset;

    TileRect {
        min_x: (cx - VIEW_LEFT as i32).max(0) as u16,
        min_y: (cy - VIEW_TOP as i32).max(0) as u16,
        max_x: (cx + VIEW_RIGHT as i32).max(0) as u16,
        max_y: (cy + VIEW_BOTTOM as i32).max(0) as u16,
    }
}

/// Must match the server's `map_query::iter_visible_floors`.
pub fn visible_floors(z: u8) -> RangeInclusive<u8> {
    if z <= BASE_FLOOR {
        MIN_FLOOR..=BASE_FLOOR
    } else {
        z.saturating_sub(UNDERGROUND_REACH).max(BASE_FLOOR + 1)
            ..=(z + UNDERGROUND_REACH).min(MAX_FLOOR)
    }
}

pub fn in_viewport(center: &Position, pos: &Position) -> bool {
    visible_floors(center.z).contains(&pos.z) && floor_viewport_rect(center, pos.z).contains(pos)
}

/// Whether the client still needs `pos`: inside the server's window, or inside the one around
/// `player`'s committed position, which is what is on screen until a step and any teleport
/// deferred behind it have landed.
pub fn is_kept(center: &Position, player: Option<&Position>, pos: &Position) -> bool {
    in_viewport(center, pos) || player.is_some_and(|player| in_viewport(player, pos))
}

/// The center of the last viewport the server described. Written only by
/// `on_describe_map` and `on_ack_walk`; the player's predicted position never moves it.
#[derive(Resource, Debug, Default, PartialEq)]
pub struct ViewportCenter(pub Option<Position>);

pub fn evict_outside_viewport(
    center: Res<ViewportCenter>,
    player: Query<Ref<Position>, With<Player>>,
    mut map: ResMut<Map>,
    mut queue: ResMut<ChangedTileQueue>,
) {
    let player = player.single().ok();
    if !center.is_changed() && !player.as_ref().is_some_and(|p| p.is_changed()) {
        return;
    }
    let Some(center) = &center.0 else {
        return;
    };
    let player = player.as_deref();
    queue
        .changed_positions
        .extend(map.evict_unless(|pos| is_kept(center, player, pos)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use bevy::ecs::system::RunSystemOnce;

    use crate::items::{ChangedTileQueue, Item, ItemConfig, ItemFlag, ItemId};
    use crate::map::Map;

    fn rect(min_x: u16, min_y: u16, max_x: u16, max_y: u16) -> TileRect {
        TileRect {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    #[test]
    fn the_rect_is_centred_on_the_players_own_floor() {
        let center = Position::new(100, 100, 7);
        assert_eq!(floor_viewport_rect(&center, 7), rect(92, 94, 109, 107));
    }

    #[test]
    fn a_floor_above_shifts_down_right_a_tile_per_floor() {
        let center = Position::new(100, 100, 7);
        assert_eq!(floor_viewport_rect(&center, 5), rect(94, 96, 111, 109));
    }

    #[test]
    fn a_floor_below_shifts_up_left_a_tile_per_floor() {
        let center = Position::new(100, 100, 9);
        assert_eq!(floor_viewport_rect(&center, 10), rect(91, 93, 108, 106));
    }

    #[test]
    fn the_rect_clamps_at_the_map_origin() {
        let center = Position::new(3, 2, 7);
        assert_eq!(floor_viewport_rect(&center, 7), rect(0, 0, 12, 9));
    }

    #[test]
    fn the_rect_does_not_wrap_above_i16() {
        let center = Position::new(40000, 40000, 7);
        assert_eq!(
            floor_viewport_rect(&center, 7),
            rect(39992, 39994, 40009, 40007)
        );
    }

    #[test]
    fn above_ground_every_surface_floor_is_visible() {
        assert_eq!(visible_floors(7), 0..=7);
        assert_eq!(visible_floors(3), 0..=7);
    }

    #[test]
    fn underground_reaches_two_floors_each_way_and_never_the_surface() {
        assert_eq!(visible_floors(8), 8..=10);
        assert_eq!(visible_floors(11), 9..=13);
        assert_eq!(visible_floors(15), 13..=15);
    }

    #[test]
    fn a_floor_out_of_range_is_outside_whatever_its_rect() {
        let center = Position::new(100, 100, 8);
        assert!(in_viewport(&center, &Position::new(100, 100, 8)));
        assert!(!in_viewport(&center, &Position::new(100, 100, 7)));
    }

    #[test]
    fn the_window_reaches_eight_left_nine_right_six_up_seven_down() {
        let center = Position::new(100, 100, 7);
        for edge in [(92, 100), (109, 100), (100, 94), (100, 107)] {
            assert!(
                in_viewport(&center, &Position::new(edge.0, edge.1, 7)),
                "{edge:?}"
            );
        }
        for beyond in [(91, 100), (110, 100), (100, 93), (100, 108)] {
            assert!(
                !in_viewport(&center, &Position::new(beyond.0, beyond.1, 7)),
                "{beyond:?}"
            );
        }
    }

    fn a_world_with_tiles(tiles: &[Position], center: Option<Position>) -> World {
        let ground = Arc::new(ItemConfig {
            id: ItemId(1),
            flags: vec![ItemFlag::Ground],
            friction: None,
            slot: None,
            minimap_color: None,
            elevation: None,
        });
        let mut map = Map::default();
        for tile in tiles {
            map.replace_tile(vec![Arc::new(Item::new(ground.clone(), 1))], tile);
        }
        let mut world = World::new();
        world.insert_resource(map);
        world.init_resource::<ChangedTileQueue>();
        world.insert_resource(ViewportCenter(center));
        world
    }

    #[test]
    fn eviction_queues_the_tiles_it_stripped_for_despawn() {
        let far = Position::new(120, 100, 7);
        let mut world = a_world_with_tiles(
            &[Position::new(100, 100, 7), far.clone()],
            Some(Position::new(100, 100, 7)),
        );

        world.run_system_once(evict_outside_viewport).unwrap();

        let queued: Vec<Position> = world
            .resource::<ChangedTileQueue>()
            .changed_positions
            .iter()
            .cloned()
            .collect();
        assert_eq!(queued, vec![far]);
    }

    #[test]
    fn with_no_center_yet_nothing_is_evicted() {
        let far = Position::new(120, 100, 7);
        let mut world = a_world_with_tiles(std::slice::from_ref(&far), None);

        world.run_system_once(evict_outside_viewport).unwrap();

        assert!(
            world
                .resource::<ChangedTileQueue>()
                .changed_positions
                .is_empty()
        );
        assert!(world.resource::<Map>().get_items(&far).is_some());
    }

    fn a_player_standing_at(world: &mut World, position: Position) {
        world.spawn((
            crate::player::components::Player {
                agent_id: crate::agent::AgentId(1),
            },
            position,
        ));
    }

    /// A hole from 7 to 8: the description of floor 8 arrives while the step onto the hole is still
    /// sliding, and the teleport waits for it to finish.
    #[test]
    fn the_floor_still_on_screen_survives_a_description_of_the_floor_below() {
        let on_screen = Position::new(100, 100, 7);
        let mut world = a_world_with_tiles(
            std::slice::from_ref(&on_screen),
            Some(Position::new(100, 100, 8)),
        );
        a_player_standing_at(&mut world, on_screen.clone());

        world.run_system_once(evict_outside_viewport).unwrap();

        assert!(world.resource::<Map>().get_items(&on_screen).is_some());
    }

    /// Stairs from 8 to 9: floor 8's window around the destination slides a tile down-right, so
    /// the player's own top row falls outside it.
    #[test]
    fn the_top_row_on_screen_survives_a_description_one_floor_down() {
        let top_row = Position::new(100, 94, 8);
        let mut world = a_world_with_tiles(
            std::slice::from_ref(&top_row),
            Some(Position::new(100, 100, 9)),
        );
        a_player_standing_at(&mut world, Position::new(100, 100, 8));

        world.run_system_once(evict_outside_viewport).unwrap();

        assert!(world.resource::<Map>().get_items(&top_row).is_some());
    }

    #[test]
    fn once_the_player_lands_the_floor_it_left_is_evicted() {
        let left_behind = Position::new(100, 100, 7);
        let mut world = a_world_with_tiles(
            std::slice::from_ref(&left_behind),
            Some(Position::new(100, 100, 8)),
        );
        a_player_standing_at(&mut world, Position::new(100, 100, 8));

        world.run_system_once(evict_outside_viewport).unwrap();

        assert!(world.resource::<Map>().get_items(&left_behind).is_none());
    }
}
