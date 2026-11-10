use crate::{
    conf::map::TILE_SIZE,
    core::{Appearances, InstanceManager, SheetEvicted, SheetUser, SpriteAnimator, SpriteConfig},
    items::ItemConfig,
    items::{
        Item,
        item::ItemFlag,
        material::{ItemInstance, ItemMaterial},
    },
    map::{DrawLayer, DrawOrder, DrawRank, FloorEntities, Map, Position, draws_before},
};
use bevy::prelude::*;
use bevy::{asset::RenderAssetUsages, mesh::MeshTag, render::storage::ShaderStorageBuffer};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[derive(Component)]
pub struct SpawnedItem;

/// A lying item's cell on a tile it spreads onto: `tile` is the one it covers,
/// `home` the item's own. It draws with the item, over everything between the
/// two, until a creature, effect or missile is drawn over it; then it keys to
/// that one's tile, under it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SpreadCell {
    tile: Position,
    home: Position,
    slot: u32,
}

impl SpreadCell {
    /// Whether something keyed to `drawn` overlaps this cell and would be
    /// covered by it. A 64 px sprite reaches one tile up and left of its key.
    fn is_under(&self, drawn: &Position) -> bool {
        drawn.z == self.tile.z
            && matches!(drawn.x.wrapping_sub(self.tile.x), 0 | 1)
            && matches!(drawn.y.wrapping_sub(self.tile.y), 0 | 1)
            && draws_before(drawn, &self.home)
    }

    /// `under` is the tile of the first thing drawn over the cell, if any.
    fn placement<'a>(&'a self, under: Option<&'a Position>) -> (&'a Position, DrawLayer, u32) {
        match under {
            Some(tile) => (tile, DrawLayer::Lying, 0),
            None => (&self.home, DrawLayer::Items, self.slot),
        }
    }
}

pub fn settle_spread_cells(
    mut cells: Query<(&SpreadCell, &mut DrawOrder), With<SpawnedItem>>,
    others: Query<&DrawOrder, Without<SpawnedItem>>,
) {
    for (cell, mut order) in &mut cells {
        let under = others
            .iter()
            .filter(|other| other.at_or_above(DrawLayer::Creature) && cell.is_under(&other.pos))
            .map(|other| &other.pos)
            .reduce(|first, pos| if draws_before(pos, first) { pos } else { first });
        let (pos, layer, slot) = cell.placement(under);
        DrawOrder::move_to(&mut order, pos, layer, slot);
    }
}

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

