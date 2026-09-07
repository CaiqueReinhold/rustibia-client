use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::MeshTag;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderStorageBuffer;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};

use crate::conf::effects::STATIC_DURATION;
use crate::core::sprite::{AnimationLoop, SpriteAnimation, SpriteConfig};
use crate::core::{Appearances, InstanceManager, SpriteAnimator, SpriteSheet};
use crate::map::FloorEntities;
use crate::map::Position;
use crate::map::{DrawLayer, DrawOrder, DrawRank};
use crate::network::events::ShowEffect;

/// One effect's slot in the shader storage buffer.
///
/// Byte-identical to `ItemInstance`: both describe a single-layer atlas sprite
/// with a bounding box and a shift, which is all `shaders/items.wgsl` consumes.
#[repr(C)]
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct EffectInstance {
    pub sprite_id: u32,
    pub _pad: u32, // required std430 alignment padding before vec2
    pub bbox_min: Vec2,
    pub bbox_size: Vec2,
    pub shift: Vec2,
}

/// A separate type from `ItemMaterial` despite pointing at the same shader: a
/// second `Material2dPlugin` registration for one type panics.
#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct EffectMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,

    #[uniform(2)]
    pub atlas_grid: Vec2,

    #[storage(3, read_only)]
    pub instances: Handle<ShaderStorageBuffer>,

    #[uniform(4)]
    pub mesh_size: Vec2,
}

