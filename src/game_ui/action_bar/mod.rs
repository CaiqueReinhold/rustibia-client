use bevy::prelude::*;

use crate::core::{ActiveCharacter, GameState, SessionCleanup, SpellBook};
use crate::player::Keybinds;

mod activation;
mod assign_item;
mod bar;
mod hotkey_dialog;
mod menu;
mod persistence;
mod spell_dialog;
mod state;
mod target_dialog;

pub use activation::ActionSlotActivated;
pub use bar::spawn_action_bar;
pub use state::ActionBar;

pub struct ActionBarPlugin;

impl Plugin for ActionBarPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionBar>()
            .init_resource::<bar::VisibleSlots>()
            .init_resource::<persistence::WrittenActionBar>()
            .add_systems(Startup, bar::setup_action_bar_assets)
            .add_systems(OnEnter(GameState::InGame), persistence::load_action_bar)
            .add_systems(
                Update,
                (
                    persistence::prune_unknown_spells,
                    bar::update_slot_count,
                    bar::redraw_slots.run_if(
                        resource_changed::<ActionBar>
                            .or(resource_changed::<Keybinds>)
                            .or(resource_changed::<SpellBook>)
                            .or(resource_changed::<bar::VisibleSlots>),
                    ),
                    persistence::save_action_bar
                        .run_if(resource_changed::<ActionBar>.or(resource_changed::<Keybinds>)),
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                cleanup_session.in_set(SessionCleanup),
            )
            .add_observer(activation::on_action_slot_activated)
            .add_systems(
                Update,
                target_dialog::update_aim_options.run_if(in_state(GameState::InGame)),
            )
            .add_observer(target_dialog::on_open_target_dialog)
            .add_observer(target_dialog::on_target_dialog_button)
            .add_observer(assign_item::on_object_picked)
            .add_systems(
                Update,
                spell_dialog::update_spell_row_highlight.run_if(in_state(GameState::InGame)),
            )
            .add_observer(spell_dialog::on_open_assign_spell_dialog)
            .add_observer(spell_dialog::on_assign_spell_button)
            .add_systems(
                Update,
                (
                    hotkey_dialog::capture_hotkey,
                    hotkey_dialog::refresh_hotkey_dialog,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            )
            .add_observer(hotkey_dialog::on_open_hotkey_dialog)
            .add_observer(hotkey_dialog::on_hotkey_dialog_button)
            .add_observer(menu::on_open_slot_menu)
            .add_observer(menu::on_slot_menu_picked);
    }
}

/// Writes any change the last `Update` did not reach before resetting. `ActiveCharacter` is still
/// here: the core's cleanup drops it through a command, applied after every member of this set.
fn cleanup_session(
    mut commands: Commands,
    bar: Res<ActionBar>,
    mut keybinds: ResMut<Keybinds>,
    mut written: ResMut<persistence::WrittenActionBar>,
    character: Option<Res<ActiveCharacter>>,
) {
    persistence::flush_action_bar(&bar, &keybinds, &mut written, character.as_deref());
    keybinds.unbind_all_slots();
    commands.insert_resource(persistence::WrittenActionBar::default());
    commands.insert_resource(ActionBar::default());
    commands.insert_resource(bar::VisibleSlots::default());
}
