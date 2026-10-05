use bevy::prelude::*;

use crate::conf::ui::{SIDE_PANEL_WIDTH, button_row as conf, ui_colors, z_index::Z_WINDOW};
use crate::game_ui::button::spawn_panel_button;
use crate::game_ui::skills::ToggleSkillsWindow;
use crate::game_ui::{
    GameUiAssets, Index, RightPanelDock, UIWindow, UIWindowDock, UiWindowRef, WindowId,
};
use crate::network::RequestLogout;

/// A headless window in the right dock, below the inventory.
///
/// Same recipe as the minimap and inventory windows: no title bar, so no drag
/// handle and no way to move it to another dock. It is spawned after the inventory
/// so it lands last in the dock's child order.
pub(super) fn spawn_button_row(
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock), With<RightPanelDock>>,
    ui_assets: Res<GameUiAssets>,
) {
    let Ok((dock_entity, dock)) = dock_q.single() else {
        return;
    };

    let window_id = WindowId::new();

    let logout = spawn_panel_button(
        &mut commands,
        "Logout",
        Some(ui_assets.buttons.logout.clone()),
        &ui_assets,
        None,
    );
    commands
        .entity(logout)
        .observe(|mut event: On<Pointer<Click>>, mut commands: Commands| {
            event.propagate(false);
            commands.trigger(RequestLogout);
        });

    let skills = spawn_panel_button(
        &mut commands,
        "Skills",
        Some(ui_assets.buttons.skills.clone()),
        &ui_assets,
        None,
    );
    commands
        .entity(skills)
        .observe(|mut event: On<Pointer<Click>>, mut commands: Commands| {
            event.propagate(false);
            commands.trigger(ToggleSkillsWindow);
        });

    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Start,
            column_gap: Val::Px(conf::PADDING_Y),
            padding: UiRect::axes(Val::Px(conf::PADDING_X), Val::Px(conf::PADDING_Y)),
            ..default()
        })
        .add_child(skills)
        .add_child(logout)
        .id();

    commands.entity(content).insert(UiWindowRef { window_id });

    let window = commands
        .spawn((
            UIWindow {
                id: window_id,
                dock_id: dock.id,
            },
            Index(0),
            Node {
                left: Val::Px(-2.0),
                width: Val::Px(SIDE_PANEL_WIDTH),
                height: Val::Px(conf::HEIGHT),
                min_height: Val::Px(conf::HEIGHT),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::hidden(),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
            },
            ZIndex(Z_WINDOW),
        ))
        .add_child(content)
        .id();

    commands.entity(dock_entity).add_child(window);
}