impl Material2d for EffectMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/items.wgsl".into()
    }
    fn vertex_shader() -> ShaderRef {
        "shaders/items.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Resource, Debug, Default)]
pub struct EffectMaterials {
    pub(super) by_group: HashMap<String, (Handle<Mesh>, Handle<EffectMaterial>)>,
    pub(super) buffer: Handle<ShaderStorageBuffer>,
}

pub fn setup_resources(mut commands: Commands, mut buffers: ResMut<Assets<ShaderStorageBuffer>>) {
    commands.insert_resource(EffectMaterials {
        by_group: HashMap::new(),
        buffer: buffers.add(ShaderStorageBuffer::new(&[0], RenderAssetUsages::all())),
    });
}

/// A live effect. Parented to its floor entity, which is what makes floor
/// occlusion apply to it without this module knowing how occlusion works.
#[derive(Component)]
pub struct Effect {
    /// `None` when the animation ends itself — the animator is the authority.
    /// `Some` for the 16 effects whose loop mode never would.
    ttl: Option<Timer>,
}

pub(super) fn init_material(
    group: &str,
    sheet: &SpriteSheet,
    materials: &mut Assets<EffectMaterial>,
    meshes: &mut Assets<Mesh>,
    effect_materials: &mut EffectMaterials,
) {
    let material = materials.add(EffectMaterial {
        texture: sheet.texture().clone(),
        atlas_grid: sheet.grid_size,
        mesh_size: sheet.sprite_size,
        instances: effect_materials.buffer.clone(),
    });
    let mesh = meshes.add(Mesh::from(Rectangle::new(
        sheet.sprite_size.x,
        sheet.sprite_size.y,
    )));
    effect_materials
        .by_group
        .insert(group.to_string(), (mesh, material));
}

/// Everything about an instance except its sprite id, which needs an animator
/// that does not exist yet.
///
/// `boxes` is indexed by `pattern_x` alone, not per phase: effect 41 is 2x2 with
/// 2 boxes, effect 1 has 6 phases and 1 box.
pub(super) fn init_instance(
    instance: &mut EffectInstance,
    sprite: &SpriteConfig,
    sprite_size: Vec2,
    pattern_x: u32,
) {
    instance.shift = sprite.shift;
    // A box's `.max` holds a SIZE, not a maximum corner — `read_sprite_config`
    // parses `[x, y, w, h]` into `Rect { min, max }` and the shader consumes
    // the second pair as an extent. Do not "correct" this to `max - min`; it
    // shrinks every effect.
    match sprite.boxes.get(pattern_x as usize) {
        Some(bbox) => {
            instance.bbox_min = bbox.min;
            instance.bbox_size = bbox.max;
        }
        None => {
            instance.bbox_min = Vec2::ZERO;
            instance.bbox_size = sprite_size;
        }
    }
}

pub fn on_show_effect(
    event: On<ShowEffect>,
    mut commands: Commands,
    mut instances: ResMut<InstanceManager<EffectInstance>>,
    mut effect_materials: ResMut<EffectMaterials>,
    mut materials: ResMut<Assets<EffectMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    appearances: Res<Appearances>,
    floors: Res<FloorEntities>,
) {
    // A missing effect is cosmetic, so this warns where `get_outfit`'s
    // equivalent gap raises `ClientOutdated` and ends the session.
    let Some(sprite) = appearances.get_effect(event.effect_id) else {
        warn!(
            "server sent effect {:?}, which this client's assets do not have",
            event.effect_id
        );
        return;
    };
    let sprite = Arc::clone(sprite);
    let sheet = appearances.get_sheet(&sprite.group);
    let sprite_size = sheet.sprite_size;

    if !effect_materials.by_group.contains_key(&sprite.group) {
        init_material(
            &sprite.group,
            sheet,
            &mut materials,
            &mut meshes,
            &mut effect_materials,
        );
    }
    let (mesh, material) = effect_materials.by_group[&sprite.group].clone();

    // Computed once per message, not once per tile: `pass_duration` samples a
    // `NonUniform` animation's phases with `fastrand`, so rolling it inside the
    // loop below would let one area effect's tiles die at independently
    // sampled moments instead of all together.
    let ttl = lifetime(&sprite.animation);

    for tile in effect_tiles(&event.position, &event.delta) {
        let (pattern_x, pattern_y) = pattern_for(&tile, &sprite);

        let index = instances.alloc_index();
        let instance = instances.get_mut(index);
        init_instance(instance, &sprite, sprite_size, pattern_x);

        let animator = SpriteAnimator::new(Arc::clone(&sprite), pattern_x, pattern_y, 0);
        instance.sprite_id = animator.current_sprite_ids[0];

        let entity = commands
            .spawn((
                Effect {
                    ttl: ttl.map(|d| Timer::new(d, TimerMode::Once)),
                },
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
                MeshTag(index),
                Transform::from_translation(anchor(tile.to_world(), sprite_size)),
                DrawOrder::new(tile.clone(), DrawRank::Standing, DrawLayer::Effect, 0),
                Visibility::Inherited,
                animator,
            ))
            .id();
        commands
            .entity(floors.floors[tile.z as usize])
            .add_child(entity);
    }
}

/// Every tile the message paints. The wire carries no floor per tile, so an
/// area effect is flat by construction.
fn effect_tiles(base: &Position, delta: &[(i8, i8)]) -> Vec<Position> {
    let mut tiles = Vec::with_capacity(delta.len() + 1);
    tiles.push(base.clone());
    tiles.extend(
        delta
            .iter()
            .map(|(dx, dy)| base.delta(*dx as i32, *dy as i32)),
    );
    tiles
}

/// The pattern a tile draws, taken from its own absolute coordinates — which is
/// what tiles a multi-cell effect seamlessly across an area, and what keeps it
/// stable per tile so a repeat does not shimmer.
fn pattern_for(position: &Position, sprite: &SpriteConfig) -> (u32, u32) {
    (
        position.x as u32 % sprite.pattern_x.max(1),
        position.y as u32 % sprite.pattern_y.max(1),
    )
}

/// `Position::to_world` returns the tile's TOP-LEFT CORNER. A 64 px quad centres
/// on that corner correctly — large Tibia sprites extend up and to the left of
/// their tile — and a 32 px one has to be nudged half a tile down and right. The
/// rule is per axis because two effect sheet groups are 32x64 and 64x32.
pub(super) fn anchor(world: Vec3, sprite_size: Vec2) -> Vec3 {
    let half_tile_x = if sprite_size.x <= 32.0 { 16.0 } else { 0.0 };
    let half_tile_y = if sprite_size.y <= 32.0 { -16.0 } else { 0.0 };
    Vec3::new(world.x + half_tile_x, world.y + half_tile_y, 0.0)
}

/// How long an effect lives, or `None` when its own animation ends it.
///
/// `Counted` returns `None` because `count` is a number of RUNS, not phases:
/// effect 77 runs 402 times over a 1490 ms pass, so a rule expressed as a single
/// pass would truncate it 400-fold. See the vault's `effects-rendering` for what
/// that costs.
///
/// The `never_advances` arm has to come first: `SpriteAnimation::Static` reports
/// `AnimationLoop::Infinite`, so matching on the loop mode alone gives a static
/// effect a zero-length pass.
fn lifetime(animation: &SpriteAnimation) -> Option<Duration> {
    if animation.never_advances() {
        return Some(STATIC_DURATION);
    }
    match animation.loop_mode() {
        AnimationLoop::Counted { .. } => None,
        AnimationLoop::Infinite | AnimationLoop::PingPong => Some(animation.pass_duration()),
    }
}

/// Must run `.after(AnimationSet)`: `SpriteAnimator` sets `finished` only once
/// the last phase has had its full time on screen, so running earlier holds
/// every effect a frame past its end.
pub fn despawn_finished_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut effects: Query<(Entity, &mut Effect, &SpriteAnimator)>,
) {
    for (entity, mut effect, animator) in &mut effects {
        let done = match effect.ttl.as_mut() {
            Some(timer) => timer.tick(time.delta()).just_finished(),
            None => animator.is_finished(),
        };
        if done {
            commands.entity(entity).despawn();
        }
    }
}

