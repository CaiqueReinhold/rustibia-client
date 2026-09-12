use bevy::prelude::*;

pub mod components;
mod events;
mod hotkey;
mod interaction;
mod keyboard;
pub mod movement;
pub mod pathfinding;
mod session;
pub mod spells;
pub mod target;
pub use hotkey::Hotkey;
pub use interaction::{
    ContainerNavTarget, InteractionIntent, InteractionMode, MouseHoverState, ObjectPicked,
    TargetingSource,
};
pub use keyboard::Keybinds;

use crate::core::{GameState, SessionCleanup, SessionEnding};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<interaction::MouseHoverState>()
            .init_resource::<interaction::InteractionMode>()
            .init_resource::<keyboard::Keybinds>()
            .init_resource::<movement::MovementQueue>()
            .init_resource::<movement::PlayerElevation>()
            .init_resource::<target::CombatTarget>()
            .add_systems(Startup, keyboard::init_repeat_state)
            .add_systems(
                OnExit(GameState::InGame),
                (session::cleanup_session, keyboard::init_repeat_state).in_set(SessionCleanup),
            )
            .add_systems(
                PreUpdate,
                (
                    interaction::update_hover_state,
                    keyboard::read_player_input
                        .run_if(in_state(GameState::InGame))
                        .run_if(not(resource_exists::<SessionEnding>)),
                    keyboard::cancel_targeting_on_escape
                        .run_if(in_state(GameState::InGame))
                        .run_if(not(resource_exists::<SessionEnding>)),
                ),
            )
            .add_systems(
                Update,
                (movement::process_move_queue, movement::fire_pending_action)
                    .chain()
                    .run_if(in_state(GameState::InGame))
                    .run_if(not(resource_exists::<SessionEnding>)),
            )
            .add_systems(
                Update,
                movement::update_player_elevation
                    .run_if(in_state(GameState::InGame))
                    .after(crate::agent::movement::move_agent),
            )
            .add_systems(
                PostUpdate,
                (
                    // Before layout — and so before `TransformSystems::Propagate` —
                    // so the camera the HUD reads is the one this frame renders with.
                    movement::center_on_player
                        .before(bevy::ui::UiSystems::Layout)
                        .run_if(in_state(GameState::InGame)),
                    interaction::sync_targeting_cursor.run_if(in_state(GameState::InGame)),
                ),
            )
            .add_systems(
                Update,
                events::check_game_ready.run_if(in_state(GameState::Connecting)),
            )
            .add_observer(interaction::attach_observers)
            .add_observer(interaction::on_interaction_intent)
            .add_observer(movement::on_player_walk)
            .add_observer(movement::on_ack_walk)
            .add_observer(movement::on_player_position)
            .add_observer(movement::on_walk_denied)
            .add_observer(movement::player_changed_direction_ack)
            .add_observer(movement::on_player_change_direction)
            .add_observer(movement::on_update_elevation_player)
            .add_observer(events::spawn_player)
            .add_observer(events::on_slot_update)
            .add_observer(events::on_capacity_update)
            .add_observer(interaction::on_targeting_tile_changed)
            .add_observer(interaction::on_targeting_container_updated)
            .add_observer(interaction::on_targeting_container_closed)
            .add_observer(interaction::on_targeting_inventory_updated)
            .add_observer(target::on_target_lost)
            .add_observer(spells::on_cast_spell_requested)
            .add_observer(spells::on_spell_cast);
    }
}
