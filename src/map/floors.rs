use std::ops::RangeInclusive;

use bevy::prelude::*;

use crate::{
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

fn covered_up(pos: &Position) -> Option<Position> {
    Some(Position::new(
        pos.x.checked_add(1)?,
        pos.y.checked_add(1)?,
        pos.z.checked_sub(1)?,
    ))
}

/// The highest floor drawn over `camera`. A transcription of OTClient's
/// `MapView::calcFirstVisibleFloor`.
fn first_visible_floor(map: &Map, camera: &Position) -> u8 {
    let mut first = if camera.z > BASE_FLOOR {
        camera
            .z
            .saturating_sub(UNDERGROUND_REACH)
            .max(BASE_FLOOR + 1)
    } else {
        MIN_FLOOR
    };

    for ix in -1i32..=1 {
        for iy in -1i32..=1 {
            if first >= camera.z {
                return first;
            }
            let (Ok(x), Ok(y)) = (
                u16::try_from(camera.x as i32 + ix),
                u16::try_from(camera.y as i32 + iy),
            ) else {
                continue;
            };
            let pos = Position::new(x, y, camera.z);
            let look_possible = map.is_look_possible(&pos);
            let centre = ix == 0 && iy == 0;
            if !centre && (ix.abs() == iy.abs() || !look_possible) {
                continue;
            }

            let mut upper = pos.clone();
            let mut covered = pos;
            while let Some(next) = covered_up(&covered) {
                covered = next;
                upper.z = covered.z;
                if upper.z < first {
                    break;
                }
                if map.limits_floors_view(&upper, !look_possible) {
                    first = upper.z + 1;
                    break;
                }
                if map.limits_floors_view(&covered, look_possible) {
                    first = covered.z + 1;
                    break;
                }
            }
        }
    }
    first
}

pub fn update_floors_visibility(
    mut commands: Commands,
    player_q: Query<&Position, With<Player>>,
    floor_ents: Res<FloorEntities>,
    map: Res<Map>,
    mut drawn: Local<Option<RangeInclusive<u8>>>,
) {
    let Ok(camera) = player_q.single() else {
        return;
    };

    let last = if camera.z <= BASE_FLOOR {
        BASE_FLOOR
    } else {
        (camera.z + UNDERGROUND_REACH).min(MAX_FLOOR)
    };
    let visible = first_visible_floor(&map, camera)..=last;
    if drawn.as_ref() == Some(&visible) {
        return;
    }

    for z in MIN_FLOOR..=MAX_FLOOR {
        let visibility = if visible.contains(&z) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        commands
            .entity(floor_ents.floors[z as usize])
            .insert(visibility);
    }
    *drawn = Some(visible);
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

    fn item(flags: Vec<ItemFlag>) -> Arc<Item> {
        Arc::new(Item::new(
            Arc::new(ItemConfig {
                id: ItemId(100),
                flags,
                friction: None,
                slot: None,
                minimap_color: None,
                elevation: None,
            }),
            1,
        ))
    }

    fn surface_player(map: &mut Map) -> Position {
        let player = Position::new(100, 100, 7);
        ground_at(map, player.clone());
        player
    }

    #[test]
    fn nothing_above_shows_every_floor_from_the_top() {
        let mut map = Map::default();
        let player = surface_player(&mut map);

        assert_eq!(first_visible_floor(&map, &player), 0);
    }

    #[test]
    fn underground_the_view_reaches_two_floors_up_and_never_the_surface() {
        let mut deep = Map::default();
        ground_at(&mut deep, Position::new(100, 100, 10));
        assert_eq!(first_visible_floor(&deep, &Position::new(100, 100, 10)), 8);

        let mut shallow = Map::default();
        ground_at(&mut shallow, Position::new(100, 100, 8));
        assert_eq!(
            first_visible_floor(&shallow, &Position::new(100, 100, 8)),
            8
        );
    }

    #[test]
    fn ground_directly_above_the_player_is_a_ceiling() {
        let mut map = Map::default();
        let player = surface_player(&mut map);
        ground_at(&mut map, Position::new(100, 100, 5));

        assert_eq!(first_visible_floor(&map, &player), 6);
    }

    /// Two floors up the covering tile is two along on both axes; a probe along one
    /// axis only lands on a tile that covers nothing.
    #[test]
    fn the_tile_drawn_over_the_player_is_a_ceiling() {
        let mut covered = Map::default();
        let player = surface_player(&mut covered);
        ground_at(&mut covered, Position::new(102, 102, 5));
        assert_eq!(first_visible_floor(&covered, &player), 6);

        let mut beside = Map::default();
        surface_player(&mut beside);
        ground_at(&mut beside, Position::new(102, 100, 5));
        assert_eq!(first_visible_floor(&beside, &player), 0);
    }

    #[test]
    fn a_see_through_neighbour_is_probed() {
        let mut map = Map::default();
        let player = surface_player(&mut map);
        ground_at(&mut map, Position::new(101, 100, 7));
        ground_at(&mut map, Position::new(101, 100, 5));

        assert_eq!(first_visible_floor(&map, &player), 6);
    }

    #[test]
    fn a_neighbour_that_blocks_sight_is_not_probed() {
        let mut map = Map::default();
        let player = surface_player(&mut map);
        map.replace_tile(
            vec![
                item(vec![ItemFlag::Ground]),
                item(vec![ItemFlag::Bottom, ItemFlag::BlockSight]),
            ],
            &Position::new(101, 100, 7),
        );
        ground_at(&mut map, Position::new(101, 100, 5));

        assert_eq!(first_visible_floor(&map, &player), 0);
    }

    #[test]
    fn a_missing_neighbour_is_not_probed() {
        let mut map = Map::default();
        let player = surface_player(&mut map);
        ground_at(&mut map, Position::new(101, 100, 5));

        assert_eq!(first_visible_floor(&map, &player), 0);
    }

    #[test]
    fn diagonal_neighbours_are_not_probed() {
        let mut map = Map::default();
        let player = surface_player(&mut map);
        ground_at(&mut map, Position::new(101, 101, 7));
        ground_at(&mut map, Position::new(101, 101, 5));

        assert_eq!(first_visible_floor(&map, &player), 0);
    }

    /// The west neighbour is probed before the player's own tile, so it finds the
    /// higher ceiling first; the lower one must still win.
    #[test]
    fn a_later_probe_lowers_the_first_floor() {
        let mut map = Map::default();
        let player = surface_player(&mut map);
        ground_at(&mut map, Position::new(99, 100, 7));
        ground_at(&mut map, Position::new(99, 100, 4));
        ground_at(&mut map, Position::new(100, 100, 6));

        assert_eq!(first_visible_floor(&map, &player), 7);
    }

    #[test]
    fn the_map_edge_does_not_wrap() {
        let mut map = Map::default();
        let player = Position::new(65535, 65535, 7);
        ground_at(&mut map, player.clone());
        ground_at(&mut map, Position::new(0, 0, 6));
        ground_at(&mut map, Position::new(0, 0, 5));

        assert_eq!(first_visible_floor(&map, &player), 0);
    }

    /// The physical probe uses the strict test and the geometric one the free test.
    #[test]
    fn a_plain_wall_hides_only_from_the_covering_tile() {
        let mut above = Map::default();
        let player = surface_player(&mut above);
        above.replace_tile(
            vec![item(vec![ItemFlag::Bottom])],
            &Position::new(100, 100, 6),
        );
        assert_eq!(first_visible_floor(&above, &player), 0);

        let mut covering = Map::default();
        surface_player(&mut covering);
        covering.replace_tile(
            vec![item(vec![ItemFlag::Bottom])],
            &Position::new(101, 101, 6),
        );
        assert_eq!(first_visible_floor(&covering, &player), 7);
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
        ground_at(&mut map, Position::new(100, 100, 10));
        ground_at(&mut map, Position::new(101, 101, 9));
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
        ground_at(&mut map, Position::new(100, 100, 1));
        ground_at(&mut map, Position::new(101, 101, 0));
        let (mut world, floors) = world_with_floors(Position::new(100, 100, 1), map);

        world.run_system_once(update_floors_visibility).unwrap();

        assert_eq!(visibility(&world, &floors, 1), Some(Visibility::Visible));
        assert_eq!(visibility(&world, &floors, 0), Some(Visibility::Hidden));
    }

    #[test]
    fn a_ceiling_that_arrives_without_a_step_hides_its_floor() {
        let mut map = Map::default();
        let player = surface_player(&mut map);
        let (mut world, floors) = world_with_floors(player, map);
        let system = world.register_system(update_floors_visibility);

        world.run_system(system).unwrap();
        assert_eq!(visibility(&world, &floors, 6), Some(Visibility::Visible));

        ground_at(&mut world.resource_mut::<Map>(), Position::new(101, 101, 6));
        world.run_system(system).unwrap();
        assert_eq!(visibility(&world, &floors, 6), Some(Visibility::Hidden));
    }
}
