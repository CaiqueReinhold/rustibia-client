use crate::core::{ChatMessageType, SayTarget};
use crate::game_ui::chat::state::{ChannelId, ChatState};

/// Which tab an inbound message belongs in.
///
/// Note the asymmetry: a channel message is keyed by `channel` and a private message by
/// its author. A private message carries channel 0, so keying it by channel would funnel
/// every conversation into one tab.
pub fn channel_for(
    state: &mut ChatState,
    message_type: ChatMessageType,
    channel: u16,
    author: &str,
) -> ChannelId {
    match message_type {
        ChatMessageType::Local => ChannelId::Local,
        ChatMessageType::Channel => ChannelId::Server(channel),
        ChatMessageType::Private => state.pm_tab(author),
    }
}

/// What to put on the wire for a message typed into `id`, and whether the client must
/// render its own copy.
pub struct Outbound {
    pub target: SayTarget,
    /// True only for private messages. The server echoes local speech (it fans out
    /// with `originator: None`) and channel messages (the sender is a member), but
    /// delivers a private message solely to the recipient.
    pub echo: bool,
}

/// `None` for a private tab whose name this client has somehow lost — there is nothing
/// to address the message to, and a message sent to the wrong person is worse than one
/// that does not send.
pub fn outbound_for(state: &ChatState, id: ChannelId) -> Option<Outbound> {
    match id {
        ChannelId::Local => Some(Outbound {
            target: SayTarget::Local,
            echo: false,
        }),
        ChannelId::Server(channel) => Some(Outbound {
            target: SayTarget::Channel(channel),
            echo: false,
        }),
        ChannelId::Private(_) => state.pm_name(id).map(|name| Outbound {
            target: SayTarget::Player(name.to_owned()),
            echo: true,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_speech_routes_to_the_local_tab() {
        let mut state = ChatState::default();
        assert_eq!(
            channel_for(&mut state, ChatMessageType::Local, 0, "Rizael"),
            ChannelId::Local
        );
    }

    #[test]
    fn a_channel_message_routes_by_channel_not_author() {
        let mut state = ChatState::default();
        assert_eq!(
            channel_for(&mut state, ChatMessageType::Channel, 3, "Rizael"),
            ChannelId::Server(3)
        );
    }

    /// A private conversation is keyed by the *other party*, which for an inbound
    /// message is the author — not the channel field, which is 0.
    #[test]
    fn a_private_message_routes_by_author_not_channel() {
        let mut state = ChatState::default();
        let tab = channel_for(&mut state, ChatMessageType::Private, 0, "Rizael");
        assert!(matches!(tab, ChannelId::Private(_)));
    }

    /// The tab id is minted per correspondent, so two people are two tabs and the same
    /// person twice is one — however their name was capitalised.
    #[test]
    fn a_correspondent_keeps_one_tab_and_two_do_not_share() {
        let mut state = ChatState::default();
        let first = channel_for(&mut state, ChatMessageType::Private, 0, "Rizael");
        let other = channel_for(&mut state, ChatMessageType::Private, 0, "Kestrel");
        let again = channel_for(&mut state, ChatMessageType::Private, 0, "rizael");

        assert_ne!(first, other);
        assert_eq!(first, again);
    }

    #[test]
    fn local_is_sent_without_echo() {
        let state = ChatState::default();
        let out = outbound_for(&state, ChannelId::Local).expect("local always sends");
        assert_eq!(out.target, SayTarget::Local);
        assert!(
            !out.echo,
            "the server echoes local speech back to the speaker"
        );
    }

    #[test]
    fn a_channel_is_sent_without_echo() {
        let state = ChatState::default();
        let out = outbound_for(&state, ChannelId::Server(4)).expect("a channel always sends");
        assert_eq!(out.target, SayTarget::Channel(4));
        assert!(!out.echo, "the sender is a member, so the server echoes");
    }

    /// The one case the server does not echo: it delivers a private message only to
    /// the recipient, so the sender must render its own copy. The wire carries the
    /// name, so the tab id never leaves this client.
    #[test]
    fn a_private_message_is_addressed_by_name_and_echoed_locally() {
        let mut state = ChatState::default();
        let tab = state.pm_tab("Rizael");

        let out = outbound_for(&state, tab).expect("a named correspondent sends");
        assert_eq!(out.target, SayTarget::Player("Rizael".to_owned()));
        assert!(
            out.echo,
            "the server delivers private messages only to the recipient"
        );
    }

    /// A tab id with no name behind it cannot be addressed, and guessing would send
    /// the message to whoever holds that index instead.
    #[test]
    fn a_private_tab_with_no_name_does_not_send() {
        let state = ChatState::default();
        assert!(outbound_for(&state, ChannelId::Private(9)).is_none());
    }
}
