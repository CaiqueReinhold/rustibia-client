mod graphics;
mod persistence;
mod state;
mod window;

pub use state::Settings;
pub use window::OpenOptionsWindow;

use bevy::prelude::*;

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        let settings = persistence::read_settings(&persistence::file_path());
        app.insert_resource(persistence::WrittenSettings(Some(persistence::serialize(
            &settings,
        ))))
        .insert_resource(settings)
        .add_systems(
            Update,
            persistence::save_settings.run_if(resource_changed::<Settings>),
        )
        .add_systems(
            Update,
            graphics::apply_present_mode.run_if(resource_changed::<Settings>),
        )
        .add_observer(window::on_open_options_window)
        .add_observer(window::on_select_options_page)
        .add_observer(window::on_flip_setting)
        .add_observer(window::on_options_button)
        .add_systems(
            Update,
            (window::sync_toggle_ticks, window::sync_options_slider)
                .run_if(any_with_component::<window::OptionsWindow>),
        );
    }
}