pub fn on_remove_effect(
    event: On<Remove, Effect>,
    tags: Query<&MeshTag, With<Effect>>,
    mut instances: ResMut<InstanceManager<EffectInstance>>,
) {
    let Ok(tag) = tags.get(event.entity) else {
        return;
    };

    instances.dealloc_index(tag.0);
}

/// Mandatory, unlike floating text: effects hang off the floor entities, which
/// are `Startup`-spawned and outlive the session, so nothing else collects them.
pub(super) fn cleanup_session(mut commands: Commands, effects: Query<Entity, With<Effect>>) {
    for entity in &effects {
        commands.entity(entity).despawn();
    }
    commands.insert_resource(InstanceManager::<EffectInstance>::default());
}

/// The `Changed<SpriteAnimator>` filter only gates because `tick_sprite_animators`
/// writes through `bypass_change_detection` and flags a change on a real phase
/// advance and nothing else.
pub fn update_effect_instances(
    effects: Query<(&SpriteAnimator, &MeshTag), (With<Effect>, Changed<SpriteAnimator>)>,
    mut instances: ResMut<InstanceManager<EffectInstance>>,
) {
    for (animator, tag) in &effects {
        instances.update(tag.0, |instance| {
            instance.sprite_id = animator.current_sprite_ids[0];
        });
    }
}

