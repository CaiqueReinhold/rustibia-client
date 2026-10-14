use bevy::prelude::*;
use bevy_text_outline::TextOutline;

use crate::conf::ui::{chat as conf, ui_colors};
use crate::game_ui::GameUiAssets;
use crate::game_ui::chat::channels_dialog::OpenChannelsDialog;
use crate::game_ui::chat::events::{
    ActivateChannel, ChannelClosedUi, ChannelOpenedUi, CloseChannel, MessageAppendedUi,
};
use crate::game_ui::chat::state::{ChannelId, ChatState};

const TAB_HEIGHT: f32 = 22.0;
const TAB_MAX_WIDTH: f32 = 90.0;
const UNREAD_TAB_COLOR: Srgba = Srgba::new(0.85, 0.20, 0.20, 1.0);

#[derive(Component)]
pub struct TabStripRoot;

#[derive(Component)]
pub struct TabsRow;

#[derive(Component)]
pub struct ChannelButtons;

#[derive(Component)]
pub struct ChannelTab {
    pub channel_id: ChannelId,
}

#[derive(Component)]
pub struct ChannelTabTitle {
    pub channel_id: ChannelId,
}

pub fn spawn_tab_strip(
    commands: &mut Commands,
    state: &ChatState,
    ui_assets: &GameUiAssets,
) -> Entity {
    let strip = commands
        .spawn((
            TabStripRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(TAB_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(4.0), Val::ZERO),
                bottom: Val::Px(-2.0),
                ..default()
            },
            ZIndex(1),
        ))
        .id();

    let row = commands
        .spawn((
            TabsRow,
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Stretch,
                flex_grow: 1.0,
                overflow: Overflow::clip(),
                ..default()
            },
        ))
        .id();

    let initial_tabs: Vec<Entity> = state
        .channels
        .iter()
        .map(|channel| {
            spawn_tab_entity(
                commands,
                channel.config.id,
                &channel.config.name,
                channel.config.id == state.active,
                channel.unread,
                ui_assets,
            )
        })
        .collect();
    if !initial_tabs.is_empty() {
        commands.entity(row).add_children(&initial_tabs);
    }

    let buttons = commands
        .spawn((
            ChannelButtons,
            Node {
                height: Val::Px(TAB_HEIGHT - 4.0),
                flex_direction: FlexDirection::Row,
                ..default()
            },
        ))
        .id();

    add_channel_buttons(buttons, commands, state, ui_assets);

    commands.entity(strip).add_children(&[row, buttons]);
    strip
}

