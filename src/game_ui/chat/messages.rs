use bevy::ecs::message::MessageReader;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::{ControlOrientation, CoreScrollbarThumb, Scrollbar};

use crate::conf::ui::{chat, ui_colors};
use crate::game_ui::GameUiAssets;
use crate::game_ui::chat::events::{ActivateChannel, MessageAppendedUi, MessageTrimmedUi};
use crate::game_ui::chat::state::{ChannelId, ChatState, StoredMessage};

#[derive(Component)]
pub struct MessagePanel;

#[derive(Component)]
pub struct MessageScroller;

#[derive(Component)]
pub struct ChatMessageNode {
    pub channel_id: ChannelId,
    pub sequence: u64,
}

pub fn spawn_message_panel(
    commands: &mut Commands,
    state: &ChatState,
    ui_assets: &GameUiAssets,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(2.0), Val::Px(0.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
            },
        ))
        .id();

    let root_bg = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
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

    commands.entity(root).add_child(root_bg);

    let msg_panel = commands
        .spawn((
            MessagePanel,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                ..default()
            },
            ScrollPosition(Vec2 {
                x: 0.0,
                y: f32::MAX,
            }),
            Hovered::default(),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
            },
        ))
        .with_children(|parent| {
            parent
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_grow: 0.0,
                        flex_shrink: 0.0,
                        overflow: Overflow::hidden(),
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
                .add_child(msg_panel)
                .with_child((
                    Node {
                        min_width: px(10),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    Scrollbar {
                        orientation: ControlOrientation::Vertical,
                        target: msg_panel,
                        min_thumb_length: 10.0,
                    },
                    BackgroundColor(chat::INPUT_BG_COLOR.with_alpha(0.8).into()),
                    Children::spawn(Spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            border_radius: BorderRadius::all(px(4)),
                            ..default()
                        },
                        Hovered::default(),
                        BackgroundColor(Srgba::new(0.486, 0.486, 0.529, 1.0).into()),
                        CoreScrollbarThumb,
                    ))),
                ));
        })
        .id();

    commands.entity(root_bg).add_child(panel);

    let scroller = commands
        .spawn((
            MessageScroller,
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();
    commands.entity(msg_panel).add_child(scroller);

    if let Some(channel) = state.channel(state.active) {
        for stored in channel.messages.iter() {
            spawn_message_row(
                commands,
                scroller,
                stored,
                state.active,
                channel.config.text_color,
                ui_assets,
            );
        }
    }

    root
}

fn spawn_message_row(
    commands: &mut Commands,
    parent: Entity,
    stored: &StoredMessage,
    target_channel: ChannelId,
    text_color: Color,
    ui_assets: &GameUiAssets,
) {
    let prefix = format!("[{}] ", stored.timestamp.format("%H:%M"));
    let row = commands
        .spawn((
            ChatMessageNode {
                channel_id: target_channel,
                sequence: stored.sequence,
            },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
        ))
        .id();
    commands.entity(parent).add_child(row);

    commands.entity(row).with_children(|c| {
        c.spawn((
            Text::new(prefix),
            TextColor(text_color),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
        ));
        if let Some(author) = &stored.message.author {
            c.spawn((
                Text::new(format!("{author}: ")),
                TextColor(text_color),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 11.0,
                    ..default()
                },
            ));
        }
        c.spawn((
            Text::new(stored.message.text.clone()),
            TextColor(text_color),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
        ));
    });
}

pub fn on_activate_channel_render(
    event: On<ActivateChannel>,
    mut commands: Commands,
    state: Res<ChatState>,
    scroller_q: Query<Entity, With<MessageScroller>>,
    existing_q: Query<Entity, With<ChatMessageNode>>,
    mut panel_q: Query<&mut ScrollPosition, With<MessagePanel>>,
    ui_assets: Res<GameUiAssets>,
) {
    let Ok(scroller) = scroller_q.single() else {
        return;
    };
    for entity in existing_q.iter() {
        commands.entity(entity).despawn();
    }

    let Some(channel) = state.channel(event.channel_id) else {
        return;
    };
    for stored in channel.messages.iter() {
        spawn_message_row(
            &mut commands,
            scroller,
            stored,
            event.channel_id,
            channel.config.text_color,
            &ui_assets,
        );
    }

    if let Ok(mut sp) = panel_q.single_mut() {
        sp.0.y = f32::MAX;
    }
}

pub fn on_message_appended_ui_render(
    event: On<MessageAppendedUi>,
    mut commands: Commands,
    state: Res<ChatState>,
    scroller_q: Query<Entity, With<MessageScroller>>,
    mut panel_q: Query<&mut ScrollPosition, With<MessagePanel>>,
    ui_assets: Res<GameUiAssets>,
) {
    if state.active != event.channel_id {
        return;
    }
    let Some(channel) = state.channel(event.channel_id) else {
        return;
    };
    let Some(stored) = channel
        .messages
        .iter()
        .find(|m| m.sequence == event.sequence)
    else {
        return;
    };
    let Ok(scroller) = scroller_q.single() else {
        return;
    };
    spawn_message_row(
        &mut commands,
        scroller,
        stored,
        event.channel_id,
        channel.config.text_color,
        &ui_assets,
    );

    if channel.scroll_pinned_bottom
        && let Ok(mut sp) = panel_q.single_mut()
    {
        sp.0.y = f32::MAX;
    }
}

pub fn on_message_trimmed_ui_render(
    event: On<MessageTrimmedUi>,
    mut commands: Commands,
    nodes: Query<(Entity, &ChatMessageNode)>,
) {
    for (entity, node) in nodes.iter() {
        if node.channel_id == event.channel_id && node.sequence == event.sequence {
            commands.entity(entity).despawn();
        }
    }
}

pub fn track_scroll_pinning(
    mut state: ResMut<ChatState>,
    mut mouse_wheel_reader: MessageReader<MouseWheel>,
    mut msg_panel_q: Query<(&Hovered, &ComputedNode, &mut ScrollPosition), With<MessagePanel>>,
) {
    let Ok((hovered, node, mut scroll)) = msg_panel_q.single_mut() else {
        return;
    };

    if !hovered.0 {
        return;
    }

    let max_offset = (node.content_size() - node.size()) * node.inverse_scale_factor();
    for mouse_wheel in mouse_wheel_reader.read() {
        let mut delta = -Vec2::new(mouse_wheel.x, mouse_wheel.y);

        if mouse_wheel.unit == MouseScrollUnit::Line {
            delta *= chat::LINE_HEIGHT;
        }

        if scroll.y + delta.y <= 0.0 {
            scroll.y = 0.0;
        } else if scroll.y + delta.y >= max_offset.y {
            scroll.y = max_offset.y;
        } else {
            scroll.y += delta.y;
        }
    }

    let pinned = (max_offset.y - scroll.0.y).abs() < 4.0;
    let active = state.active;
    if let Some(c) = state.channel_mut(active) {
        c.scroll_pinned_bottom = pinned;
    }
}
