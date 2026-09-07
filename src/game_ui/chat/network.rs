use bevy::prelude::*;

use crate::conf::ui::chat as conf;
use crate::core::{ChatMessageType, FloatingTextType};
use crate::game_ui::chat::events::{AppendChatMessage, CloseChannel, OpenChannel};
use crate::game_ui::chat::routing::channel_for;
use crate::game_ui::chat::state::{ChannelConfig, ChannelId, ChatMessage, ChatState};
use crate::network::events::{
    ChannelListReceived, ChatMessageReceived, PrivateChatOpened, ShowFloatingText,
};
use crate::network::{ClientMessage, SendMessage};

/// Server channels are static config, so the list is fetched once and cached.
pub fn request_channels(mut commands: Commands) {
    commands.trigger(SendMessage(ClientMessage::RequestChannels));
}

pub fn on_channel_list_received(event: On<ChannelListReceived>, mut state: ResMut<ChatState>) {
    state.available = event
        .channels
        .iter()
        .map(|(id, name)| ChannelConfig {
            id: ChannelId::Server(*id),
            name: name.clone(),
            closeable: true,
            text_color: Color::Srgba(conf::LOCAL_CHANNEL_COLOR),
        })
        .collect();
}

/// Opens the tab the player asked for. Unlike the introduction it replaces, this
/// message answers `CLI_OPEN_PM_CHAT` and nothing else, so it can open a tab outright
/// without mistaking a stranger's first message for a request.
pub fn on_private_chat_opened(
    event: On<PrivateChatOpened>,
    mut state: ResMut<ChatState>,
    mut commands: Commands,
) {
    let id = state.pm_tab(&event.name);
    commands.trigger(OpenChannel {
        config: ChannelConfig {
            id,
            name: event.name.clone(),
            closeable: true,
            text_color: Color::Srgba(conf::LOCAL_CHANNEL_COLOR),
        },
    });
}

pub fn on_chat_message_received(
    event: On<ChatMessageReceived>,
    mut state: ResMut<ChatState>,
    mut commands: Commands,
) {
    let channel_id = channel_for(&mut state, event.message_type, event.channel, &event.author);

    // A private message from someone we have no tab for opens one.
    if matches!(channel_id, ChannelId::Private(_)) && !state.is_open(channel_id) {
        commands.trigger(OpenChannel {
            config: ChannelConfig {
                id: channel_id,
                name: event.author.clone(),
                closeable: true,
                text_color: Color::Srgba(conf::LOCAL_CHANNEL_COLOR),
            },
        });
    } else if !state.is_open(channel_id) {
        // A channel message for a tab we already closed — a race against
        // CLI_CLOSE_CHANNEL. Drop it.
        return;
    }

    if matches!(event.message_type, ChatMessageType::Local)
        && let Some(position) = event.position.clone()
    {
        commands.trigger(ShowFloatingText {
            text: event.text.clone(),
            position,
            text_type: FloatingTextType::PlayerMessage,
            color: None,
            speaker: Some(event.author.clone()),
        });
    }

    commands.trigger(AppendChatMessage {
        message: ChatMessage {
            text: event.text.clone(),
            channel_id: Some(channel_id),
            author: Some(event.author.clone()),
        },
    });
}

/// Wire sends live on the state events rather than the click sites: there is no case
/// where a `Server` channel is opened or closed locally without telling the server, and
/// `Local`/`Private` simply do not send.
pub fn on_open_channel_wire(event: On<OpenChannel>, mut commands: Commands) {
    if let ChannelId::Server(id) = event.config.id {
        commands.trigger(SendMessage(ClientMessage::OpenChannel { channel: id }));
    }
}

pub fn on_close_channel_wire(event: On<CloseChannel>, mut commands: Commands) {
    if let ChannelId::Server(id) = event.channel_id {
        commands.trigger(SendMessage(ClientMessage::CloseChannel { channel: id }));
    }
}
