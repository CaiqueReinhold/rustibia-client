use bevy::input_focus::InputFocus;
use bevy::prelude::*;

use crate::core::{EndGameSession, GameState, SessionEndReason, SessionEnding};
use crate::game_ui::GameUiAssets;
use crate::game_ui::chat::events::ExitChatMode;
use crate::game_ui::death::DeathModal;
use crate::game_ui::modal::{DialogButtonPressed, ModalDialog, ModalOrder};
use crate::network::LogoutRequested;
use crate::network::events::ConnectionLost;

#[derive(Component)]
pub(super) struct DisconnectedModal;

/// What a `ConnectionLost` means, given the state around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConnectionLostAction {
    /// Not ours: no session was running, or a session-ending notice is already up.
    Ignore,
    /// The player asked for this — tear down without a word.
    EndNow,
    /// Tell the player and wait for them to dismiss it.
    Notify,
}

/// The whole decision, as a pure function so it can be tested on its own.
pub(super) fn action_for_connection_lost(
    state: &GameState,
    logout_requested: bool,
    session_ending_modal_open: bool,
) -> ConnectionLostAction {
    if *state != GameState::InGame || session_ending_modal_open {
        return ConnectionLostAction::Ignore;
    }
    if logout_requested {
        return ConnectionLostAction::EndNow;
    }
    ConnectionLostAction::Notify
}

/// Shows an unexpected drop over the world as it stood. Nothing is torn down yet —
/// the world stays rendered behind the modal, and `SessionEnding` gates the input
/// that would otherwise let the player walk around a dead map.
pub(super) fn on_connection_lost(
    _: On<ConnectionLost>,
    mut commands: Commands,
    state: Res<State<GameState>>,
    logout: Option<Res<LogoutRequested>>,
    existing: Query<(), Or<(With<DisconnectedModal>, With<DeathModal>)>>,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    mut input_focus: ResMut<InputFocus>,
) {
    match action_for_connection_lost(state.get(), logout.is_some(), !existing.is_empty()) {
        ConnectionLostAction::Ignore => {}
        ConnectionLostAction::EndNow => {
            commands.trigger(EndGameSession {
                reason: SessionEndReason::Logout,
            });
        }
        ConnectionLostAction::Notify => {
            let root = ModalDialog::message(
                "Connection Lost",
                "You have been disconnected from the game server.",
                &mut commands,
                &ui_assets,
                &mut order,
            );
            commands.entity(root).insert(DisconnectedModal);
            commands.insert_resource(SessionEnding);
            // Enter must reach the modal, not the chat bar the player may have been
            // typing in when the connection dropped.
            commands.trigger(ExitChatMode);
            input_focus.clear();
        }
    }
}

pub(super) fn on_dismiss(
    event: On<DialogButtonPressed>,
    modals: Query<(), With<DisconnectedModal>>,
    mut commands: Commands,
) {
    if !modals.contains(event.dialog) {
        return;
    }
    commands.trigger(EndGameSession {
        reason: SessionEndReason::Disconnected,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A logout ends the session the moment the socket closes: the player asked for
    /// it, so telling them it happened is noise.
    #[test]
    fn an_expected_disconnect_ends_the_session_silently() {
        assert_eq!(
            action_for_connection_lost(&GameState::InGame, true, false),
            ConnectionLostAction::EndNow
        );
    }

    /// An unexpected drop is shown over the world as it stood; nothing is torn down
    /// until the player dismisses it.
    #[test]
    fn an_unexpected_disconnect_notifies_first() {
        assert_eq!(
            action_for_connection_lost(&GameState::InGame, false, false),
            ConnectionLostAction::Notify
        );
    }

    /// A drop while connecting belongs to the login screen, which already reports it
    /// on the login form. This must not take that over.
    #[test]
    fn a_drop_before_the_session_started_is_not_ours() {
        assert_eq!(
            action_for_connection_lost(&GameState::Connecting, false, false),
            ConnectionLostAction::Ignore
        );
    }

    /// A drop while a session-ending notice — this one or the death notice — is up
    /// adds nothing.
    #[test]
    fn a_second_drop_does_not_stack_a_second_modal() {
        assert_eq!(
            action_for_connection_lost(&GameState::InGame, false, true),
            ConnectionLostAction::Ignore
        );
    }
}