fn spread_cell(position: &Position, offset: IVec2, stack_index: u32) -> Option<SpreadCell> {
    (offset != IVec2::ZERO).then(|| SpreadCell {
        tile: Position::new(
            position.x.saturating_add_signed(offset.x as i16),
            position.y.saturating_add_signed(offset.y as i16),
            position.z,
        ),
        home: position.clone(),
        slot: stack_index,
    })
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
            DrawOrder::new(position.clone(), rank, layer, stack_index as u32),
        );
        if let Some(spread) = spread_cell(position, offset, stack_index as u32) {
            commands.entity(cell).insert(spread);
        }
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
            name: None,
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

    fn corpse_cell(offset: IVec2) -> SpreadCell {
        spread_cell(&Position::new(1000, 1000, 7), offset, 3).unwrap()
    }

    fn cell_key(cell: &SpreadCell, under: Option<&Position>) -> f32 {
        let (pos, layer, slot) = cell.placement(under);
        key(pos, layer, slot)
    }

    fn key(pos: &Position, layer: DrawLayer, slot: u32) -> f32 {
        let origin = crate::map::DrawOrigin::around(&Position::new(1000, 1000, 7));
        DrawOrder::new(pos.clone(), DrawRank::Standing, layer, slot).key(&origin)
    }

    #[test]
    fn a_corpses_own_tile_has_no_spread_cell() {
        assert_eq!(
            spread_cell(&Position::new(1000, 1000, 7), IVec2::ZERO, 3),
            None
        );
    }

    #[test]
    fn a_spread_cell_names_the_tile_it_covers() {
        assert_eq!(
            corpse_cell(IVec2::new(-1, -1)).tile,
            Position::new(999, 999, 7)
        );
        assert_eq!(
            corpse_cell(IVec2::new(0, -1)).tile,
            Position::new(1000, 999, 7)
        );
    }

    /// A 64 px wall on a tile between a cell's and the corpse's reaches back
    /// over the cell. Left alone, the cell must cover it as the corpse's own
    /// tile does, or the wall cuts the corpse in two.
    #[test]
    fn an_undisturbed_cell_draws_over_a_wall_between_it_and_the_corpse() {
        let north_west = corpse_cell(IVec2::new(-1, -1));

        for wall_tile in [Position::new(1000, 999, 7), Position::new(999, 1000, 7)] {
            for layer in [DrawLayer::Bottom, DrawLayer::Top] {
                assert!(key(&wall_tile, layer, 15) < cell_key(&north_west, None));
            }
        }
    }

    #[test]
    fn a_cell_a_creature_stands_on_draws_under_it() {
        let cell = corpse_cell(IVec2::new(-1, -1));
        let under = cell_key(&cell, Some(&cell.tile));

        assert!(under < key(&cell.tile, DrawLayer::Creature, 0));
        assert!(key(&cell.tile, DrawLayer::Items, 15) < under);
    }

    /// The screenshot case: a creature on the corpse's north tile reaches the
    /// north-west cell, and the wall west of the corpse reaches it too. Keyed
    /// to the creature's tile the cell stays over that wall; keyed to its own
    /// it would go under.
    #[test]
    fn a_cell_drops_no_further_than_the_creature_over_it() {
        let cell = corpse_cell(IVec2::new(-1, -1));
        let creature = Position::new(1000, 999, 7);
        let wall = Position::new(999, 1000, 7);
        let under = cell_key(&cell, Some(&creature));

        assert!(cell.is_under(&creature));
        assert!(key(&wall, DrawLayer::Top, 15) < under);
        assert!(under < key(&creature, DrawLayer::Creature, 0));
    }

    #[test]
    fn a_cell_is_under_whatever_is_keyed_to_the_tiles_reaching_it() {
        let cell = corpse_cell(IVec2::new(-1, 0));

        assert!(cell.is_under(&Position::new(999, 1000, 7)));
        assert!(cell.is_under(&Position::new(999, 1001, 7)));

        assert!(
            !cell.is_under(&Position::new(998, 1000, 7)),
            "left of the cell"
        );
        assert!(!cell.is_under(&Position::new(999, 1002, 7)), "out of reach");
        assert!(
            !cell.is_under(&Position::new(999, 1000, 6)),
            "another floor"
        );
    }

    /// From the corpse's own tile on, tile order already draws it over the
    /// whole corpse, and dropping the cell would only hand it to a wall.
    #[test]
    fn a_cell_stays_put_for_what_draws_after_the_corpse_anyway() {
        let cell = corpse_cell(IVec2::new(-1, 0));

        assert!(!cell.is_under(&Position::new(1000, 1000, 7)));
        assert!(!cell.is_under(&Position::new(1000, 1001, 7)));
    }

    #[test]
    fn a_cell_drops_only_while_a_creature_is_over_it() {
        let mut app = App::new();
        app.add_systems(Update, settle_spread_cells);
        let home = Position::new(1000, 1000, 7);
        let spread = corpse_cell(IVec2::new(-1, -1));
        let north = Position::new(1000, 999, 7);
        let at = |pos: &Position, layer| DrawOrder::new(pos.clone(), DrawRank::Standing, layer, 0);
        let with_the_corpse = DrawOrder::new(home, DrawRank::Standing, DrawLayer::Items, 3);
        let cell = app
            .world_mut()
            .spawn((SpawnedItem, spread.clone(), with_the_corpse.clone()))
            .id();
        let creature = app
            .world_mut()
            .spawn(at(&Position::new(990, 990, 7), DrawLayer::Creature))
            .id();
        let order = |app: &App| app.world().get::<DrawOrder>(cell).unwrap().clone();

        app.update();
        assert_eq!(order(&app), with_the_corpse);

        app.world_mut()
            .entity_mut(creature)
            .insert(at(&north, DrawLayer::Creature));
        app.update();
        assert_eq!(order(&app), at(&north, DrawLayer::Lying));

        let nearer = app
            .world_mut()
            .spawn(at(&spread.tile, DrawLayer::Effect))
            .id();
        app.update();
        assert_eq!(
            order(&app),
            at(&spread.tile, DrawLayer::Lying),
            "under the first of the two"
        );

        app.world_mut().despawn(nearer);
        app.world_mut()
            .entity_mut(creature)
            .insert(at(&north, DrawLayer::PassingThrough));
        app.update();
        assert_eq!(order(&app), with_the_corpse);
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
