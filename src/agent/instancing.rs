use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::MeshTag;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::storage::ShaderStorageBuffer;

use crate::agent::components::{Agent, AgentAnimConfigs};
use crate::agent::world_hud::spawn_world_hud;
use crate::agent::{AgentId, FacingDirection, Health, Mana};
use crate::core::OutfitColors;
use crate::core::OutfitId;
use crate::core::{Appearances, InstanceManager, OutfitSprite, SpriteSheet};
use crate::core::{MAX_LAYERS, SpriteAnimator, SpriteConfig};

use crate::agent::{
    material::{AgentInstance, AgentMaterial, AgentParams},
    movement::{MoveQueue, Moving},
};
use crate::map::{DrawLayer, DrawOrder, DrawRank, Map, Position};

#[derive(Resource, Default, Debug)]
pub struct LoadedMaterials {
    materials: HashMap<String, (Handle<Mesh>, Handle<AgentMaterial>)>,
    buffer: Handle<ShaderStorageBuffer>,
}

pub fn init_instances_buffer(
    mut commands: Commands,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
) {
    let loaded_materials = LoadedMaterials {
        materials: HashMap::new(),
        buffer: buffers.add(ShaderStorageBuffer::new(&[0], RenderAssetUsages::all())),
    };
    commands.insert_resource(loaded_materials);
}

pub fn resolve_agent_sprite_ids(
    config: &SpriteConfig,
    phase: u32,
    direction: u32,
    addons: u32,
    mounted: u32,
) -> ([u32; MAX_LAYERS], u32) {
    debug_assert!(
        (config.pattern_y * config.layers) as usize <= MAX_LAYERS,
        "outfit has more layers than MAX_LAYERS={MAX_LAYERS}: pattern_y={} layers={}",
        config.pattern_y,
        config.layers,
    );
    let mut sprite_ids = [0u32; MAX_LAYERS];
    let mut slot = 0usize;

    for addon in 0..config.pattern_y {
        if addon > 0 && (addons & addon) == 0 {
            continue;
        }
        for layer in 0..config.layers {
            if slot >= MAX_LAYERS {
                break;
            }
            let index = (((phase * config.pattern_z + mounted) * config.pattern_y + addon)
                * config.pattern_x
                + direction)
                * config.layers
                + layer;
            // Safe index (mirrors resolve_simple_sprite_ids): a stale/edge phase or
            // pattern value must never panic — fall back to sprite 0.
            sprite_ids[slot] = config.sprite_ids.get(index as usize).copied().unwrap_or(0);
            slot += 1;
        }
    }
    (sprite_ids, slot as u32)
}

pub fn spawn_agent(
    commands: &mut Commands,
    loaded_materials: &mut LoadedMaterials,
    materials: &mut Assets<AgentMaterial>,
    meshes: &mut Assets<Mesh>,
    buffers: &mut Assets<ShaderStorageBuffer>,
    instances: &mut InstanceManager<AgentInstance>,
    font: &Handle<Font>,
    appearances: &Appearances,
    outfit_id: OutfitId,
    map: &Map,
    outfit_colors: OutfitColors,
    facing: FacingDirection,
    speed: u16,
    addons: u8,
    position: Position,
    name: String,
    health: Option<Health>,
    mana: Option<Mana>,
    agent_id: AgentId,
) -> Option<Entity> {
    // `None` means the server sent an outfit this client's assets don't have.
    // Callers raise `ClientOutdated`; there is no sensible stand-in sprite.
    let outfit = appearances.get_outfit(outfit_id)?;
    let sheet = appearances.get_sheet(&outfit.still_sprite.group);

    if !loaded_materials
        .materials
        .contains_key(&outfit.still_sprite.group)
    {
        init_material(outfit, sheet, materials, meshes, buffers, loaded_materials);
    }

    let (mesh, material) = loaded_materials
        .materials
        .get(&outfit.still_sprite.group)
        .unwrap();

    let agent = Agent {
        agent_id,
        direction: facing,
        addons,
        outfit_colors,
        speed,
        boxes: [
            outfit.still_sprite.boxes.clone().try_into().unwrap(),
            outfit.moving_sprite.boxes.clone().try_into().unwrap(),
        ],
        shift: outfit.still_sprite.shift,
        ..default()
    };

    let index = instances.alloc_index();
    let instance = instances.get_mut(index);
    let (sprite_ids, layer_count) =
        resolve_agent_sprite_ids(&outfit.still_sprite, 0, facing as u32, addons as u32, 0);
    instance.sprite_ids = sprite_ids;
    instance.layer_count = layer_count;
    instance.outfit_colors = outfit_colors.packed();
    let bbox = &outfit.still_sprite.boxes[facing as usize];
    instance.bbox_min = bbox.min;
    instance.bbox_size = bbox.max;
    instance.shift = outfit.still_sprite.shift;

    let world_position = position.to_world();
    let elevation = map.get_elevation(&position) as f32;
    let entity = commands
        .spawn((
            agent,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            MeshTag(index),
            DrawOrder::new(position.clone(), DrawRank::Standing, DrawLayer::Creature, 0),
            position,
            // z is left at zero and filled in by `map::draw_order`, which owns
            // every game-world z in the client.
            Transform::from_xyz(
                world_position.x - elevation,
                world_position.y + elevation,
                0.0,
            ),
            SpriteAnimator::new(Arc::clone(&outfit.still_sprite), facing as u32, 0, 0),
            AgentAnimConfigs {
                still: Arc::clone(&outfit.still_sprite),
                moving: Arc::clone(&outfit.moving_sprite),
            },
            MoveQueue::default(),
        ))
        .id();

    if let Some(health) = &health {
        commands.entity(entity).insert(health.clone());
    }

    if let Some(mana) = &mana {
        commands.entity(entity).insert(mana.clone());
    }

    let world_y_offset =
        outfit.still_sprite.boxes[0].max.y / 2.0 + outfit.still_sprite.shift.y + 5.0;
    let hud = spawn_world_hud(
        commands,
        entity,
        font,
        &name,
        health.as_ref(),
        mana.as_ref(),
        world_y_offset,
    );
    commands.entity(entity).insert(hud);

    Some(entity)
}

