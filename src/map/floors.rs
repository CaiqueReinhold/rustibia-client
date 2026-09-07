use bevy::prelude::*;

use crate::{
    agent::WalkingDirection,
    conf::map::{BASE_FLOOR, MAX_FLOOR, MIN_FLOOR, UNDERGROUND_REACH},
    map::{Map, Position},
    player::components::Player,
};

#[derive(Resource, Debug)]
pub struct FloorEntities {
    pub floors: [Entity; (MAX_FLOOR + 1) as usize],
}

pub fn setup_floors(mut commands: Commands) {
    let mut floors = Vec::new();
    for _ in MIN_FLOOR..=MAX_FLOOR {
        floors.push(
            commands
                .spawn((Transform::default(), GlobalTransform::default()))
                .id(),
        );
    }
    commands.insert_resource(FloorEntities {
        floors: floors.try_into().unwrap(),
    });
}

/// The tile on `floor` that visually covers `pos`: one tile `dir`-wards per floor
/// between the two. `None` when that tile falls off the map.
fn covering_tile(dir: WalkingDirection, pos: &Position, floor: u8) -> Option<Position> {
    let floor_offset = (pos.z as i32) - (floor as i32);
    let (dx, dy) = match dir {
        WalkingDirection::East => (floor_offset, 0),
        WalkingDirection::West => (-floor_offset, 0),
        WalkingDirection::North => (0, -floor_offset),
        WalkingDirection::South => (0, floor_offset),
        _ => (0, 0),
    };

    let x = u16::try_from(pos.x as i32 + dx).ok()?;
    let y = u16::try_from(pos.y as i32 + dy).ok()?;
    Some(Position::new(x, y, floor))
}

fn has_oclusion(dir: WalkingDirection, pos: &Position, floor: u8, map: &Map) -> bool {
    let Some(offset_pos) = covering_tile(dir, pos, floor) else {
        return false;
    };

    if map.is_bottom(&offset_pos) && matches!(dir, WalkingDirection::East | WalkingDirection::South)
    {
        return true;
    }

    (!map.block_sight(&(pos.clone() + dir)) || (floor as i32 - pos.z as i32) < -1)
        && map.is_ground(&offset_pos)
}

fn is_floor_visible(map: &Map, pos: &Position, floor: u8) -> bool {
    if (pos.z <= BASE_FLOOR) && (floor > BASE_FLOOR) {
        return false;
    }

    if (pos.z > BASE_FLOOR) && (floor <= BASE_FLOOR) {
        return false;
    }

    if pos.z == floor {
        return true;
    }

    if floor > pos.z {
        return true;
    }

    if has_oclusion(WalkingDirection::North, pos, floor, map) {
        return false;
    }

    if has_oclusion(WalkingDirection::East, pos, floor, map) {
        return false;
    }

    if has_oclusion(WalkingDirection::South, pos, floor, map) {
        return false;
    }

    if has_oclusion(WalkingDirection::West, pos, floor, map) {
        return false;
    }

    true
}

