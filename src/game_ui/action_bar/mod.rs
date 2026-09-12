use bevy::prelude::*;

use crate::core::{GameState, SessionCleanup};

mod activation;
mod persistence;
mod state;

pub use activation::ActionSlotActivated;
pub use state::{ActionBar, ActionSlot, Aim, SlotAction};

pub struct ActionBarPlugin;

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionBar>()
            .add_systems(OnEnter(GameState::InGame), persistence::load_action_bar)
            .add_systems(
                Update,
                (
                    persistence::prune_unknown_spells,
                    persistence::save_action_bar,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                cleanup_session.in_set(SessionCleanup),
            )
            .add_observer(activation::on_action_slot_activated);
    }
}

fn cleanup_session(mut commands: Commands) {
    commands.insert_resource(ActionBar::default());
}
