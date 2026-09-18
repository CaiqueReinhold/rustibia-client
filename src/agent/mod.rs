use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

use crate::core::{AnimationSet, GameState, InstanceManager, SessionCleanup};

// mod colors;
mod components;
mod events;
mod hud;
mod instancing;
mod material;
pub mod movement;
mod session;
mod world_hud;

pub use crate::agent::components::*;
pub use crate::agent::instancing::{LoadedMaterials, spawn_agent};
pub use crate::agent::material::{AgentInstance, AgentMaterial};
pub use crate::agent::movement::{MoveQueue, Moving, StartAgentMove, UpdateElevation};

pub struct AgentPlugin;

impl Plugin for AgentPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<material::AgentMaterial>::default())
            .init_resource::<InstanceManager<AgentInstance>>()
            .add_systems(Startup, instancing::init_instances_buffer)
            .add_systems(
                Update,
                (
                    movement::move_agent,
                    movement::process_agent_move_queues,
                    movement::teleport_agents,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                instancing::set_agent_animation_state.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                instancing::update_agent_instances
                    .after(AnimationSet)
                    .after(instancing::set_agent_animation_state)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PostUpdate,
                instancing::upload_instance_buffer.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PostUpdate,
                movement::sync_agent_draw_order
                    .before(crate::map::DrawOrderSet)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    hud::update_display_name_health_state,
                    hud::update_display_name_color,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    hud::update_hud_bar_ratios,
                    hud::update_hud_bar_health_state.after(hud::update_hud_bar_ratios),
                    hud::update_hud_bar_colors.after(hud::update_hud_bar_health_state),
                    hud::resize_hud_fill.after(hud::update_hud_bar_ratios),
                    world_hud::resize_world_hud_fill.after(hud::update_hud_bar_ratios),
                    world_hud::update_world_hud_bar_colors.after(hud::update_hud_bar_health_state),
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PostUpdate,
                hud::update_hud_visibility.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                session::cleanup_session.in_set(SessionCleanup),
            )
            .add_observer(movement::on_start_agent_move)
            .add_observer(movement::on_agent_change_direction)
            .add_observer(movement::on_update_elevation)
            .add_observer(movement::on_teleport_agent)
            .add_observer(events::on_spawn_agent)
            .add_observer(events::on_move_agent)
            .add_observer(events::on_remove_agent)
            .add_observer(events::on_agent_life_changed)
            .add_observer(events::on_agent_mana_changed)
            .add_observer(events::on_agent_speed_updated);
    }
}
