use crate::{
    conf::map::TILE_SIZE,
    core::{Appearances, InstanceManager, SheetEvicted, SheetUser, SpriteAnimator, SpriteConfig},
    items::ItemConfig,
    items::{
        Item,
        item::ItemFlag,
        material::{ItemInstance, ItemMaterial},
    },
    map::{DrawLayer, DrawOrder, DrawRank, FloorEntities, Map, Position},
};
use bevy::prelude::*;
use bevy::{asset::RenderAssetUsages, mesh::MeshTag, render::storage::ShaderStorageBuffer};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Component)]
pub struct SpawnedItem;

#[derive(Resource, Debug, Default)]
pub struct ItemState {
    pub occupied_tiles: HashMap<Position, Entity>,
}

#[derive(Resource, Debug, Default)]
pub struct LoadedMaterials {
    materials: HashMap<String, (Handle<Mesh>, Handle<ItemMaterial>)>,
    buffer: Handle<ShaderStorageBuffer>,
}

#[derive(Resource, Debug, Default)]
pub struct ChangedTileQueue {
    pub changed_positions: VecDeque<Position>,
}

pub fn setup_resources(mut commands: Commands, mut buffers: ResMut<Assets<ShaderStorageBuffer>>) {
    let loaded_materials = LoadedMaterials {
        materials: HashMap::new(),
        buffer: buffers.add(ShaderStorageBuffer::new(&[0], RenderAssetUsages::all())),
    };
    commands.insert_resource(loaded_materials);
}

pub fn on_remove_item(
    event: On<Remove, SpawnedItem>,
    tag_q: Query<&MeshTag, With<SpawnedItem>>,
    mut instances: ResMut<InstanceManager<ItemInstance>>,
) {
    let Ok(tag) = tag_q.get(event.entity) else {
        return;
    };

    instances.dealloc_index(tag.0);
}

pub fn on_sheet_evicted(event: On<SheetEvicted>, mut loaded: ResMut<LoadedMaterials>) {
    loaded.materials.remove(&event.group);
}

pub fn process_tile_changed(
    mut queue: ResMut<ChangedTileQueue>,
    mut commands: Commands,
    mut state: ResMut<ItemState>,
    mut instances: ResMut<InstanceManager<ItemInstance>>,
    mut loaded_materials: ResMut<LoadedMaterials>,
    mut materials: ResMut<Assets<ItemMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    map: Res<Map>,
    appearances: Res<Appearances>,
    floor_entities: Res<FloorEntities>,
) {
    while let Some(position) = queue.changed_positions.pop_front() {
        if let Some(entity) = state.occupied_tiles.remove(&position) {
            commands
                .entity(floor_entities.floors[position.z as usize])
                .detach_child(entity);
            commands.entity(entity).despawn();
        }

        if let Some(mut items) = map.get_items(&position).map(Iterator::peekable)
            && items.peek().is_some()
        {
            let world_pos = position.to_world();
            // The parent contributes nothing to z: a tile's ground and its wall
            // sit in different ranks, so no tile-wide base key exists to put here.
            let parent = commands
                .spawn((
                    Transform::from_xyz(world_pos.x, world_pos.y, 0.0),
                    Visibility::Inherited,
                ))
                .id();
            commands
                .entity(floor_entities.floors[position.z as usize])
                .add_child(parent);

            let mut elevation = 0.0;
            for (i, item) in items.enumerate() {
                let item_entity = spawn_item(
                    item,
                    &position,
                    i,
                    &mut commands,
                    &mut instances,
                    &mut loaded_materials,
                    &appearances,
                    &mut materials,
                    &mut meshes,
                    elevation,
                );
                commands.entity(parent).add_child(item_entity);

                if let Some(item_elev) = item.config.elevation {
                    elevation += item_elev as f32;
                    if elevation >= 24.0 {
                        elevation = 24.0;
                    }
                }
            }
            state.occupied_tiles.insert(position.clone(), parent);
        }
    }
}

/// Where an item draws: which whole-floor pass, and where within its tile.
fn placement(config: &ItemConfig) -> (DrawRank, DrawLayer) {
    if config.has_flag(ItemFlag::Top) {
        (DrawRank::Standing, DrawLayer::Top)
    } else if config.has_flag(ItemFlag::Ground) {
        (DrawRank::Ground, DrawLayer::Ground)
    } else if config.has_flag(ItemFlag::Border) {
        (DrawRank::Ground, DrawLayer::Border)
    } else if config.has_flag(ItemFlag::LyingObject) {
        (DrawRank::Standing, DrawLayer::Items)
    } else if config.has_flag(ItemFlag::Bottom) {
        (DrawRank::Standing, DrawLayer::Bottom)
    } else {
        (DrawRank::Standing, DrawLayer::Items)
    }
}

