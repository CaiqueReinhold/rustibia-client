use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::render::storage::ShaderStorageBuffer;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d};

use crate::core::MAX_LAYERS;

#[repr(C)]
#[derive(ShaderType, Clone, Copy, Debug, Default, PartialEq)]
pub struct AgentInstance {
    pub sprite_ids: [u32; MAX_LAYERS],
    pub layer_count: u32,
    pub outfit_colors: u32, // packed: head | body<<8 | legs<<16 | feet<<24 (indices into COLOR_TABLE)
    pub bbox_min: Vec2,
    pub bbox_size: Vec2,
    pub shift: Vec2,
    pub highlighted: u32,
}

#[repr(C)]
#[derive(ShaderType, Clone, Copy, Debug, Default)]
pub struct AgentParams {
    pub atlas_grid: Vec2,
    pub pulse_base: f32,
    pub pulse_amplitude: f32,
    pub pulse_period: f32,
}

#[derive(Asset, AsBindGroup, TypePath, Debug, Clone, Default)]
pub struct AgentMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Handle<Image>,

    #[uniform(2)]
    pub params: AgentParams,

    #[storage(3, read_only)]
    pub instances: Handle<ShaderStorageBuffer>,
}

impl Material2d for AgentMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/agent.wgsl".into()
    }
    fn vertex_shader() -> ShaderRef {
        "shaders/agent.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}
