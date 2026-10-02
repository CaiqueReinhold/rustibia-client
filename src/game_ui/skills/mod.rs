mod state;
mod types;
mod window;

pub use state::{SkillsState, on_experience_changed, on_player_skills, on_skill_changed};
pub use types::{DISPLAY_ORDER, SkillProgress, SkillType};

use bevy::prelude::*;

use window::SkillsWindow;

use crate::conf::ui::SKILLS_WINDOW_HEIGHT;
use crate::core::GameState;
use crate::game_ui::{AddUIWindow, CloseUIWindow, GameUiAssets, UiWindowRef};

#[derive(Event, Debug)]
pub struct ToggleSkillsWindow;

pub fn on_toggle_skills_window(
    _event: On<ToggleSkillsWindow>,
    mut commands: Commands,
    open_q: Query<&UiWindowRef, With<SkillsWindow>>,
    ui_assets: Res<GameUiAssets>,
    state: Res<SkillsState>,
) {
    if let Ok(window_ref) = open_q.single() {
        commands.trigger(CloseUIWindow {
            window_id: window_ref.window_id,
        });
        return;
    }

    let content = window::spawn_skills_content(&mut commands, &ui_assets, &state);
    commands.trigger(AddUIWindow {
        content,
        default_height: SKILLS_WINDOW_HEIGHT,
        title: "Skills".to_string(),
        custom_buttons: Vec::new(),
    });
}

pub struct SkillsPlugin;

impl Plugin for SkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SkillsState>()
            .add_observer(on_player_skills)
            .add_observer(on_skill_changed)
            .add_observer(on_experience_changed)
            .add_observer(on_toggle_skills_window)
            .add_systems(
                Update,
                window::update_skills_window
                    .run_if(in_state(GameState::InGame))
                    .run_if(resource_changed::<SkillsState>),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_ui::WindowId;
    use crate::game_ui::assets::{UiInventory, UiWindow as UiWindowAssets};

    /// `UIWindowPlugin` cannot run under `MinimalPlugins`: its `on_window_scroll`
    /// system reads `MessageReader<MouseWheel>`, which panics unless a real input
    /// plugin has initialized that message type. So this exercises the observer
    /// directly on a bare `World` rather than through the real window plugin,
    /// and asserts on the effect the observer itself is responsible for -- that
    /// it calls `spawn_skills_content` when nothing is open, and that it fires
    /// `CloseUIWindow` for the right window when something is.
    fn world_with_assets(state: SkillsState) -> World {
        let mut world = World::new();
        world.insert_resource(GameUiAssets {
            font: Handle::default(),
            name_font: Handle::default(),
            window: UiWindowAssets::default(),
            inventory: UiInventory::default(),
            background_dark: Handle::default(),
            background_light: Handle::default(),
            bar_overlay: Handle::default(),
            title_background: Handle::default(),
        });
        world.insert_resource(state);
        world.add_observer(on_toggle_skills_window);
        world
    }

    #[test]
    fn opens_when_none_is_open() {
        let mut world = world_with_assets(SkillsState::default());

        world.trigger(ToggleSkillsWindow);
        world.flush();

        let mut query = world.query_filtered::<Entity, With<SkillsWindow>>();
        assert_eq!(query.iter(&world).count(), 1);
    }

    #[test]
    fn closes_the_one_that_is_open() {
        let mut world = world_with_assets(SkillsState::default());
        let open_window_id = WindowId::new();
        world.spawn((
            SkillsWindow,
            UiWindowRef {
                window_id: open_window_id,
            },
        ));

        #[derive(Resource, Default)]
        struct SeenClose(Option<WindowId>);
        world.init_resource::<SeenClose>();
        world.add_observer(|event: On<CloseUIWindow>, mut seen: ResMut<SeenClose>| {
            seen.0 = Some(event.window_id);
        });

        world.trigger(ToggleSkillsWindow);
        world.flush();

        assert_eq!(world.resource::<SeenClose>().0, Some(open_window_id));

        let mut query = world.query_filtered::<Entity, With<SkillsWindow>>();
        assert_eq!(
            query.iter(&world).count(),
            1,
            "the observer must not have spawned a second window"
        );
    }
}
