use std::time::Duration;

use bevy::prelude::*;
use bevy::{camera::visibility::RenderLayers, time::common_conditions::on_timer};

mod action_bar;
mod assets;
pub mod button;
mod button_row;
mod chat;
pub mod context_menu;
mod disconnect;
mod game_overlay;
mod leftpanel;
mod login;
mod modal;
mod outdated;
mod rightpanel;
pub mod scaling;
mod session;
mod skills;
mod toppanel;
mod window;

pub use action_bar::{ActionBar, ActionSlotActivated};
pub use assets::GameUiAssets;
pub use chat::{ChatMode, events::EnterChatMode};
pub use context_menu::{ContextMenu, ContextMenuEntry, ContextMenuPicked, ContextMenuRoot};
pub use game_overlay::GameViewport;
pub use login::{LoginPhase, PendingLoginError};
pub use modal::{
    DialogButton, DialogButtonId, DialogButtonPressed, ModalDialog, ModalDialogRoot, ModalOrder,
};
pub use rightpanel::RightPanelDock;
pub use skills::{SkillProgress, SkillType};
pub use window::{
    AddUIWindow, CloseUIWindow, Index, ReplaceUIWindowContent, UIWindow, UIWindowDock, UiWindowRef,
    WindowId,
};

use crate::camera::GameRenderTexture;
use crate::core::{GameState, PingState, SessionCleanup};

#[derive(Component)]
pub struct MainUI;

#[derive(Component)]
pub struct PingView;

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ui_text_input::TextInputPlugin)
            .add_plugins((
                window::UIWindowPlugin,
                chat::ChatPlugin,
                login::LoginPlugin,
                skills::SkillsPlugin,
                action_bar::ActionBarPlugin,
            ))
            .add_systems(OnEnter(GameState::InGame), spawn_main_ui)
            .add_systems(
                OnEnter(GameState::InGame),
                button_row::spawn_button_row.after(crate::items::inventory::spawn_inventory_ui),
            )
            .add_systems(Startup, assets::setup_game_ui_assets)
            .add_systems(
                Update,
                toppanel::update_bar.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                game_overlay::update_viewport_size.run_if(in_state(GameState::InGame)),
            )
            .add_systems(Update, update_ping.run_if(on_timer(Duration::from_secs(1))))
            .init_resource::<modal::ModalOrder>()
            .add_systems(Update, (modal::modal_keyboard, button::panel_button_hover))
            .add_observer(context_menu::close_other_context_menus)
            .add_systems(
                Update,
                (
                    context_menu::close_context_menus_on_escape,
                    context_menu::keep_context_menus_on_screen,
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                session::cleanup_session.in_set(SessionCleanup),
            )
            .add_observer(outdated::on_client_outdated)
            .add_observer(outdated::on_dismiss)
            .add_observer(disconnect::on_connection_lost)
            .add_observer(disconnect::on_dismiss);
    }
}

pub(crate) fn spawn_main_ui(
    mut commands: Commands,
    render_texture: Res<GameRenderTexture>,
    ui_assets: Res<GameUiAssets>,
    chat_state: Res<chat::ChatState>,
) {
    let main_ui = commands
        .spawn((
            MainUI,
            DespawnOnExit(GameState::InGame),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            RenderLayers::layer(1),
        ))
        .id();

    let left_panel = leftpanel::spawn_left_panel(&mut commands, &ui_assets);
    let middle_container = commands
        .spawn((Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            ..default()
        },))
        .id();
    let right_panel = rightpanel::spawn_right_panel(&mut commands, &ui_assets);
    commands
        .entity(main_ui)
        .add_children(&[left_panel, middle_container, right_panel]);

    let top_panel = toppanel::spawn_top_panel(&mut commands, &ui_assets);
    let gameview = game_overlay::spawn_gameviewport(&mut commands, &render_texture, &ui_assets);
    let chat = chat::spawn_chat_root(&mut commands, &chat_state, &ui_assets);
    commands
        .entity(middle_container)
        .add_children(&[top_panel, gameview, chat]);

    let ping_view = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(15.0),
                ..default()
            },
            Children::spawn(Spawn((
                PingView,
                Text::new(""),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ))),
        ))
        .id();
    commands.entity(gameview).add_child(ping_view);
}

pub fn update_ping(mut ping_text: Single<&mut Text, With<PingView>>, ping_state: Res<PingState>) {
    let text = format!("Ping: {}ms", ping_state.current().as_millis());
    ping_text.0 = text;
}
