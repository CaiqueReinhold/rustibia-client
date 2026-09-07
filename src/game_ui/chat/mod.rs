use bevy::prelude::*;

use crate::conf::ui::{CHAT_BOX_HEIGHT, SEPARATOR_HEIGHT, ui_colors};
use crate::core::GameState;
use crate::game_ui::GameUiAssets;

pub mod channels_dialog;
pub mod events;
pub mod input;
pub mod messages;
pub mod network;
pub mod routing;
pub mod state;
pub mod tabs;

pub use state::{ChatMode, ChatState};

#[derive(Component)]
pub struct ChatRoot;

pub struct ChatPlugin;

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<state::ChatState>()
            .init_resource::<state::ChatMode>()
            .add_observer(state::on_open_channel)
            .add_observer(state::on_close_channel)
            .add_observer(state::on_activate_channel)
            .add_observer(state::on_append_chat_message)
            .add_observer(state::on_enter_chat_mode)
            .add_observer(state::on_exit_chat_mode)
            .add_observer(tabs::on_message_appended_ui_tabs)
            .add_observer(tabs::rebuild_tabs_on_open)
            .add_observer(tabs::rebuild_tabs_on_close)
            .add_observer(tabs::restyle_tabs_on_activate)
            .add_observer(messages::on_activate_channel_render)
            .add_observer(messages::on_message_appended_ui_render)
            .add_observer(messages::on_message_trimmed_ui_render)
            .add_observer(input::on_submit_chat_input)
            .add_observer(network::on_channel_list_received)
            .add_observer(network::on_private_chat_opened)
            .add_observer(network::on_chat_message_received)
            .add_observer(network::on_open_channel_wire)
            .add_observer(network::on_close_channel_wire)
            .add_observer(channels_dialog::on_open_channels_dialog)
            .add_observer(channels_dialog::on_channels_dialog_button)
            .add_systems(OnEnter(GameState::InGame), network::request_channels)
            .add_systems(
                Update,
                (
                    messages::track_scroll_pinning,
                    input::keyboard_exit_chat_mode,
                    input::on_text_input_submit,
                    input::on_chat_mode_changed,
                )
                    .run_if(in_state(GameState::InGame)),
            );
    }
}

pub fn spawn_chat_root(
    commands: &mut Commands,
    state: &ChatState,
    ui_assets: &GameUiAssets,
) -> Entity {
    let root = commands
        .spawn((
            ChatRoot,
            Node {
                position_type: PositionType::Relative,
                width: Val::Percent(100.0),
                height: Val::Px(CHAT_BOX_HEIGHT),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ImageNode {
                image: ui_assets.background_dark.clone(),
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 1.0,
                },
                ..default()
            },
        ))
        .id();

    let separator = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(SEPARATOR_HEIGHT),
                border: UiRect::axes(Val::ZERO, Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
            },
        ))
        .with_child((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode {
                image: ui_assets.background_light.clone(),
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 1.0,
                },
                ..default()
            },
        ))
        .id();

    let tab_strip = tabs::spawn_tab_strip(commands, state, ui_assets);
    let panel = messages::spawn_message_panel(commands, state, ui_assets);
    let input = input::spawn_input(commands, ui_assets);

    commands
        .entity(root)
        .add_children(&[separator, tab_strip, panel, input]);

    root
}