pub fn update_floors_visibility(
    mut commands: Commands,
    position_q: Query<&Position, (With<Player>, Changed<Position>)>,
    floor_ents: Res<FloorEntities>,
    map: Res<Map>,
) {
    let Ok(position) = position_q.single() else {
        return;
    };

    let mut set = |floor: u8, visibility: Visibility| {
        commands
            .entity(floor_ents.floors[floor as usize])
            .insert(visibility);
    };

    if position.z <= BASE_FLOOR {
        let mut z = position.z as i16;
        while z >= MIN_FLOOR as i16 && is_floor_visible(&map, position, z as u8) {
            set(z as u8, Visibility::Visible);
            z -= 1;
        }
        for hidden in MIN_FLOOR as i16..=z {
            set(hidden as u8, Visibility::Hidden);
        }

        for z in (position.z + 1)..=BASE_FLOOR {
            set(z, Visibility::Visible);
        }
        for z in (BASE_FLOOR + 1)..=MAX_FLOOR {
            set(z, Visibility::Hidden);
        }
    } else {
        let shallowest = position
            .z
            .saturating_sub(UNDERGROUND_REACH)
            .max(BASE_FLOOR + 1);
        let mut z = position.z as i16;
        while z >= shallowest as i16 && is_floor_visible(&map, position, z as u8) {
            set(z as u8, Visibility::Visible);
            z -= 1;
        }
        for hidden in (BASE_FLOOR as i16 + 1)..=z {
            set(hidden as u8, Visibility::Hidden);
        }

        let deepest = (position.z + UNDERGROUND_REACH).min(MAX_FLOOR);
        for z in (position.z + 1)..=deepest {
            set(z, Visibility::Visible);
        }
        for z in (deepest + 1)..=MAX_FLOOR {
            set(z, Visibility::Hidden);
        }
        for z in MIN_FLOOR..=BASE_FLOOR {
            set(z, Visibility::Hidden);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Agent;
    use crate::agent::AgentId;
    use crate::items::ItemId;
    use crate::items::{Item, ItemConfig, ItemFlag};
    use bevy::ecs::system::RunSystemOnce;
    use std::sync::Arc;

    fn ground_at(map: &mut Map, pos: Position) {
        map.replace_tile(
            vec![Arc::new(Item::new(
                Arc::new(ItemConfig {
                    id: ItemId(100),
                    flags: vec![ItemFlag::Ground],
                    friction: Some(150),
                    slot: None,
                    minimap_color: None,
                    elevation: None,
                }),
                1,
            ))],
            &pos,
        );
    }

    /// The offset is the gap to the *player's* floor, not to `BASE_FLOOR`, and it
    /// runs down-right.
    #[test]
    fn the_covering_tile_is_measured_from_the_players_floor() {
        let player = Position::new(100, 100, 9);

        assert_eq!(
            covering_tile(WalkingDirection::East, &player, 8),
            Some(Position::new(101, 100, 8)),
        );
        assert_eq!(
            covering_tile(WalkingDirection::South, &player, 8),
            Some(Position::new(100, 101, 8)),
        );
        // Two floors up, two tiles along.
        assert_eq!(
            covering_tile(WalkingDirection::East, &player, 7),
            Some(Position::new(102, 100, 7)),
        );
    }

    /// A player on `BASE_FLOOR` is the case every other test on this system was
    /// written against; it must keep its answers.
    #[test]
    fn the_surface_offset_is_unchanged() {
        let player = Position::new(100, 100, 7);

        assert_eq!(
            covering_tile(WalkingDirection::East, &player, 6),
            Some(Position::new(101, 100, 6)),
        );
        assert_eq!(
            covering_tile(WalkingDirection::North, &player, 5),
            Some(Position::new(100, 98, 5)),
        );
    }

    /// A covering tile off the map is no tile, so it hides nothing — and asking
    /// for one must not wrap into a real coordinate.
    #[test]
    fn a_covering_tile_off_the_map_hides_nothing() {
        let mut map = Map::default();
        // Ground on the tiles a wrapped coordinate would land on.
        for pos in [Position::new(65535, 0, 8), Position::new(0, 65535, 8)] {
            ground_at(&mut map, pos);
        }
        let player = Position::new(0, 0, 9);

        assert!(covering_tile(WalkingDirection::West, &player, 8).is_none());
        assert!(covering_tile(WalkingDirection::North, &player, 8).is_none());
        assert!(
            is_floor_visible(&map, &player, 8),
            "nothing covers a player in the map's corner"
        );
    }

    /// Ground on the covering tile is a ceiling. Two floors up, not one: the four
    /// probes are symmetric, so a one-floor gap cannot tell a sign error from a
    /// correct offset.
    #[test]
    fn a_ceiling_two_floors_up_hides_that_floor() {
        let player = Position::new(100, 100, 10);

        let mut covered = Map::default();
        ground_at(&mut covered, Position::new(102, 100, 8));
        assert!(!is_floor_visible(&covered, &player, 8));

        let mut uncovered = Map::default();
        ground_at(&mut uncovered, Position::new(101, 100, 8));
        assert!(
            is_floor_visible(&uncovered, &player, 8),
            "one tile along is the covering tile for floor 9, not floor 8"
        );
    }

    /// Occlusion only ever asks what is above the player.
    #[test]
    fn a_deeper_floor_is_never_occluded() {
        let mut map = Map::default();
        ground_at(&mut map, Position::new(101, 100, 10));
        ground_at(&mut map, Position::new(99, 100, 10));

        assert!(is_floor_visible(&map, &Position::new(100, 100, 9), 10));
    }

    fn world_with_floors(player: Position, map: Map) -> (World, Vec<Entity>) {
        let mut world = World::new();
        world.insert_resource(map);
        let floors: Vec<Entity> = (MIN_FLOOR..=MAX_FLOOR)
            .map(|_| world.spawn(Visibility::Inherited).id())
            .collect();
        world.insert_resource(FloorEntities {
            floors: floors.clone().try_into().unwrap(),
        });
        world.spawn((
            Player {
                agent_id: AgentId(1),
            },
            Agent::default(),
            player,
        ));
        (world, floors)
    }

    fn visibility(world: &World, floors: &[Entity], z: u8) -> Option<Visibility> {
        world.get::<Visibility>(floors[z as usize]).copied()
    }

    /// The view reaches `UNDERGROUND_REACH` down and no further. A floor drawn
    /// past that shows tiles the previous position left in it.
    #[test]
    fn underground_the_view_stops_two_floors_down() {
        let (mut world, floors) = world_with_floors(Position::new(100, 100, 8), Map::default());

        world.run_system_once(update_floors_visibility).unwrap();

        assert_eq!(visibility(&world, &floors, 8), Some(Visibility::Visible));
        assert_eq!(visibility(&world, &floors, 9), Some(Visibility::Visible));
        assert_eq!(visibility(&world, &floors, 10), Some(Visibility::Visible));
        for z in 11..=MAX_FLOOR {
            assert_eq!(
                visibility(&world, &floors, z),
                Some(Visibility::Hidden),
                "floor {z} is out of reach"
            );
        }
    }

    /// `MAX_FLOOR` is inside the range that gets hidden, not one past its end.
    #[test]
    fn the_deepest_floor_is_hidden_when_out_of_reach() {
        let (mut world, floors) = world_with_floors(Position::new(100, 100, 12), Map::default());

        world.run_system_once(update_floors_visibility).unwrap();

        assert_eq!(visibility(&world, &floors, 14), Some(Visibility::Visible));
        assert_eq!(
            visibility(&world, &floors, MAX_FLOOR),
            Some(Visibility::Hidden)
        );
    }

    /// On the bottom floor there is nothing below to reach; the ceiling window
    /// still runs upward.
    #[test]
    fn the_bottom_floor_is_drawn_when_standing_on_it() {
        let (mut world, floors) = world_with_floors(Position::new(100, 100, 15), Map::default());

        world.run_system_once(update_floors_visibility).unwrap();

        assert_eq!(visibility(&world, &floors, 15), Some(Visibility::Visible));
        assert_eq!(visibility(&world, &floors, 13), Some(Visibility::Visible));
        assert_eq!(visibility(&world, &floors, 12), Some(Visibility::Hidden));
    }

    /// A ceiling hides itself and every floor above it, all the way to the
    /// surface.
    #[test]
    fn underground_a_ceiling_hides_itself_and_everything_above_it() {
        let mut map = Map::default();
        // Covers a player on 10 from floor 9.
        ground_at(&mut map, Position::new(101, 100, 9));
        let (mut world, floors) = world_with_floors(Position::new(100, 100, 10), map);

        world.run_system_once(update_floors_visibility).unwrap();

        assert_eq!(visibility(&world, &floors, 10), Some(Visibility::Visible));
        assert_eq!(visibility(&world, &floors, 9), Some(Visibility::Hidden));
        assert_eq!(visibility(&world, &floors, 8), Some(Visibility::Hidden));
        for z in MIN_FLOOR..=BASE_FLOOR {
            assert_eq!(visibility(&world, &floors, z), Some(Visibility::Hidden));
        }
    }

    /// With no ceiling, every floor above ground is drawn and every one below is
    /// hidden.
    #[test]
    fn on_the_surface_every_floor_above_ground_is_drawn() {
        let (mut world, floors) = world_with_floors(Position::new(100, 100, 7), Map::default());

        world.run_system_once(update_floors_visibility).unwrap();

        for z in MIN_FLOOR..=BASE_FLOOR {
            assert_eq!(visibility(&world, &floors, z), Some(Visibility::Visible));
        }
        for z in (BASE_FLOOR + 1)..=MAX_FLOOR {
            assert_eq!(visibility(&world, &floors, z), Some(Visibility::Hidden));
        }
    }

    /// Floor 0 is hideable — it is the inclusive end of the hidden range, not one
    /// past it.
    #[test]
    fn floor_zero_is_hidden_when_it_is_the_ceiling() {
        let mut map = Map::default();
        // Covers a player on 1 from floor 0.
        ground_at(&mut map, Position::new(101, 100, 0));
        let (mut world, floors) = world_with_floors(Position::new(100, 100, 1), map);

        world.run_system_once(update_floors_visibility).unwrap();

        assert_eq!(visibility(&world, &floors, 1), Some(Visibility::Visible));
        assert_eq!(visibility(&world, &floors, 0), Some(Visibility::Hidden));
    }
}