fn add_channel_buttons(
    parent: Entity,
    commands: &mut Commands,
    state: &ChatState,
    ui_assets: &GameUiAssets,
) {
    commands.entity(parent).despawn_children();

    let active_is_closeable = state
        .channels
        .iter()
        .find(|channel| channel.config.id == state.active)
        .map(|channel| channel.config.closeable)
        .unwrap_or(false);
    let active_id = state.active;

    if active_is_closeable {
        let close = commands
            .spawn((
                Button,
                Node {
                    width: Val::Px(TAB_HEIGHT - 4.0),
                    height: Val::Px(TAB_HEIGHT - 4.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border: UiRect::new(Val::Px(1.0), Val::Px(1.0), Val::Px(1.0), Val::ZERO),
                    bottom: Val::Px(-2.0),
                    ..default()
                },
                BorderColor {
                    top: ui_colors::LIGHT_BORDER_COLOR.into(),
                    right: ui_colors::DARK_BORDER_COLOR.into(),
                    bottom: Color::BLACK,
                    left: ui_colors::LIGHT_BORDER_COLOR.into(),
                },
            ))
            .with_child((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
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
            .with_child((
                Node {
                    position_type: PositionType::Relative,
                    display: Display::Flex,
                    ..default()
                },
                Text::new("×"),
                TextColor(conf::TAB_TITLE_COLOR.into()),
                TextFont {
                    font_size: 13.0,
                    ..default()
                },
            ))
            .observe(move |_: On<Pointer<Click>>, mut commands: Commands| {
                commands.trigger(CloseChannel {
                    channel_id: active_id,
                });
            })
            .id();
        commands.entity(parent).add_child(close);
    }

    let plus = commands
        .spawn((
            Button,
            Node {
                width: Val::Px(TAB_HEIGHT - 4.0),
                height: Val::Px(TAB_HEIGHT - 4.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::new(Val::Px(1.0), Val::Px(1.0), Val::Px(1.0), Val::ZERO),
                bottom: Val::Px(-2.0),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: Color::BLACK,
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
            },
        ))
        .with_child((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
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
        .with_child((
            Node {
                position_type: PositionType::Relative,
                display: Display::Flex,
                ..default()
            },
            Text::new("+"),
            TextColor(conf::TAB_TITLE_COLOR.into()),
            TextFont {
                font_size: 13.0,
                ..default()
            },
        ))
        .observe(|_: On<Pointer<Click>>, mut commands: Commands| {
            commands.trigger(OpenChannelsDialog);
        })
        .id();
    commands.entity(parent).add_child(plus);
}

fn rebuild_tab_strip(
    commands: &mut Commands,
    state: &ChatState,
    row_q: &Query<Entity, With<TabsRow>>,
    existing_tabs_q: &Query<Entity, With<ChannelTab>>,
    ui_assets: &GameUiAssets,
) {
    let Ok(row) = row_q.single() else {
        return;
    };

    for tab in existing_tabs_q.iter() {
        commands.entity(tab).despawn();
    }

    let mut new_children = Vec::new();
    for channel in state.channels.iter() {
        new_children.push(spawn_tab_entity(
            commands,
            channel.config.id,
            &channel.config.name,
            channel.config.id == state.active,
            channel.unread,
            ui_assets,
        ));
    }

    if !new_children.is_empty() {
        commands.entity(row).add_children(&new_children);
    }
}

/// Driven by `ChannelOpenedUi`, not `OpenChannel`, so it reads a `ChatState` that
/// already contains the new channel. Observing `OpenChannel` made this a sibling of
/// the observer that pushes the channel, and sibling observers are unordered — the
/// rebuild could run first and silently omit the tab.
pub fn rebuild_tabs_on_open(
    _event: On<ChannelOpenedUi>,
    mut commands: Commands,
    state: Res<ChatState>,
    row_q: Query<Entity, With<TabsRow>>,
    existing_tabs_q: Query<Entity, With<ChannelTab>>,
    ui_assets: Res<GameUiAssets>,
) {
    rebuild_tab_strip(&mut commands, &state, &row_q, &existing_tabs_q, &ui_assets);
}

/// Driven by `ChannelClosedUi` for the same reason as [`rebuild_tabs_on_open`].
pub fn rebuild_tabs_on_close(
    _event: On<ChannelClosedUi>,
    mut commands: Commands,
    state: Res<ChatState>,
    row_q: Query<Entity, With<TabsRow>>,
    existing_tabs_q: Query<Entity, With<ChannelTab>>,
    ui_assets: Res<GameUiAssets>,
) {
    rebuild_tab_strip(&mut commands, &state, &row_q, &existing_tabs_q, &ui_assets);
}

pub fn restyle_tabs_on_activate(
    event: On<ActivateChannel>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    state: Res<ChatState>,
    mut tabs_q: Query<(&ChannelTab, &Children, &mut Node, &mut ZIndex)>,
    mut tab_container_q: Query<&mut ImageNode>,
    mut tab_title_q: Query<(&ChannelTabTitle, &mut TextColor)>,
    buttons_q: Single<Entity, With<ChannelButtons>>,
) {
    let active_id = event.channel_id;
    for (tab, children, mut node, mut zindex) in tabs_q.iter_mut() {
        let is_active = tab.channel_id == active_id;
        node.border.bottom = Val::Px(if is_active { 0.0 } else { 2.0 });
        zindex.0 = if is_active { 1 } else { 0 };
        if let Some(mut img_node) = children
            .first()
            .and_then(|child| tab_container_q.get_mut(*child).ok())
        {
            img_node.image = if is_active {
                ui_assets.background_light.clone()
            } else {
                ui_assets.background_dark.clone()
            };
        };
    }

    for (title, mut color) in tab_title_q.iter_mut() {
        let is_active = title.channel_id == active_id;
        if is_active {
            color.0 = conf::TAB_TITLE_COLOR.into()
        } else {
            color.0 = conf::TAB_TITLE_COLOR_INACTIVE.into()
        }
    }

    add_channel_buttons(*buttons_q, &mut commands, &state, &ui_assets);
}

fn spawn_tab_entity(
    commands: &mut Commands,
    id: ChannelId,
    name: &str,
    active: bool,
    unread: bool,
    ui_assets: &GameUiAssets,
) -> Entity {
    let bg: Handle<Image> = if active {
        ui_assets.background_light.clone()
    } else {
        ui_assets.background_dark.clone()
    };
    let title_color: Color = if unread && !active {
        UNREAD_TAB_COLOR.into()
    } else if active {
        conf::TAB_TITLE_COLOR.into()
    } else {
        conf::TAB_TITLE_COLOR_INACTIVE.into()
    };

    let bottom_border = if active { 0.0 } else { 2.0 };

    let tab_root = commands
        .spawn((
            ChannelTab { channel_id: id },
            Button,
            Node {
                height: Val::Px(TAB_HEIGHT),
                max_width: Val::Px(TAB_MAX_WIDTH),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                border: UiRect::new(
                    Val::Px(2.0),
                    Val::Px(2.0),
                    Val::Px(2.0),
                    Val::Px(bottom_border),
                ),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
            },
            if active { ZIndex(1) } else { ZIndex(0) },
        ))
        .observe(
            move |_: On<Pointer<Click>>, mut commands: Commands, state: Res<ChatState>| {
                if state.active != id {
                    commands.trigger(ActivateChannel { channel_id: id });
                }
            },
        )
        .id();

    let tab_bg = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                padding: UiRect::all(Val::Px(2.0)),
                position_type: PositionType::Relative,
                ..default()
            },
            ImageNode {
                image: bg,
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 1.0,
                },
                ..default()
            },
        ))
        .id();

    commands.entity(tab_root).add_child(tab_bg);

    let title = commands
        .spawn((
            ChannelTabTitle { channel_id: id },
            Node {
                height: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_shrink: 1.0,
                position_type: PositionType::Relative,
                ..default()
            },
            Text::new(name.to_string()),
            TextColor(title_color),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 10.0,
                weight: FontWeight::BOLD,
                ..default()
            },
            TextLayout::new_with_justify(Justify::Center),
            TextOutline {
                width: 1.0,
                ..default()
            },
        ))
        .id();
    commands.entity(tab_bg).add_child(title);

    tab_root
}

pub fn on_message_appended_ui_tabs(
    event: On<MessageAppendedUi>,
    state: Res<ChatState>,
    mut titles: Query<(&ChannelTabTitle, &mut TextColor)>,
) {
    let Some(channel) = state.channel(event.channel_id) else {
        return;
    };
    let active = state.active == event.channel_id;
    let color: Color = if channel.unread && !active {
        UNREAD_TAB_COLOR.into()
    } else {
        conf::TAB_TITLE_COLOR.into()
    };
    for (title, mut text_color) in titles.iter_mut() {
        if title.channel_id == event.channel_id {
            text_color.0 = color;
        }
    }
}