/// Cuts an item's box into the parts over each tile its sprite covers.
///
/// `bbox` and every returned box are `(min, size)` in sprite pixels, y down:
/// `Rect::max` holds the size, as the item shader reads it. Each part is paired
/// with its tile's offset from the item's own tile, which is the sprite's
/// bottom-right square; a square the box does not reach is left out.
fn lying_cells(sprite_size: Vec2, bbox: Rect) -> Vec<(IVec2, Rect)> {
    let cols = (sprite_size.x / TILE_SIZE) as i32;
    let rows = (sprite_size.y / TILE_SIZE) as i32;
    let bbox_end = bbox.min + bbox.max;

    let mut cells = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let square = Vec2::new(col as f32, row as f32) * TILE_SIZE;
            let min = bbox.min.max(square);
            let end = bbox_end.min(square + TILE_SIZE);
            if min.x < end.x && min.y < end.y {
                cells.push((
                    IVec2::new(col - (cols - 1), row - (rows - 1)),
                    Rect {
                        min,
                        max: end - min,
                    },
                ));
            }
        }
    }
    cells
}

fn cell_order(position: &Position, offset: IVec2, stack_index: u32) -> DrawOrder {
    if offset == IVec2::ZERO {
        return DrawOrder::new(
            position.clone(),
            DrawRank::Standing,
            DrawLayer::Items,
            stack_index,
        );
    }
    let tile = Position::new(
        position.x.saturating_add_signed(offset.x as i16),
        position.y.saturating_add_signed(offset.y as i16),
        position.z,
    );
    DrawOrder::new(tile, DrawRank::Standing, DrawLayer::Lying, 0)
}

fn spawn_item(
    item: &Item,
    position: &Position,
    stack_index: usize,
    commands: &mut Commands,
    instances: &mut InstanceManager<ItemInstance>,
    loaded_materials: &mut LoadedMaterials,
    appearances: &Appearances,
    materials: &mut Assets<ItemMaterial>,
    meshes: &mut Assets<Mesh>,
    elevation: f32,
) -> Entity {
    let sprite = appearances.get_item(item.config.id);
    let sheet = appearances.get_sheet(&sprite.group);

    if !loaded_materials.materials.contains_key(&sprite.group) {
        init_material(
            &sprite.group,
            materials,
            meshes,
            loaded_materials,
            appearances,
        );
    }

    let (mesh, material) = loaded_materials.materials.get(&sprite.group).unwrap();
    let patterns = item.get_patterns(position, &sprite);
    let bbox = item_bbox(&sprite, patterns.0);

    let (rank, layer) = placement(&item.config);
    let half_tile_x = if sheet.sprite_size.x <= 32.0 {
        16.0
    } else {
        0.0
    };
    let half_tile_y = if sheet.sprite_size.y <= 32.0 {
        -16.0
    } else {
        0.0
    };
    let translation = Vec3::new(-elevation + half_tile_x, elevation + half_tile_y, 0.0);

    let lying = layer == DrawLayer::Items && item.config.has_flag(ItemFlag::LyingObject);
    if !lying || sheet.sprite_size == Vec2::splat(TILE_SIZE) {
        return spawn_drawable(
            commands,
            instances,
            mesh,
            material,
            &sprite,
            patterns,
            bbox,
            translation,
            DrawOrder::new(position.clone(), rank, layer, stack_index as u32),
        );
    }

    let holder = commands
        .spawn((
            Transform::from_translation(translation),
            Visibility::Inherited,
        ))
        .id();
    for (offset, cell_box) in lying_cells(sheet.sprite_size, bbox) {
        let cell = spawn_drawable(
            commands,
            instances,
            mesh,
            material,
            &sprite,
            patterns,
            cell_box,
            Vec3::ZERO,
            cell_order(position, offset, stack_index as u32),
        );
        commands.entity(holder).add_child(cell);
    }
    holder
}

fn item_bbox(sprite: &SpriteConfig, pattern_x: u32) -> Rect {
    sprite
        .boxes
        .get(pattern_x as usize)
        .copied()
        .unwrap_or(Rect {
            min: Vec2::ZERO,
            max: Vec2::splat(TILE_SIZE),
        })
}

