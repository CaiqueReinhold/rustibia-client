use bevy::prelude::*;

use crate::{
    core::{GameState, SessionCleanup},
    network::systems::ConnectionState,
};

pub mod events;
pub mod login;
mod messages;
mod session;
mod systems;
mod tls;

pub use messages::{ClientMessage, ItemStack, ServerMessage};
pub use systems::{LoginCredentials, LogoutRequested, RequestLogout, SendMessage};
pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(systems::on_connect)
            .add_observer(systems::on_send_message)
            .add_observer(systems::on_login_error_cleanup)
            .add_observer(systems::on_connection_lost_cleanup)
            .add_observer(systems::on_client_outdated)
            .add_observer(systems::on_request_logout)
            .add_observer(login::on_fetch_characters)
            .add_observer(login::on_generate_game_token)
            .add_systems(
                Update,
                systems::receive_messages.run_if(resource_exists::<ConnectionState>),
            )
            .add_systems(
                Update,
                (login::pool_login_task, login::pool_generate_game_token)
                    .run_if(in_state(GameState::LoginScreen)),
            )
            .add_systems(OnEnter(GameState::Connecting), systems::connect)
            .add_systems(
                OnExit(GameState::InGame),
                session::cleanup_session.in_set(SessionCleanup),
            );
    }
}
