use bevy::ecs::message::MessageReader;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy_ui_text_input::actions::{TextInputAction, TextInputEdit};
use bevy_ui_text_input::{
    SubmitText, TextInputContents, TextInputMode, TextInputNode, TextInputPrompt, TextInputQueue,
};

use crate::conf::ui::{chat as conf, ui_colors};
use crate::game_ui::GameUiAssets;
use crate::game_ui::chat::events::{ExitChatMode, SubmitChatInput};
use crate::game_ui::chat::state::{ChatMessage, ChatMode, ChatState};

#[derive(Component)]
pub struct ChatInputBar;

#[derive(Component)]
pub struct ChatInputField;

pub fn spawn_input(commands: &mut Commands, ui_assets: &GameUiAssets) -> Entity {
    let bar = commands
        .spawn((
            ChatInputBar,
            Node {
                width: Val::Percent(100.0),
                border: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(0.0), Val::Px(2.0)),
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

    let bar_bg = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(3.0)),
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

    commands.entity(bar).add_child(bar_bg);

    let field = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(conf::INPUT_HEIGHT),
                padding: UiRect::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(conf::INPUT_BG_COLOR.into()),
        ))
        .with_child((
            ChatInputField,
            TextInputNode {
                mode: TextInputMode::SingleLine,
                clear_on_submit: true,
                is_enabled: false,
                max_chars: Some(conf::MAX_MESSAGE_LENGTH),
                ..default()
            },
            TextInputContents::default(),
            TextInputPrompt {
                text: "Type a message...".to_string(),
                color: Some(conf::INPUT_PLACEHOLDER_COLOR.into()),
                ..default()
            },
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 12.0,
                ..default()
            },
            TextColor(conf::TAB_TITLE_COLOR.into()),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(conf::INPUT_HEIGHT - 8.0),
                ..default()
            },
        ))
        .id();
    commands.entity(bar_bg).add_child(field);
    bar
}

pub fn keyboard_exit_chat_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    chat_mode: Res<ChatMode>,
    mut field_q: Query<&mut TextInputQueue, With<ChatInputField>>,
    mut commands: Commands,
) {
    if !chat_mode.active {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        // Clear the field by queuing SelectAll + Delete
        if let Ok(mut queue) = field_q.single_mut() {
            queue.add(TextInputAction::Edit(TextInputEdit::SelectAll));
            queue.add(TextInputAction::Edit(TextInputEdit::Delete));
        }
        commands.trigger(ExitChatMode);
    }
}

/// React to `bevy_ui_text_input::SubmitText` (fired on Enter while focused)
/// — emit `SubmitChatInput` and exit chat mode.
///
/// `SubmitText` is a global message, not a per-entity one, so every focused text
/// field in the game raises it — including the channels dialog's player-name field.
/// Without the entity guard, pressing Enter there would speak the typed name into
/// whatever chat tab happened to be active.
pub fn on_text_input_submit(
    mut events: MessageReader<SubmitText>,
    field_q: Query<Entity, With<ChatInputField>>,
    mut commands: Commands,
) {
    let Ok(chat_field) = field_q.single() else {
        return;
    };
    for event in events.read() {
        if event.entity != chat_field {
            continue;
        }
        let text = event.text.trim().to_string();
        if !text.is_empty() {
            commands.trigger(SubmitChatInput { text });
        }
        commands.trigger(ExitChatMode);
    }
}

/// Toggle the input field's enabled state and focus when chat mode flips.
pub fn on_chat_mode_changed(
    mode: Res<ChatMode>,
    mut field_q: Query<(Entity, &mut TextInputNode), With<ChatInputField>>,
    mut input_focus: ResMut<InputFocus>,
) {
    if !mode.is_changed() {
        return;
    }
    if let Ok((entity, mut node)) = field_q.single_mut() {
        node.is_enabled = mode.active;
        if mode.active {
            input_focus.set(entity);
        } else {
            input_focus.clear();
        }
    }
}

/// Sends what was typed to the server, and renders a local copy only when the server
/// will not echo one back. See `routing::outbound_for`.
pub fn on_submit_chat_input(
    event: On<SubmitChatInput>,
    state: Res<ChatState>,
    mut commands: Commands,
) {
    use crate::game_ui::chat::events::AppendChatMessage;
    use crate::game_ui::chat::routing::outbound_for;
    use crate::network::{ClientMessage, SendMessage};

    let active = state.active;
    let Some(out) = outbound_for(&state, active) else {
        return;
    };

    commands.trigger(SendMessage(ClientMessage::Say {
        message: event.text.clone(),
        target: out.target,
    }));

    if out.echo {
        commands.trigger(AppendChatMessage {
            message: ChatMessage {
                text: event.text.clone(),
                channel_id: Some(active),
                author: None,
            },
        });
    }
}