fn spawn_drawable(
    commands: &mut Commands,
    instances: &mut InstanceManager<ItemInstance>,
    mesh: &Handle<Mesh>,
    material: &Handle<ItemMaterial>,
    sprite: &Arc<SpriteConfig>,
    (px, py, pz): (u32, u32, u32),
    bbox: Rect,
    translation: Vec3,
    order: DrawOrder,
) -> Entity {
    let index = instances.alloc_index();
    let instance = instances.get_mut(index);
    instance.bbox_min = bbox.min;
    instance.bbox_size = bbox.max;
    instance.shift = sprite.shift;

    let animator = SpriteAnimator::new(Arc::clone(sprite), px, py, pz);
    instance.sprite_id = animator.current_sprite_ids[0];

    commands
        .spawn((
            SpawnedItem,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            MeshTag(index),
            Transform::from_translation(translation),
            order,
            Visibility::Inherited,
            animator,
            SheetUser(sprite.group.clone()),
        ))
        .id()
}

fn init_material(
    group: &str,
    materials: &mut Assets<ItemMaterial>,
    meshes: &mut Assets<Mesh>,
    loaded_materials: &mut LoadedMaterials,
    appearances: &Appearances,
) {
    let sheet = appearances.get_sheet(group);
    let material_handle = materials.add(ItemMaterial {
        texture: sheet.texture(),
        atlas_grid: sheet.grid_size,
        mesh_size: sheet.sprite_size,
        instances: loaded_materials.buffer.clone(),
    });
    let mesh_handle = meshes.add(Mesh::from(Rectangle::new(
        sheet.sprite_size.x,
        sheet.sprite_size.y,
    )));
    loaded_materials
        .materials
        .insert(group.to_string(), (mesh_handle, material_handle));
}

pub fn update_item_instances(
    items_q: Query<(&SpriteAnimator, &MeshTag), (With<SpawnedItem>, Changed<SpriteAnimator>)>,
    mut instances: ResMut<InstanceManager<ItemInstance>>,
) {
    for (animator, tag) in &items_q {
        instances.update(tag.0, |instance| {
            instance.sprite_id = animator.current_sprite_ids[0];
        });
    }
}

