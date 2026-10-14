use bevy::prelude::*;

use crate::camera::{GameRenderTexture, HudRenderTexture};
use crate::conf::ui::z_index::Z_MAIN_UI;
use crate::conf::ui::{
    GAME_VIEWPORT_FRAME, GAME_VIEWPORT_MIN_HEIGHT, GAME_VIEWPORT_MIN_WIDTH, ui_colors,
};
use crate::conf::viewport::{GAME_VIEW_HEIGHT, GAME_VIEW_WIDTH};
use crate::core::SpellGroup;
use crate::game_ui::GameUiAssets;
use crate::game_ui::cooldown::{
    CooldownAssets, CooldownOverlay, group_icon, spawn_cooldown_overlay,
};
use crate::game_ui::scaling::logical_size;

const COOLDOWN_ICON_SIZE: f32 = 20.0;
const COOLDOWN_ICON_GAP: f32 = 3.0;
const COOLDOWN_GROUP_ROW_INSET: f32 = 4.0;

#[derive(Component)]
pub struct GameViewport;

#[derive(Component)]
pub(super) struct GameViewportContainer;

#[derive(Component)]
pub(super) struct GameViewportBorder;

const ASPECT_RATIO: f32 = GAME_VIEW_WIDTH / GAME_VIEW_HEIGHT;

pub(super) fn update_viewport_size(
    container_query: Query<&ComputedNode, (With<GameViewportContainer>, Changed<ComputedNode>)>,
    mut border_query: Query<&mut Node, With<GameViewportBorder>>,
) {
    let Ok(computed) = container_query.single() else {
        return;
    };
    let Ok(mut node) = border_query.single_mut() else {
        return;
    };

    // Logical: the results below are written back as `Val::Px`.
    let avail = logical_size(computed) - Vec2::splat(GAME_VIEWPORT_FRAME);
    let (avail_w, avail_h) = (avail.x, avail.y);

    let (w, h) = if avail_w / avail_h > ASPECT_RATIO {
        // height-limited: fit to height
        let h = avail_h;
        (h * ASPECT_RATIO, h)
    } else {
        // width-limited: fit to width
        let w = avail_w;
        (w, w / ASPECT_RATIO)
    };

    node.width = Val::Px(w);
    node.height = Val::Px(h);
}

/// Spawn the game viewport node. The game camera renders into an offscreen
/// texture which is displayed here as an ImageNode, scaled to fill the
/// available space while preserving the game's aspect ratio.
pub fn spawn_gameviewport(
    commands: &mut Commands,
    render_texture: &GameRenderTexture,
    hud_texture: &HudRenderTexture,
    ui_assets: &GameUiAssets,
    cooldown_assets: &CooldownAssets,
) -> Entity {
    let group_cooldowns = spawn_group_cooldowns(commands, cooldown_assets);
    commands
        .spawn((
            GameViewportContainer,
            Node {
                max_width: Val::Percent(100.0),
                max_height: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(GAME_VIEWPORT_MIN_HEIGHT + GAME_VIEWPORT_FRAME),
                min_width: Val::Px(GAME_VIEWPORT_MIN_WIDTH + GAME_VIEWPORT_FRAME),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
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
            ZIndex(Z_MAIN_UI),
        ))
        .with_children(|view| {
            view.spawn((
                GameViewportBorder,
                Node {
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(1.0)),
                    margin: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BorderColor {
                    top: ui_colors::DARK_BORDER_COLOR.into(),
                    right: ui_colors::LIGHT_BORDER_COLOR.into(),
                    bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                    left: ui_colors::DARK_BORDER_COLOR.into(),
                },
            ))
            .with_children(|border| {
                border.spawn((
                    GameViewport,
                    Node {
                        height: Val::Percent(100.0),
                        width: Val::Percent(100.0),
                        ..default()
                    },
                    ImageNode::new(render_texture.0.clone()),
                    children![(
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        ImageNode::new(hud_texture.0.clone()),
                        Pickable::IGNORE,
                    )],
                ));

                let border_ent = border.target_entity();
                border
                    .commands_mut()
                    .entity(border_ent)
                    .add_child(group_cooldowns);
            });
        })
        .id()
}

fn spawn_group_cooldowns(commands: &mut Commands, assets: &CooldownAssets) -> Entity {
    let row = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(COOLDOWN_GROUP_ROW_INSET),
                bottom: Val::Px(COOLDOWN_GROUP_ROW_INSET),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(COOLDOWN_ICON_GAP),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();

    for group in SpellGroup::ALL {
        let overlay = spawn_cooldown_overlay(commands, CooldownOverlay::Group(group), None);
        let icon = commands
            .spawn((
                group_icon(assets, group),
                Node {
                    width: Val::Px(COOLDOWN_ICON_SIZE),
                    height: Val::Px(COOLDOWN_ICON_SIZE),
                    flex_shrink: 0.0,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .add_child(overlay)
            .id();
        commands.entity(row).add_child(icon);
    }

    row
}
