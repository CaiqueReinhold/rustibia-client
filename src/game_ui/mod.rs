use std::time::Duration;

use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::CursorIcon;
use bevy::{camera::visibility::RenderLayers, time::common_conditions::on_timer};

mod action_bar;
mod assets;
pub mod button;
mod button_row;
mod chat;
pub mod context_menu;
mod cooldown;
mod death;
mod disconnect;
mod files;
mod floating;
mod game_overlay;
mod leftpanel;
mod login;
mod modal;
mod outdated;
mod rightpanel;
pub mod scaling;
mod session;
mod settings;
mod shop;
mod skills;
mod status_bar;
mod toppanel;
mod window;

pub use action_bar::ActionSlotActivated;
pub use assets::GameUiAssets;
pub use chat::{ChatMode, events::EnterChatMode};
pub use context_menu::{ContextMenu, ContextMenuEntry, ContextMenuPicked, ContextMenuRoot};
pub use floating::{FloatingDialog, FloatingDialogClosed};
pub use game_overlay::GameViewport;
pub use login::{LoginPhase, PendingLoginError};
pub use modal::{
    DialogButton, DialogButtonId, DialogButtonPressed, ModalDialog, ModalDialogRoot, ModalOrder,
};
pub use rightpanel::RightPanelDock;
pub use settings::Settings;
pub use shop::ShopOffer;
pub use skills::{SkillProgress, SkillType};
pub use window::{
    AddUIWindow, CloseUIWindow, Index, ReplaceUIWindowContent, UIWindow, UIWindowDock, UiWindowRef,
    WindowId,
};

use crate::camera::{GameRenderTexture, HudRenderTexture};
use crate::conf::ui::{
    CHAT_BOX_MIN_HEIGHT, GAME_VIEWPORT_FRAME, GAME_VIEWPORT_MIN_HEIGHT, SEPARATOR_HEIGHT, ui_colors,
};
use crate::core::{GameState, PingState, SessionCleanup};
use crate::game_ui::chat::ChatRoot;
use crate::game_ui::game_overlay::GameViewportContainer;
use crate::game_ui::scaling::logical_size;

const FPS_REFRESH_MS: u64 = 500;

#[derive(Component)]
pub struct MainUI;

#[derive(Component)]
pub struct PingView;

#[derive(Component)]
pub struct FpsView;

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ui_text_input::TextInputPlugin)
            .add_plugins((
                window::UIWindowPlugin,
                chat::ChatPlugin,
                login::LoginPlugin,
                skills::SkillsPlugin,
                shop::ShopPlugin,
                action_bar::ActionBarPlugin,
                settings::SettingsPlugin,
            ))
            .add_systems(OnEnter(GameState::InGame), spawn_main_ui)
            .add_systems(
                OnEnter(GameState::InGame),
                button_row::spawn_button_row.after(crate::items::inventory::spawn_inventory_ui),
            )
            .add_systems(Startup, assets::setup_game_ui_assets)
            .add_systems(Startup, cooldown::setup_cooldown_assets)
            .add_systems(Startup, status_bar::setup_status_assets)
            .add_systems(
                Update,
                toppanel::update_bar.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PostUpdate,
                cooldown::update_cooldown_overlays
                    .before(bevy::ui::UiSystems::Content)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PostUpdate,
                status_bar::update_status_icons
                    .before(bevy::ui::UiSystems::Layout)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                game_overlay::update_viewport_size.run_if(in_state(GameState::InGame)),
            )
            .add_systems(Update, update_ping.run_if(on_timer(Duration::from_secs(1))))
            .add_systems(
                Update,
                update_fps.run_if(on_timer(Duration::from_millis(FPS_REFRESH_MS))),
            )
            .add_systems(
                Update,
                apply_overlay_text_settings.run_if(resource_changed::<Settings>),
            )
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
            .add_observer(disconnect::on_dismiss)
            .add_observer(death::on_player_died)
            .add_observer(death::on_dismiss);
    }
}

