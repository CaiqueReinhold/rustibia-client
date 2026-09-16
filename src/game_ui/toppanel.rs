use bevy::prelude::*;

use crate::agent::{Health, HealthState, HudBar, Mana};
use crate::conf::ui::cooldown as cooldown_conf;
use crate::conf::ui::z_index::Z_MAIN_UI;
use crate::conf::ui::{TOP_BAR_HEIGHT, UI_BAR_HEIGHT, ui_colors};
use crate::core::SpellGroup;
use crate::game_ui::assets::GameUiAssets;
use crate::game_ui::cooldown::{
    CooldownAssets, CooldownOverlay, group_icon, spawn_cooldown_overlay,
};
use crate::game_ui::status_bar::{StatusAssets, spawn_status_bar};
use crate::player::components::Player;

#[derive(Component)]
pub struct TopPanel;

#[derive(Resource)]
pub struct BarEntities {
    pub health_bar: Entity,
    pub health_text: Entity,
    pub mana_bar: Entity,
    pub mana_text: Entity,
}

pub fn spawn_top_panel(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    cooldown_assets: &CooldownAssets,
    status_assets: &StatusAssets,
) -> Entity {
    let top_panel = commands
        .spawn((
            TopPanel,
            Node {
                position_type: PositionType::Relative,
                width: Val::Percent(100.0),
                max_height: Val::Px(TOP_BAR_HEIGHT),
                min_height: Val::Px(TOP_BAR_HEIGHT),
                border: UiRect {
                    top: Val::Px(1.0),
                    left: Val::Px(2.0),
                    right: Val::Px(2.0),
                    bottom: Val::Px(2.0),
                },
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
            },
            ZIndex(Z_MAIN_UI),
        ))
        .id();

    let panel_inner = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(2.0)),
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

    commands.entity(top_panel).add_child(panel_inner);

    let bars_container = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            row_gap: Val::Px(3.0),
            ..default()
        },))
        .id();
    commands.entity(panel_inner).add_child(bars_container);

    let (health_bar, health_text) = spawn_ui_bar(bars_container, commands, true, ui_assets, 1.0);
    let (mana_bar, mana_text) = spawn_ui_bar(bars_container, commands, false, ui_assets, 0.3);

    commands.insert_resource(BarEntities {
        health_bar,
        health_text,
        mana_bar,
        mana_text,
        // experience,
    });

    let group_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(cooldown_conf::ICON_GAP),
            margin: UiRect::top(Val::Px(cooldown_conf::ICON_GAP)),
            ..default()
        })
        .id();
    commands.entity(panel_inner).add_child(group_row);

    for group in SpellGroup::ALL {
        let overlay = spawn_cooldown_overlay(commands, CooldownOverlay::Group(group), None);
        let icon = commands
            .spawn((
                group_icon(cooldown_assets, group),
                Node {
                    width: Val::Px(cooldown_conf::ICON_SIZE),
                    height: Val::Px(cooldown_conf::ICON_SIZE),
                    flex_shrink: 0.0,
                    ..default()
                },
            ))
            .add_child(overlay)
            .id();
        commands.entity(group_row).add_child(icon);
    }

    let status_bar = spawn_status_bar(commands, &ui_assets.background_dark, status_assets);
    commands.entity(group_row).add_child(status_bar);

    top_panel
}

fn spawn_ui_bar(
    parent: Entity,
    commands: &mut Commands,
    add_health_state: bool,
    ui_assets: &GameUiAssets,
    alpha: f32,
) -> (Entity, Entity) {
    let hud_bar = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            HudBar { ratio: 1.0 },
            ImageNode {
                image: ui_assets.bar_overlay.clone(),
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: false,
                    stretch_value: 1.0,
                },
                color: Srgba::new(1.0, 1.0, 1.0, alpha).into(),
                ..default()
            },
        ))
        .id();
    if add_health_state {
        commands.entity(hud_bar).insert(HealthState::Full);
    } else {
        commands
            .entity(hud_bar)
            .insert(BackgroundColor(ui_colors::MANA_BAR_COLOR.into()));
    }

    let text = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            Text::new(""),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextLayout::new_with_justify(Justify::Center),
        ))
        .id();

    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(50.0),
                height: Val::Px(UI_BAR_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
        ))
        .add_child(hud_bar)
        .add_child(text)
        .id();
    commands.entity(parent).add_child(bar);

    (hud_bar, text)
}

pub fn update_bar(
    bars: Res<BarEntities>,
    player: Single<(&Health, &Mana), (With<Player>, Or<(Changed<Health>, Changed<Mana>)>)>,
    mut bar_q: Query<&mut HudBar>,
    mut text_q: Query<&mut Text>,
) {
    let (health, mana) = *player;

    if let Ok(mut health_bar) = bar_q.get_mut(bars.health_bar) {
        health_bar.ratio = health.ratio();
    }

    if let Ok(mut health_text) = text_q.get_mut(bars.health_text) {
        health_text.0 = format!("{}/{}", health.current, health.max);
    }

    if let Ok(mut mana_bar) = bar_q.get_mut(bars.mana_bar) {
        mana_bar.ratio = mana.ratio();
    }

    if let Ok(mut mana_text) = text_q.get_mut(bars.mana_text) {
        mana_text.0 = format!("{}/{}", mana.current, mana.max);
    }
}