pub fn upload_instance_buffer(
    mut instances: ResMut<InstanceManager<ItemInstance>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    loaded_materials: Res<LoadedMaterials>,
    mut materials: ResMut<Assets<ItemMaterial>>,
) {
    if !instances.is_dirty() {
        return;
    }

    let Some(ssb) = buffers.get_mut(&loaded_materials.buffer) else {
        return;
    };
    ssb.set_data(instances.get_buffer_data());
    instances.reset_dirty();

    for (_, mat) in loaded_materials.materials.values() {
        let _ = materials.get_mut(mat);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::ItemId;
    use crate::map::{DrawOrder, DrawOrigin, Position};

    fn config(flags: Vec<ItemFlag>) -> ItemConfig {
        ItemConfig {
            id: ItemId(1),
            flags,
            friction: None,
            slot: None,
            minimap_color: None,
            elevation: None,
        }
    }

    /// The flags an item really carries, taken from `appearances.json`: a blood
    /// pool is `bottom` + `liquidpool` and no `lying_object`, while a corpse is
    /// `lying_object` and carries no placement flag at all.
    #[test]
    fn a_corpse_lies_over_its_own_blood_pool() {
        let pool = placement(&config(vec![
            ItemFlag::Bottom,
            ItemFlag::LiquidPool,
            ItemFlag::Unmove,
        ]));
        let corpse = placement(&config(vec![ItemFlag::LyingObject, ItemFlag::Container]));

        assert_eq!(pool, (DrawRank::Standing, DrawLayer::Bottom));
        assert_eq!(corpse, (DrawRank::Standing, DrawLayer::Items));

        let tile = Position::new(1000, 1000, 7);
        let origin = DrawOrigin::around(&tile);
        let key = |(rank, layer)| DrawOrder::new(tile.clone(), rank, layer, 0).key(&origin);

        assert!(key(pool) < key(corpse));
    }

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect {
            min: Vec2::new(x, y),
            max: Vec2::new(w, h),
        }
    }

    #[test]
    fn a_full_2x2_sprite_splits_into_four_cells() {
        let cells = lying_cells(Vec2::new(64.0, 64.0), rect(0.0, 0.0, 64.0, 64.0));

        assert_eq!(
            cells,
            vec![
                (IVec2::new(-1, -1), rect(0.0, 0.0, 32.0, 32.0)),
                (IVec2::new(0, -1), rect(32.0, 0.0, 32.0, 32.0)),
                (IVec2::new(-1, 0), rect(0.0, 32.0, 32.0, 32.0)),
                (IVec2::new(0, 0), rect(32.0, 32.0, 32.0, 32.0)),
            ]
        );
    }

    /// A real corpse box (item 52861), a small body straddling the centre.
    #[test]
    fn a_cell_holds_only_the_part_of_the_box_over_its_tile() {
        let cells = lying_cells(Vec2::new(64.0, 64.0), rect(30.0, 28.0, 34.0, 36.0));

        assert_eq!(
            cells,
            vec![
                (IVec2::new(-1, -1), rect(30.0, 28.0, 2.0, 4.0)),
                (IVec2::new(0, -1), rect(32.0, 28.0, 32.0, 4.0)),
                (IVec2::new(-1, 0), rect(30.0, 32.0, 2.0, 32.0)),
                (IVec2::new(0, 0), rect(32.0, 32.0, 32.0, 32.0)),
            ]
        );
    }

    #[test]
    fn a_wide_or_tall_sprite_splits_in_two() {
        assert_eq!(
            lying_cells(Vec2::new(64.0, 32.0), rect(17.0, 2.0, 47.0, 29.0)),
            vec![
                (IVec2::new(-1, 0), rect(17.0, 2.0, 15.0, 29.0)),
                (IVec2::new(0, 0), rect(32.0, 2.0, 32.0, 29.0)),
            ]
        );
        assert_eq!(
            lying_cells(Vec2::new(32.0, 64.0), rect(0.0, 25.0, 31.0, 38.0)),
            vec![
                (IVec2::new(0, -1), rect(0.0, 25.0, 31.0, 7.0)),
                (IVec2::new(0, 0), rect(0.0, 32.0, 31.0, 31.0)),
            ]
        );
    }

    #[test]
    fn a_square_the_box_does_not_reach_has_no_cell() {
        assert_eq!(
            lying_cells(Vec2::new(64.0, 64.0), rect(32.0, 32.0, 32.0, 32.0)),
            vec![(IVec2::ZERO, rect(32.0, 32.0, 32.0, 32.0))]
        );
    }

    #[test]
    fn a_one_tile_sprite_is_one_unchanged_cell() {
        let bbox = rect(1.0, 0.0, 31.0, 32.0);

        assert_eq!(
            lying_cells(Vec2::new(32.0, 32.0), bbox),
            vec![(IVec2::ZERO, bbox)]
        );
    }

    #[test]
    fn a_corpses_own_cell_keeps_its_stack_slot() {
        let tile = Position::new(1000, 1000, 7);

        assert_eq!(
            cell_order(&tile, IVec2::ZERO, 3),
            DrawOrder::new(tile.clone(), DrawRank::Standing, DrawLayer::Items, 3)
        );
    }

    #[test]
    fn a_spread_cell_keys_to_the_tile_it_covers() {
        let tile = Position::new(1000, 1000, 7);

        assert_eq!(
            cell_order(&tile, IVec2::new(-1, -1), 3),
            DrawOrder::new(
                Position::new(999, 999, 7),
                DrawRank::Standing,
                DrawLayer::Lying,
                0
            )
        );
        assert_eq!(
            cell_order(&tile, IVec2::new(0, -1), 3).pos,
            Position::new(1000, 999, 7)
        );
    }

    /// Grounds and borders are the pass that goes first, so nothing they draw
    /// under can be cut by them.
    #[test]
    fn grounds_and_borders_are_the_first_pass() {
        assert_eq!(
            placement(&config(vec![ItemFlag::Ground])),
            (DrawRank::Ground, DrawLayer::Ground)
        );
        assert_eq!(
            placement(&config(vec![ItemFlag::Border])),
            (DrawRank::Ground, DrawLayer::Border)
        );
    }

    /// `Top` wins over everything, including a lying flag, because a door drawn
    /// under the floor it hangs in is worse than one drawn over a corpse.
    #[test]
    fn a_top_item_stays_on_top_whatever_else_it_carries() {
        assert_eq!(
            placement(&config(vec![ItemFlag::Top, ItemFlag::LyingObject])),
            (DrawRank::Standing, DrawLayer::Top)
        );
    }

    #[test]
    fn a_plain_item_stands_on_its_tile() {
        assert_eq!(
            placement(&config(vec![ItemFlag::Take])),
            (DrawRank::Standing, DrawLayer::Items)
        );
    }

    #[test]
    fn an_evicted_sheet_loses_its_item_material() {
        let mut world = World::new();
        world.insert_resource(LoadedMaterials {
            materials: HashMap::from([(
                "item-a".to_owned(),
                (Handle::default(), Handle::default()),
            )]),
            buffer: Handle::default(),
        });
        world.add_observer(on_sheet_evicted);

        world.trigger(crate::core::SheetEvicted {
            group: "item-a".to_owned(),
            sheet_name: "item-a.png".to_owned(),
        });

        assert!(world.resource::<LoadedMaterials>().materials.is_empty());
    }
}