pub fn upload_effect_buffer(
    mut instances: ResMut<InstanceManager<EffectInstance>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    effect_materials: Res<EffectMaterials>,
    mut materials: ResMut<Assets<EffectMaterial>>,
) {
    if !instances.is_dirty() {
        return;
    }

    let Some(ssb) = buffers.get_mut(&effect_materials.buffer) else {
        return;
    };
    ssb.set_data(instances.get_buffer_data());
    instances.reset_dirty();

    // Touching each material marks it changed, which is what makes the render
    // world re-extract the bind group pointing at the buffer just rewritten.
    for (_, material) in effect_materials.by_group.values() {
        let _ = materials.get_mut(material);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sprite::{AnimationLoop, SpriteAnimation, SpriteConfig};
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    fn config(pattern_x: u32, pattern_y: u32) -> SpriteConfig {
        SpriteConfig {
            id: 1,
            group: "effect-32-32-0".to_string(),
            pattern_x,
            pattern_y,
            pattern_z: 1,
            layers: 1,
            sprite_ids: vec![0],
            animation: SpriteAnimation::Static,
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        }
    }

    /// An empty delta is what the server sends today, and it must mean "just
    /// this tile" rather than "no tiles".
    #[test]
    fn an_empty_delta_paints_only_the_base_tile() {
        let tiles = effect_tiles(&Position::new(100, 200, 7), &[]);

        assert_eq!(tiles, vec![Position::new(100, 200, 7)]);
    }

    /// Deltas are relative to the base tile, and the base tile plays too. The
    /// wire carries no floor per tile, so an area effect is flat.
    #[test]
    fn deltas_paint_extra_tiles_around_the_base() {
        let tiles = effect_tiles(&Position::new(100, 200, 7), &[(1, 0), (0, -1), (-1, -1)]);

        assert_eq!(
            tiles,
            vec![
                Position::new(100, 200, 7),
                Position::new(101, 200, 7),
                Position::new(100, 199, 7),
                Position::new(99, 199, 7),
            ]
        );
    }

    /// 196 of the 207 effects are 1x1 and must always land on pattern zero.
    #[test]
    fn a_single_pattern_effect_always_draws_pattern_zero() {
        let sprite = config(1, 1);

        assert_eq!(pattern_for(&Position::new(100, 200, 7), &sprite), (0, 0));
        assert_eq!(pattern_for(&Position::new(101, 201, 7), &sprite), (0, 0));
    }

    /// The eleven patterned effects tile across an area by absolute coordinate,
    /// which is what makes a 3x3 field seamless instead of nine copies of one
    /// corner.
    #[test]
    fn a_patterned_effect_walks_its_patterns_across_adjacent_tiles() {
        let sprite = config(3, 3);

        let row: Vec<(u32, u32)> = (99..102)
            .map(|x| pattern_for(&Position::new(x, 200, 7), &sprite))
            .collect();

        assert_eq!(row, vec![(0, 2), (1, 2), (2, 2)]);
    }

    /// Stability matters: a repeat of the same area effect must not shimmer.
    /// `1028 % 3 == 2` and `1029 % 3 == 0`, so the same tile always gets `(2, 0)`
    /// — not just "the same answer as itself", which every deterministic
    /// function trivially gives.
    #[test]
    fn a_tiles_pattern_does_not_change_between_casts() {
        let sprite = config(3, 3);
        let position = Position::new(1028, 1029, 7);

        assert_eq!(pattern_for(&position, &sprite), (2, 0));
    }

    /// `pattern_x` and `pattern_y` are indistinguishable on a square pattern —
    /// every other case here uses `config(1, 1)` or `config(3, 3)`. A 2x4
    /// pattern catches a divisor swapped onto the wrong axis: `5 % 2 = 1` and
    /// `7 % 4 = 3`, so a swap would report `(1, 1)` instead.
    #[test]
    fn a_rectangular_pattern_uses_the_matching_divisor_per_axis() {
        let sprite = config(2, 4);

        assert_eq!(pattern_for(&Position::new(5, 7, 0), &sprite), (1, 3));
    }

    /// `read_sprite_config` never validates `pattern_x`/`pattern_y`, so a zero
    /// dimension is not provably impossible — and without `.max(1)` it would
    /// panic on a modulo by zero rather than degrade to pattern zero.
    #[test]
    fn a_zero_pattern_dimension_does_not_panic() {
        let sprite = config(0, 0);

        assert_eq!(pattern_for(&Position::new(5, 7, 0), &sprite), (0, 0));
    }

    /// `to_world` returns the tile's TOP-LEFT CORNER. A 32 px quad is centred on
    /// its transform, so it must be nudged half a tile down and right to cover
    /// the tile it belongs to.
    #[test]
    fn a_32px_sprite_is_nudged_onto_its_tile() {
        let placed = anchor(Vec3::new(100.0, 200.0, 0.0), Vec2::new(32.0, 32.0));

        assert_eq!(placed.x, 116.0);
        assert_eq!(placed.y, 184.0);
    }

    /// A 64 px sprite centres on the corner correctly — large Tibia sprites
    /// extend up and to the left of the tile they occupy.
    #[test]
    fn a_64px_sprite_keeps_the_tile_corner() {
        let placed = anchor(Vec3::new(100.0, 200.0, 0.0), Vec2::new(64.0, 64.0));

        assert_eq!(placed.x, 100.0);
        assert_eq!(placed.y, 200.0);
    }

    /// Two effect sheet groups are 32x64 and 64x32, so the correction has to be
    /// decided per axis. Applying it to both, or neither, puts them half a tile
    /// out on one axis.
    #[test]
    fn a_mixed_size_sprite_is_corrected_on_one_axis_only() {
        let placed = anchor(Vec3::new(100.0, 200.0, 0.0), Vec2::new(32.0, 64.0));

        assert_eq!(placed.x, 116.0, "32 px wide, so nudged");
        assert_eq!(placed.y, 200.0, "64 px tall, so not");
    }

    /// A box's `.max` holds a SIZE, not a maximum corner — `read_sprite_config`
    /// parses `[x, y, w, h]` into `Rect { min, max }` and the shader consumes
    /// the second pair as an extent. This is the test that catches a `max -
    /// min` "correction": with `min = (4, 6)` and `max = (24, 20)`, that
    /// mutation would report `(20, 14)` instead of `(24, 20)`.
    #[test]
    fn init_instance_with_a_box_uses_its_max_as_the_size_verbatim() {
        let mut sprite = config(1, 1);
        sprite.boxes = vec![Rect {
            min: Vec2::new(4.0, 6.0),
            max: Vec2::new(24.0, 20.0),
        }];
        sprite.shift = Vec2::new(1.0, 2.0);
        let mut instance = EffectInstance::default();

        init_instance(&mut instance, &sprite, Vec2::new(32.0, 32.0), 0);

        assert_eq!(instance.bbox_min, Vec2::new(4.0, 6.0));
        assert_eq!(instance.bbox_size, Vec2::new(24.0, 20.0));
        assert_eq!(instance.shift, Vec2::new(1.0, 2.0));
    }

    /// A sheet group's sprite size is the fallback, not a hardcoded 32x32 —
    /// unlike `items::instancing::init_instance`, which does hardcode it and
    /// would clip a boxless effect on a 64x64 sheet.
    #[test]
    fn init_instance_without_a_box_falls_back_to_the_sheets_sprite_size() {
        let sprite = config(1, 1); // boxes: Vec::new()
        let mut instance = EffectInstance::default();

        init_instance(&mut instance, &sprite, Vec2::new(64.0, 64.0), 0);

        assert_eq!(instance.bbox_min, Vec2::ZERO);
        assert_eq!(instance.bbox_size, Vec2::new(64.0, 64.0));
    }

    /// Effects draw over creatures and under top items, on their own tile.
    #[test]
    fn an_effect_sits_between_the_agent_and_the_top_item_planes() {
        use crate::map::DrawOrigin;

        let tile = Position::new(1000, 1000, 7);
        let origin = DrawOrigin::around(&tile);
        let effect =
            DrawOrder::new(tile.clone(), DrawRank::Standing, DrawLayer::Effect, 0).key(&origin);

        assert!(
            effect
                > DrawOrder::new(tile.clone(), DrawRank::Standing, DrawLayer::Creature, 0)
                    .key(&origin)
        );
        assert!(
            effect
                < DrawOrder::new(tile.clone(), DrawRank::Standing, DrawLayer::Top, 0).key(&origin)
        );
        // And `anchor` contributes nothing to it.
        assert_eq!(
            anchor(tile.to_world(), Vec2::new(32.0, 32.0)).z,
            0.0,
            "placement is x/y only; draw order is the DrawOrder component"
        );
    }

    /// 191 of the 207 effects are counted, and `count` is a number of RUNS, not
    /// phases: effect 77 runs 402 times over a 1490 ms pass, about ten minutes
    /// total. Only the animator knows a pass's real length, so it stays the
    /// authority — the entity and its instance slot are held for that whole
    /// run, unreachable today because the server only ever sends effects 1, 3
    /// and 4.
    #[test]
    fn a_counted_effect_defers_to_its_animator() {
        let animation = SpriteAnimation::Uniform {
            loop_mode: AnimationLoop::Counted { count: 402 },
            phase_count: 2,
            phase_duration: Duration::from_millis(4),
        };

        assert_eq!(lifetime(&animation), None);
    }

    /// The 13 infinite effects would otherwise never be despawned.
    #[test]
    fn an_endless_effect_lives_exactly_one_pass() {
        let infinite = SpriteAnimation::Uniform {
            loop_mode: AnimationLoop::Infinite,
            phase_count: 8,
            phase_duration: Duration::from_millis(100),
        };
        let pingpong = SpriteAnimation::Uniform {
            loop_mode: AnimationLoop::PingPong,
            phase_count: 4,
            phase_duration: Duration::from_millis(50),
        };

        assert_eq!(lifetime(&infinite), Some(Duration::from_millis(800)));
        assert_eq!(lifetime(&pingpong), Some(Duration::from_millis(200)));
    }

    /// Effects 200, 211 and 212 have no animation at all. `loop_mode()` reports
    /// `Infinite` for a static animation, so a lifetime rule written on the loop
    /// mode alone would give them a zero-length pass and flash them for one
    /// frame.
    #[test]
    fn a_static_effect_gets_the_fixed_duration() {
        assert_eq!(lifetime(&SpriteAnimation::Static), Some(STATIC_DURATION));
    }

    /// A counted animation, 2 phases of 100 ms, run once: finished after 200 ms.
    fn counted_animator() -> SpriteAnimator {
        SpriteAnimator::new(
            Arc::new(SpriteConfig {
                animation: SpriteAnimation::Uniform {
                    loop_mode: AnimationLoop::Counted { count: 1 },
                    phase_count: 2,
                    phase_duration: Duration::from_millis(100),
                },
                sprite_ids: vec![10, 20],
                ..config(1, 1)
            }),
            0,
            0,
            0,
        )
    }

    fn advance_and_run(world: &mut World, delta: Duration) {
        world.resource_mut::<Time<()>>().advance_by(delta);
        world
            .run_system_once(crate::core::tick_sprite_animators)
            .unwrap();
        world.run_system_once(despawn_finished_effects).unwrap();
    }

    fn world_with_time() -> World {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.init_resource::<InstanceManager<EffectInstance>>();
        world
    }

    /// 191 of the 207 effects end this way. The animator sets `finished` only
    /// after the last phase has had its full time on screen, so despawning in
    /// the same frame cuts nothing short.
    #[test]
    fn a_counted_effect_is_despawned_when_its_animation_finishes() {
        let mut world = world_with_time();
        let entity = world.spawn((Effect { ttl: None }, counted_animator())).id();

        advance_and_run(&mut world, Duration::from_millis(100));
        assert!(world.get_entity(entity).is_ok(), "one phase still to run");

        advance_and_run(&mut world, Duration::from_millis(100));
        assert!(world.get_entity(entity).is_err());
    }

    /// The 16 that never finish on their own. Their animator keeps looping, so
    /// nothing but the timer will ever collect them.
    #[test]
    fn an_effect_with_a_ttl_is_despawned_when_it_expires() {
        let mut world = world_with_time();
        let entity = world
            .spawn((
                Effect {
                    ttl: Some(Timer::new(Duration::from_millis(300), TimerMode::Once)),
                },
                counted_animator(),
            ))
            .id();

        advance_and_run(&mut world, Duration::from_millis(299));
        assert!(world.get_entity(entity).is_ok());

        advance_and_run(&mut world, Duration::from_millis(1));
        assert!(world.get_entity(entity).is_err());
    }

    /// A ttl is the authority for the effects that have one: an animator that
    /// happens to finish first must not cut them short.
    #[test]
    fn a_ttl_outranks_a_finished_animator() {
        let mut world = world_with_time();
        let entity = world
            .spawn((
                Effect {
                    ttl: Some(Timer::new(Duration::from_secs(5), TimerMode::Once)),
                },
                counted_animator(),
            ))
            .id();

        // Two frames of 100 ms, because the animator advances at most one phase
        // per frame — after these its `is_finished()` is true.
        advance_and_run(&mut world, Duration::from_millis(100));
        advance_and_run(&mut world, Duration::from_millis(100));

        assert!(
            world.get_entity(entity).is_ok(),
            "the ttl decides, not the animator"
        );
    }

    /// `items/instancing.rs` has no test module at all, and this batch copied
    /// that gap along with the code it modelled: `update_effect_instances` and
    /// `upload_effect_buffer` had zero coverage. A no-op update body, a wrong
    /// layer index, a missing `reset_dirty`, and a missing `!is_dirty()`
    /// short-circuit all survived the full suite before this test existed. (A
    /// missing material-touch loop still does -- that only matters to the
    /// render world's bind-group extraction, which nothing in this crate's
    /// unit tests observes.)
    #[test]
    fn ticking_an_effect_moves_its_sprite_through_the_uploaded_buffer() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.init_resource::<InstanceManager<EffectInstance>>();
        world.init_resource::<Assets<ShaderStorageBuffer>>();
        let buffer = world
            .resource_mut::<Assets<ShaderStorageBuffer>>()
            .add(ShaderStorageBuffer::new(&[0], RenderAssetUsages::all()));
        let buffer_handle = buffer.clone();
        world.insert_resource(EffectMaterials {
            by_group: HashMap::new(),
            buffer,
        });
        world.init_resource::<Assets<EffectMaterial>>();

        let index = world
            .resource_mut::<InstanceManager<EffectInstance>>()
            .alloc_index();
        world.spawn((Effect { ttl: None }, counted_animator(), MeshTag(index)));

        // The first upload is spawn-time bookkeeping, not part of what this test
        // exercises: it clears the dirty flag `alloc_index` set and establishes
        // the buffer's starting value (0, EffectInstance's default) before any
        // animator tick has run.
        world.run_system_once(upload_effect_buffer).unwrap();
        let before = world
            .resource::<InstanceManager<EffectInstance>>()
            .get_buffer_data()[index as usize]
            .sprite_id;

        let run = |world: &mut World, delta: Duration| {
            world.resource_mut::<Time<()>>().advance_by(delta);
            world
                .run_system_once(crate::core::tick_sprite_animators)
                .unwrap();
            world.run_system_once(update_effect_instances).unwrap();
            world.run_system_once(upload_effect_buffer).unwrap();
        };
        // Two 100 ms phase boundaries: sprite 10 -> phase 1's sprite 20.
        run(&mut world, Duration::from_millis(100));
        run(&mut world, Duration::from_millis(100));

        let after = world
            .resource::<InstanceManager<EffectInstance>>()
            .get_buffer_data()[index as usize]
            .sprite_id;
        assert_ne!(
            before, after,
            "the buffer slot never picked up the phase advance"
        );
        assert_eq!(
            after, 20,
            "must land on phase 1's sprite, not stay on phase 0's"
        );
        assert!(
            !world
                .resource::<InstanceManager<EffectInstance>>()
                .is_dirty(),
            "upload_effect_buffer must clear dirty once it has written the buffer"
        );

        // Prove the `!is_dirty()` guard actually short-circuits, not just that
        // dirty ends up false: poison the SSBO directly (bypassing the
        // manager, so `is_dirty()` stays false), then call the system again.
        // A version that re-uploads unconditionally would silently overwrite
        // the poison with the manager's (unchanged) data -- same final bytes
        // as leaving it alone, so nothing upstream of this would notice.
        world
            .resource_mut::<Assets<ShaderStorageBuffer>>()
            .get_mut(&buffer_handle)
            .unwrap()
            .data = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        world.run_system_once(upload_effect_buffer).unwrap();

        assert_eq!(
            world
                .resource::<Assets<ShaderStorageBuffer>>()
                .get(&buffer_handle)
                .unwrap()
                .data,
            Some(vec![0xAA, 0xBB, 0xCC, 0xDD]),
            "upload_effect_buffer touched the SSBO while the instance manager was clean"
        );
    }

    /// Without this a long session leaks the instance buffer: every effect ever
    /// played would hold its slot for ever.
    #[test]
    fn despawning_an_effect_frees_its_instance_slot() {
        let mut world = World::new();
        world.init_resource::<InstanceManager<EffectInstance>>();
        world.add_observer(on_remove_effect);
        let index = world
            .resource_mut::<InstanceManager<EffectInstance>>()
            .alloc_index();
        let entity = world.spawn((Effect { ttl: None }, MeshTag(index))).id();

        world.despawn(entity);

        assert_eq!(
            world
                .resource_mut::<InstanceManager<EffectInstance>>()
                .alloc_index(),
            index,
            "the freed slot must be handed out again"
        );
    }

    /// The floor entities an effect hangs off are spawned at `Startup` and
    /// outlive the session. Without this, an effect mid-animation at logout is
    /// still hanging over the map in the next session.
    ///
    /// `on_remove_effect` is registered here deliberately: it is what makes
    /// `cleanup_session`'s statement order load-bearing. Swap the despawn and
    /// the `insert_resource` and the observer pushes the old index onto the
    /// *fresh* manager's free list — `alloc_index` then hands that index out
    /// while `data` is still empty, and the next session's first effect
    /// indexes out of bounds in `get_mut`. Without the observer registered
    /// that swap is invisible to this test.
    #[test]
    fn cleanup_despawns_every_effect_and_resets_the_buffer() {
        let mut world = World::new();
        world.init_resource::<InstanceManager<EffectInstance>>();
        world.add_observer(on_remove_effect);
        let index = world
            .resource_mut::<InstanceManager<EffectInstance>>()
            .alloc_index();
        let entity = world.spawn((Effect { ttl: None }, MeshTag(index))).id();

        world.run_system_once(cleanup_session).unwrap();

        assert!(world.get_entity(entity).is_err());
        assert!(
            !world
                .resource::<InstanceManager<EffectInstance>>()
                .is_dirty(),
            "a fresh manager, not the old one with a stale dirty flag"
        );
        assert_eq!(
            world
                .resource::<InstanceManager<EffectInstance>>()
                .get_buffer_data()
                .len(),
            0,
            "the previous session's slots must not survive"
        );

        // The fresh manager's free list must be empty too: a leftover entry
        // from the old manager would hand out an index into a buffer that
        // has not grown to cover it yet.
        let mut manager = world.resource_mut::<InstanceManager<EffectInstance>>();
        assert_eq!(manager.alloc_index(), 0);
        assert_eq!(
            manager.get_buffer_data().len(),
            1,
            "the fresh manager's free list must be empty"
        );
    }
}