fn init_material(
    outfit: &OutfitSprite,
    sheet: &SpriteSheet,
    materials: &mut Assets<AgentMaterial>,
    meshes: &mut Assets<Mesh>,
    _buffers: &mut Assets<ShaderStorageBuffer>,
    loaded_materials: &mut LoadedMaterials,
) {
    let params = AgentParams {
        atlas_grid: sheet.grid_size,
    };
    let material_handle = materials.add(AgentMaterial {
        texture: sheet.texture().clone(),
        params,
        instances: loaded_materials.buffer.clone(),
    });
    let mesh_handle = meshes.add(Mesh::from(Rectangle::new(64.0, 64.0)));
    loaded_materials.materials.insert(
        outfit.still_sprite.group.clone(),
        (mesh_handle, material_handle),
    );
}

pub fn upload_instance_buffer(
    mut instances: ResMut<InstanceManager<AgentInstance>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    loaded_materials: Res<LoadedMaterials>,
    mut materials: ResMut<Assets<AgentMaterial>>,
) {
    if !instances.is_dirty() {
        return;
    }

    let Some(ssb) = buffers.get_mut(&loaded_materials.buffer) else {
        return;
    };
    ssb.set_data(instances.get_buffer_data());
    instances.reset_dirty();

    // Marking the materials changed re-prepares their bind groups against the
    // buffer just written. It only belongs on frames that actually uploaded —
    // outside this branch it re-prepared every material every frame.
    for (_, mat) in loaded_materials.materials.values() {
        let _ = materials.get_mut(mat);
    }
}

pub fn set_agent_animation_state(
    mut agents_q: Query<
        (
            &Agent,
            &mut SpriteAnimator,
            Option<&Moving>,
            &AgentAnimConfigs,
        ),
        Or<(Changed<Agent>, Changed<Moving>)>,
    >,
) {
    for (agent, mut animator, moving, configs) in &mut agents_q {
        let new_config = if moving.is_some() {
            Arc::clone(&configs.moving)
        } else {
            Arc::clone(&configs.still)
        };
        if !Arc::ptr_eq(&animator.config, &new_config) {
            animator.config = new_config;
            animator.current_phase = 0;
            animator.timer.reset();
            animator.moving_animation = moving.is_some();
        }
        animator.pattern_x = agent.direction as u32;
        animator.pattern_z = agent.mounted as u32;

        if let Some(moving) = moving {
            let phase_count = configs.moving.animation.total_animation_phases().max(1);
            // timer.fraction() reaches exactly 1.0 on the frame the move completes, which
            // would make current_phase == phase_count (one past the last valid frame) and
            // push resolve_agent_sprite_ids off the end of config.sprite_ids. Clamp it.
            animator.current_phase =
                ((moving.timer.fraction() * phase_count as f32) as u32).min(phase_count - 1);
        }
    }
}

pub fn update_agent_instances(
    agents_q: Query<
        (&Agent, &SpriteAnimator, &MeshTag, Option<&Moving>),
        Or<(Changed<SpriteAnimator>, Changed<Agent>, Changed<Moving>)>,
    >,
    mut instances: ResMut<InstanceManager<AgentInstance>>,
) {
    for (agent, animator, tag, moving) in &agents_q {
        let (sprite_ids, layer_count) = resolve_agent_sprite_ids(
            &animator.config,
            animator.current_phase,
            agent.direction as u32,
            agent.addons as u32,
            agent.mounted as u32,
        );
        let is_moving = moving.is_some() as usize;
        let bbox = &agent.boxes[is_moving][agent.direction as usize];

        instances.update(tag.0, |instance| {
            instance.sprite_ids = sprite_ids;
            instance.layer_count = layer_count;
            instance.outfit_colors = agent.outfit_colors.packed();
            instance.bbox_min = bbox.min;
            instance.bbox_size = bbox.max;
            instance.shift = agent.shift;
        });
    }
}
