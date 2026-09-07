use std::collections::VecDeque;

use bevy::color::Color;
use bevy::ecs::resource::Resource;
use bevy::prelude::*;
use chrono::{DateTime, Local};

use crate::conf::ui::chat as conf;
use crate::game_ui::chat::events::{
    ActivateChannel, AppendChatMessage, ChannelClosedUi, ChannelOpenedUi, CloseChannel,
    EnterChatMode, ExitChatMode, MessageAppendedUi, MessageTrimmedUi, OpenChannel,
};

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ChannelId {
    /// Local speech — everything said in range. Always open, never closeable.
    Local,
    /// A server channel, keyed by the id from `MSG_CHANNEL_LIST`.
    Server(u16),
    /// A private conversation, keyed by an index this client mints into `pm_names`.
    /// The server never sees it — it addresses everyone by character name.
    Private(u16),
}

#[derive(Clone, Debug)]
pub struct ChannelConfig {
    pub id: ChannelId,
    pub name: String,
    pub closeable: bool,
    pub text_color: Color,
}

#[derive(Debug)]
pub struct ChatMessage {
    pub text: String,
    pub channel_id: Option<ChannelId>,
    /// Resolved at append time, not render time. `None` for client-generated lines.
    pub author: Option<String>,
}

#[derive(Debug)]
pub struct StoredMessage {
    pub timestamp: DateTime<Local>,
    pub sequence: u64,
    pub message: ChatMessage,
}

#[derive(Debug)]
pub struct Channel {
    pub config: ChannelConfig,
    pub messages: VecDeque<StoredMessage>,
    pub unread: bool,
    pub scroll_pinned_bottom: bool,
    pub next_sequence: u64,
}

impl Channel {
    pub fn new(config: ChannelConfig) -> Self {
        Self {
            config,
            messages: VecDeque::new(),
            unread: false,
            scroll_pinned_bottom: true,
            next_sequence: 0,
        }
    }
}

#[derive(Resource, Debug)]
pub struct ChatState {
    pub channels: Vec<Channel>,
    pub active: ChannelId,
    pub history_cap: usize,
    pub available: Vec<ChannelConfig>,
    /// Correspondent names, in the order first heard from. A private tab's
    /// `ChannelId::Private(i)` is an index into this — an id this client mints for
    /// itself, never seen by the server, which addresses everyone by name.
    ///
    /// Append-only, because the index is the tab's identity: removing an entry would
    /// renumber every tab after it. It is bounded by how many people one session ever
    /// converses with.
    pm_names: Vec<String>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            channels: vec![Channel::new(local_channel_config())],
            active: ChannelId::Local,
            history_cap: conf::HISTORY_CAP_DEFAULT,
            available: Vec::new(),
            pm_names: Vec::new(),
        }
    }
}

impl ChatState {
    pub fn channel(&self, id: ChannelId) -> Option<&Channel> {
        self.channels.iter().find(|c| c.config.id == id)
    }

    pub fn channel_mut(&mut self, id: ChannelId) -> Option<&mut Channel> {
        self.channels.iter_mut().find(|c| c.config.id == id)
    }

    pub fn is_open(&self, id: ChannelId) -> bool {
        self.channels.iter().any(|c| c.config.id == id)
    }

    /// The private tab id for `name`, minting one if this is the first we have heard
    /// of them. Matched case-insensitively, so a name typed in lower case and the same
    /// name as the server spells it share one tab.
    pub fn pm_tab(&mut self, name: &str) -> ChannelId {
        let existing = self
            .pm_names
            .iter()
            .position(|known| known.eq_ignore_ascii_case(name));

        let index = existing.unwrap_or_else(|| {
            self.pm_names.push(name.to_owned());
            self.pm_names.len() - 1
        });
        ChannelId::Private(index as u16)
    }

    pub fn pm_name(&self, id: ChannelId) -> Option<&str> {
        match id {
            ChannelId::Private(index) => self.pm_names.get(index as usize).map(String::as_str),
            _ => None,
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct ChatMode {
    pub active: bool,
}

pub fn local_channel_config() -> ChannelConfig {
    ChannelConfig {
        id: ChannelId::Local,
        name: conf::LOCAL_CHANNEL_NAME.to_owned(),
        closeable: false,
        text_color: Color::Srgba(conf::LOCAL_CHANNEL_COLOR),
    }
}

pub fn on_open_channel(
    event: On<OpenChannel>,
    mut state: ResMut<ChatState>,
    mut commands: Commands,
) {
    let id = event.config.id;
    if state.is_open(id) {
        // Already present: nothing for the strip to rebuild, so no ChannelOpenedUi.
        commands.trigger(ActivateChannel { channel_id: id });
        return;
    }
    state.channels.push(Channel::new(event.config.clone()));
    // Order matters: `ActivateChannel` updates `active` first so the rebuild that
    // follows spawns the new tab already styled as the active one.
    commands.trigger(ActivateChannel { channel_id: id });
    commands.trigger(ChannelOpenedUi);
}

pub fn on_close_channel(
    event: On<CloseChannel>,
    mut state: ResMut<ChatState>,
    mut commands: Commands,
) {
    let Some(channel) = state.channel(event.channel_id) else {
        return;
    };
    if !channel.config.closeable {
        return;
    }
    let was_active = state.active == event.channel_id;
    state.channels.retain(|c| c.config.id != event.channel_id);
    if was_active {
        commands.trigger(ActivateChannel {
            channel_id: ChannelId::Local,
        });
    }
    commands.trigger(ChannelClosedUi);
}

pub fn on_activate_channel(event: On<ActivateChannel>, mut state: ResMut<ChatState>) {
    if !state.is_open(event.channel_id) {
        return;
    }
    state.active = event.channel_id;
    if let Some(c) = state.channel_mut(event.channel_id) {
        c.unread = false;
        c.scroll_pinned_bottom = true;
    }
}

pub fn on_append_chat_message(
    event: On<AppendChatMessage>,
    mut state: ResMut<ChatState>,
    mut commands: Commands,
) {
    let cap = state.history_cap;
    let active = state.active;
    let target_ids: Vec<ChannelId> = match event.message.channel_id {
        Some(id) => vec![id],
        None => state.channels.iter().map(|c| c.config.id).collect(),
    };

    for id in target_ids {
        let Some(channel) = state.channel_mut(id) else {
            continue;
        };
        let sequence = channel.next_sequence;
        channel.next_sequence += 1;

        let stored = StoredMessage {
            timestamp: Local::now(),
            sequence,
            message: ChatMessage {
                text: event.message.text.clone(),
                channel_id: event.message.channel_id,
                author: event.message.author.clone(),
            },
        };

        channel.messages.push_back(stored);

        let mut trimmed: Option<u64> = None;
        if channel.messages.len() > cap
            && let Some(popped) = channel.messages.pop_front()
        {
            trimmed = Some(popped.sequence);
        }

        if id != active {
            channel.unread = true;
        }

        if let Some(seq) = trimmed {
            commands.trigger(MessageTrimmedUi {
                channel_id: id,
                sequence: seq,
            });
        }
        commands.trigger(MessageAppendedUi {
            channel_id: id,
            sequence,
        });
    }
}

pub fn on_enter_chat_mode(_event: On<EnterChatMode>, mut mode: ResMut<ChatMode>) {
    mode.active = true;
}

pub fn on_exit_chat_mode(_event: On<ExitChatMode>, mut mode: ResMut<ChatMode>) {
    mode.active = false;
}
