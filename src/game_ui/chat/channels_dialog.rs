use bevy::prelude::*;
use bevy_ui_text_input::{TextInputContents, TextInputMode, TextInputNode, TextInputPrompt};

use crate::conf::ui::{chat as chat_conf, dialog as conf, ui_colors};
use crate::game_ui::GameUiAssets;
use crate::game_ui::chat::events::OpenChannel;
use crate::game_ui::chat::state::ChatState;
use crate::game_ui::modal::{
    DialogButton, DialogButtonId, DialogButtonPressed, ModalDialog, ModalOrder,
};
use crate::network::{ClientMessage, SendMessage};

#[derive(Event)]
pub struct OpenChannelsDialog;

#[derive(Component)]
pub struct ChannelsDialog;

/// Marks the private-name field so the OK handler can read it.
#[derive(Component)]
pub struct PrivateNameField;

pub fn on_open_channels_dialog(
    _: On<OpenChannelsDialog>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    state: Res<ChatState>,
    existing: Query<Entity, With<ChannelsDialog>>,
) {
    // One at a time — a second click must not stack dialogs.
    if !existing.is_empty() {
        return;
    }

    let handle = ModalDialog::new("Channels")
        .with_buttons([DialogButton::ok(), DialogButton::cancel()])
        .spawn(&mut commands, &ui_assets, &mut order);
    let dialog = handle.root;
    commands.entity(dialog).insert(ChannelsDialog);

    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
        ))
        .id();

    // Only channels not already open. `available` is empty until the server's
    // MSG_CHANNEL_LIST arrives, so an early click shows an empty list rather than
    // stale mock data.
    for config in state
        .available
        .iter()
        .filter(|c| !state.is_open(c.id))
        .cloned()
    {
        let label = config.name.clone();
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_child((
                Text::new(label),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            ))
            .observe(move |click: On<Pointer<Click>>, mut commands: Commands| {
                if click.button != PointerButton::Primary {
                    return;
                }
                // Optimistic: the server sends no open-confirmation, so the tab
                // appears immediately and `on_open_channel_wire` tells the server.
                commands.trigger(OpenChannel {
                    config: config.clone(),
                });
                commands.entity(dialog).despawn();
            })
            .id();
        commands.entity(list).add_child(row);
    }

    let prompt = commands
        .spawn((
            Text::new("Private channel:"),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
        ))
        .id();

    let field = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(conf::FIELD_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
        ))
        .with_child((
            PrivateNameField,
            TextInputNode {
                mode: TextInputMode::SingleLine,
                clear_on_submit: false,
                ..default()
            },
            TextInputContents::default(),
            TextInputPrompt {
                text: String::new(),
                ..default()
            },
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextColor(chat_conf::TAB_TITLE_COLOR.into()),
        ))
        .id();

    commands
        .entity(handle.content)
        .add_children(&[list, prompt, field]);
}

pub fn on_channels_dialog_button(
    event: On<DialogButtonPressed>,
    dialogs: Query<(), With<ChannelsDialog>>,
    field_q: Query<&TextInputContents, With<PrivateNameField>>,
    mut commands: Commands,
) {
    if dialogs.get(event.dialog).is_err() {
        return;
    }
    if event.button == DialogButtonId::Ok
        && let Ok(contents) = field_q.single()
    {
        let name = contents.get().trim().to_owned();
        if !name.is_empty() {
            commands.trigger(SendMessage(ClientMessage::OpenPmChat { name }));
        }
    }
    commands.entity(event.dialog).despawn();
}
