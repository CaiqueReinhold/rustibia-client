use crate::{
    core::{Appearances, InstanceManager, SpriteAnimator, SpriteConfig},
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

        if let Some(items) = map.get_items(&position) {
            let world_pos = position.to_world();
            // The parent contributes nothing to z: a tile's ground, its corpse
            // and its wall sit in three different ranks, so no tile-wide base
            // key exists to put here.
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
///
/// The order of the tests is load-bearing: a blood pool carries `bottom` AND
/// `liquidpool` in the appearance data, so `LiquidPool` has to come before
/// `Bottom` or the pool ranks with the walls and draws over the corpse that
/// bled it.
fn placement(config: &ItemConfig) -> (DrawRank, DrawLayer) {
    if config.has_flag(ItemFlag::Top) {
        (DrawRank::Standing, DrawLayer::Top)
    } else if config.has_flag(ItemFlag::Ground) {
        (DrawRank::Ground, DrawLayer::Ground)
    } else if config.has_flag(ItemFlag::Border) {
        (DrawRank::Ground, DrawLayer::Border)
    } else if config.has_flag(ItemFlag::LyingObject) {
        (DrawRank::Lying, DrawLayer::Items)
    } else if config.has_flag(ItemFlag::LiquidPool) {
        (DrawRank::Lying, DrawLayer::Bottom)
    } else if config.has_flag(ItemFlag::Bottom) {
        (DrawRank::Standing, DrawLayer::Bottom)
    } else {
        (DrawRank::Standing, DrawLayer::Items)
    }
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
    let index = instances.alloc_index();
    let instance = instances.get_mut(index);
    let patterns = item.get_patterns(position, &sprite);
    let (px, py, pz) = patterns;
    init_instance(instance, &sprite, patterns);

    let animator = SpriteAnimator::new(Arc::clone(&sprite), px, py, pz);
    instance.sprite_id = animator.current_sprite_ids[0];

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

    commands
        .spawn((
            SpawnedItem,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            MeshTag(index),
            Transform::from_translation(translation),
            DrawOrder::new(position.clone(), rank, layer, stack_index as u32),
            Visibility::Inherited,
            animator,
        ))
        .id()
}

fn init_instance(instance: &mut ItemInstance, sprite: &SpriteConfig, patterns: (u32, u32, u32)) {
    let (px, _py, _pz) = patterns;
    if !sprite.boxes.is_empty() {
        let bbox = &sprite.boxes[px as usize];
        instance.bbox_min = bbox.min;
        instance.bbox_size = bbox.max;
    } else {
        instance.bbox_min = Vec2::ZERO;
        instance.bbox_size = Vec2::new(32.0, 32.0);
    }
    instance.shift = sprite.shift;
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
        texture: sheet.texture().clone(),
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
    /// pool is `bottom` + `liquidpool` and NO `lying_object`, while a corpse is
    /// `lying_object` and carries no placement flag at all. Ranking the pool by
    /// its `bottom` flag would put it in with the walls and it would then draw
    /// over the very corpse that bled it.
    #[test]
    fn a_blood_pool_ranks_with_the_corpse_and_not_with_the_walls() {
        let pool = placement(&config(vec![
            ItemFlag::Bottom,
            ItemFlag::LiquidPool,
            ItemFlag::Unmove,
        ]));
        let corpse = placement(&config(vec![ItemFlag::LyingObject, ItemFlag::Container]));
        let wall = placement(&config(vec![ItemFlag::Bottom, ItemFlag::Unpass]));

        assert_eq!(pool, (DrawRank::Lying, DrawLayer::Bottom));
        assert_eq!(corpse, (DrawRank::Lying, DrawLayer::Items));
        assert_eq!(wall, (DrawRank::Standing, DrawLayer::Bottom));

        let tile = Position::new(1000, 1000, 7);
        let origin = DrawOrigin::around(&tile);
        let key = |(rank, layer)| DrawOrder::new(tile.clone(), rank, layer, 0).key(&origin);

        assert!(key(pool) < key(corpse), "the body lies on the pool");
        assert!(key(corpse) < key(wall), "and a wall stands over both");
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
}