pub(crate) fn spawn_main_ui(
    mut commands: Commands,
    render_texture: Res<GameRenderTexture>,
    hud_texture: Res<HudRenderTexture>,
    ui_assets: Res<GameUiAssets>,
    cooldown_assets: Res<cooldown::CooldownAssets>,
    status_assets: Res<status_bar::StatusAssets>,
    chat_state: Res<chat::ChatState>,
    settings: Res<Settings>,
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

    let top_panel = toppanel::spawn_top_panel(&mut commands, &ui_assets, &status_assets);
    let gameview = game_overlay::spawn_gameviewport(
        &mut commands,
        &render_texture,
        &hud_texture,
        &ui_assets,
        &cooldown_assets,
    );
    let action_bar = action_bar::spawn_action_bar(&mut commands, &ui_assets);
    let chat = chat::spawn_chat_root(&mut commands, &chat_state, &ui_assets);
    let separator = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(SEPARATOR_HEIGHT),
                flex_shrink: 0.0,
                border: UiRect::axes(Val::ZERO, Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
            },
            ZIndex(100),
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
        .observe(
            |_: On<Pointer<Over>>, mut commands: Commands, window: Single<Entity, With<Window>>| {
                commands.entity(*window).insert(CursorIcon::System(
                    bevy::window::SystemCursorIcon::RowResize,
                ));
            },
        )
        .observe(
            |_: On<Pointer<Out>>, mut commands: Commands, window: Single<Entity, With<Window>>| {
                commands
                    .entity(*window)
                    .insert(CursorIcon::System(bevy::window::SystemCursorIcon::Default));
            },
        )
        .observe(
            |event: On<Pointer<Drag>>,
             mut chat_q: Query<(&mut Node, &ComputedNode), With<ChatRoot>>,
             viewport_q: Query<&ComputedNode, With<GameViewportContainer>>| {
                let (Ok((mut node, chat)), Ok(viewport)) =
                    (chat_q.single_mut(), viewport_q.single())
                else {
                    return;
                };
                let height = match &node.height {
                    Val::Px(px) => *px,
                    _ => 0.0,
                };
                let shared = logical_size(chat).y + logical_size(viewport).y;
                node.height = Val::Px(dragged_chat_height(height, event.delta.y, shared));
            },
        )
        .id();
    commands
        .entity(middle_container)
        .add_children(&[top_panel, gameview, separator, action_bar, chat]);

    let ping_view = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(10.0),
                left: Val::Px(15.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Children::spawn((
                Spawn(overlay_text(PingView, &ui_assets, settings.show_ping)),
                Spawn(overlay_text(FpsView, &ui_assets, settings.show_fps)),
            )),
        ))
        .id();
    commands.entity(gameview).add_child(ping_view);
}

/// The chat's height after the separator moves `drag` pixels down. `shared` is
/// the height the chat and the game viewport's container divide between them.
fn dragged_chat_height(height: f32, drag: f32, shared: f32) -> f32 {
    let max = shared - GAME_VIEWPORT_MIN_HEIGHT - GAME_VIEWPORT_FRAME;
    (height - drag).clamp(CHAT_BOX_MIN_HEIGHT, max.max(CHAT_BOX_MIN_HEIGHT))
}

pub fn update_ping(mut ping_text: Single<&mut Text, With<PingView>>, ping_state: Res<PingState>) {
    let text = format!("Ping: {}ms", ping_state.current().as_millis());
    ping_text.0 = text;
}

fn overlay_text(marker: impl Component, ui_assets: &GameUiAssets, shown: bool) -> impl Bundle {
    (
        marker,
        Text::new(""),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            display: display_for(shown),
            ..default()
        },
    )
}

fn display_for(shown: bool) -> Display {
    if shown { Display::Flex } else { Display::None }
}

pub fn apply_overlay_text_settings(
    settings: Res<Settings>,
    mut ping_q: Query<&mut Node, (With<PingView>, Without<FpsView>)>,
    mut fps_q: Query<&mut Node, (With<FpsView>, Without<PingView>)>,
) {
    for mut node in &mut ping_q {
        node.display = display_for(settings.show_ping);
    }
    for mut node in &mut fps_q {
        node.display = display_for(settings.show_fps);
    }
}

pub fn update_fps(
    mut fps_text: Single<&mut Text, With<FpsView>>,
    diagnostics: Res<DiagnosticsStore>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps| fps.smoothed());
    fps_text.0 = fps_label(fps);
}

fn fps_label(fps: Option<f64>) -> String {
    match fps {
        Some(fps) => format!("FPS: {fps:.0}"),
        None => "FPS: -".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn the_fps_label_rounds_and_handles_no_reading() {
        assert_eq!(fps_label(Some(143.6)), "FPS: 144");
        assert_eq!(fps_label(None), "FPS: -");
    }

    #[test]
    fn a_chat_drag_stops_at_the_viewport_and_chat_minimums() {
        let shared = 600.0;
        let max = shared - GAME_VIEWPORT_MIN_HEIGHT - GAME_VIEWPORT_FRAME;

        assert_eq!(dragged_chat_height(170.0, -30.0, shared), 200.0);
        assert_eq!(dragged_chat_height(170.0, -900.0, shared), max);
        assert_eq!(
            dragged_chat_height(170.0, 900.0, shared),
            CHAT_BOX_MIN_HEIGHT
        );
        assert_eq!(dragged_chat_height(900.0, 0.0, shared), max);
        assert_eq!(
            dragged_chat_height(170.0, -900.0, 100.0),
            CHAT_BOX_MIN_HEIGHT
        );
    }

    #[test]
    fn each_overlay_text_follows_its_own_setting() {
        let mut world = World::new();
        world.insert_resource(Settings {
            show_ping: false,
            show_fps: true,
            ..default()
        });
        let ping = world.spawn((PingView, Node::default())).id();
        let fps = world.spawn((FpsView, Node::default())).id();

        world.run_system_once(apply_overlay_text_settings).unwrap();

        assert_eq!(world.get::<Node>(ping).unwrap().display, Display::None);
        assert_eq!(world.get::<Node>(fps).unwrap().display, Display::Flex);
    }
}
