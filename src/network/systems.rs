use async_channel::{Receiver, Sender, bounded};
use async_net::TcpStream;
use asynchronous_codec::Framed;
use bevy::log::info;
use bevy::{prelude::*, tasks::IoTaskPool};
use futures::{FutureExt, SinkExt, StreamExt};
use std::io;
use std::time::{Duration, Instant};

use crate::{
    config,
    core::{EndGameSession, GameState, PingState, SessionEndReason},
    network::{
        events,
        messages::{ClientMessage, GameMessageCodec, ServerMessage},
    },
};

#[derive(Event, Debug)]
pub struct Connect {
    pub auth_token: String,
}

#[derive(Event, Debug)]
pub struct SendMessage(pub ClientMessage);

#[derive(Resource, Debug)]
pub struct LoginCredentials {
    pub auth_token: String,
}

/// How long to wait for the server to close the socket after a logout before
/// tearing down anyway. The protocol has no reply to `Logout`, so this is the only
/// backstop.
const LOGOUT_TIMEOUT: Duration = Duration::from_secs(2);

/// Asked for by the UI; acted on here so the protocol stays behind the network
/// boundary.
#[derive(Event, Debug)]
pub struct RequestLogout;

/// Present from the moment the player asks to log out until teardown. Its presence
/// is what tells the connection-lost handler that the drop was intentional and needs
/// no modal.
#[derive(Resource, Debug)]
pub struct LogoutRequested {
    timer: Timer,
}

#[derive(Resource, Debug)]
pub struct ConnectionState {
    startup_messages: Option<Vec<ServerMessage>>,
    sender: Sender<ClientMessage>,
    receiver: Receiver<ServerMessage>,
}

/// Runs on `OnEnter(GameState::Connecting)`. `Connecting` is only reachable
/// via the character list's confirm handler, which inserts
/// [`LoginCredentials`] in the same command batch — so the `Res` here is
/// guaranteed present. Keep that invariant if adding new paths into
/// `Connecting` (e.g. a reconnect feature).
pub fn connect(mut commands: Commands, credentials: Res<LoginCredentials>) {
    commands.trigger(Connect {
        auth_token: credentials.auth_token.clone(),
    });
}

pub(super) fn on_connect(event: On<Connect>, mut commands: Commands, ping: Res<PingState>) {
    let (cli_send, cli_recv) = bounded(5);
    let (srv_send, srv_recv) = bounded(5);
    // Cloning the handle, not the value: the task writes each round trip into the
    // shared atomic behind it, which the UI reads without either side blocking.
    let ping = ping.clone();

    IoTaskPool::get()
        .spawn(async move {
            let conn =
                PersistentConnection::new(&config::CONFIG.server_address, srv_send, cli_recv, ping)
                    .await;
            if let Ok(conn) = conn {
                return conn.run().await;
            }
            Err(conn.err().unwrap())
        })
        .detach();

    if cli_send
        .send_blocking(ClientMessage::Login {
            auth_token: event.auth_token.clone(),
        })
        .is_err()
    {
        error!("Connection failed");
    };

    commands.insert_resource(ConnectionState {
        startup_messages: Some(Vec::new()),
        sender: cli_send,
        receiver: srv_recv,
    });
}

pub(super) fn receive_messages(mut commands: Commands, mut connection: ResMut<ConnectionState>) {
    // The async task drops its sender when the TCP connection fails or closes.
    // Without this check the client hangs in Connecting for ever when the
    // server is unreachable.
    if connection.receiver.is_closed() && connection.receiver.is_empty() {
        commands.trigger(events::ConnectionLost);
        return;
    }

    if connection.startup_messages.is_some() {
        while let Ok(msg) = connection.receiver.try_recv() {
            // A login rejection must route immediately — buffering it until
            // DescribePlayer would swallow it forever (DescribePlayer never
            // comes after a rejection).
            if matches!(msg, ServerMessage::LoginError) {
                events::route_event(msg, &mut commands);
                return;
            }
            if let ServerMessage::DescribePlayer { .. } = msg {
                events::route_event(msg, &mut commands);
                for start_msg in connection.startup_messages.as_mut().unwrap().drain(..) {
                    events::route_event(start_msg, &mut commands);
                }
                connection.startup_messages = None;
                return;
            }
            connection.startup_messages.as_mut().unwrap().push(msg);
        }
        return;
    }
    while let Ok(msg) = connection.receiver.try_recv() {
        events::route_event(msg, &mut commands);
    }
}

/// Dropping ConnectionState drops the client-message sender, which ends
/// the async task's select loop and closes the TCP stream.
pub(super) fn on_login_error_cleanup(
    _: On<events::LoginError>,
    state: Res<State<GameState>>,
    mut commands: Commands,
) {
    if *state.get() == GameState::Connecting {
        commands.remove_resource::<ConnectionState>();
    }
}

pub(super) fn on_connection_lost_cleanup(_: On<events::ConnectionLost>, mut commands: Commands) {
    commands.remove_resource::<ConnectionState>();
}

pub(super) fn on_client_outdated(_: On<events::ClientOutdated>, mut commands: Commands) {
    error!("Client is outdated: the server sent game data this client does not have");
    commands.remove_resource::<ConnectionState>();
}

pub(super) fn on_send_message(event: On<SendMessage>, connection: Option<Res<ConnectionState>>) {
    if connection.is_none() {
        return;
    }

    let connection = connection.unwrap();
    if let Err(e) = connection.sender.send_blocking(event.0.clone()) {
        error!("Error sending client message: {e}");
    };
}

pub(super) fn on_request_logout(
    _: On<RequestLogout>,
    mut commands: Commands,
    pending: Option<Res<LogoutRequested>>,
    connection: Option<Res<ConnectionState>>,
) {
    // Repeat clicks, and clicks after the connection has already gone, are noise.
    if pending.is_some() || connection.is_none() {
        return;
    }
    commands.trigger(SendMessage(ClientMessage::Logout));
    commands.insert_resource(LogoutRequested {
        timer: Timer::new(LOGOUT_TIMEOUT, TimerMode::Once),
    });
}

pub(super) fn tick_logout_timeout(
    mut commands: Commands,
    pending: Option<ResMut<LogoutRequested>>,
    time: Res<Time>,
) {
    let Some(mut pending) = pending else {
        return;
    };
    if pending.timer.tick(time.delta()).just_finished() {
        warn!("logout: the server never closed the connection, tearing down anyway");
        commands.trigger(EndGameSession {
            reason: SessionEndReason::Logout,
        });
    }
}

pub struct PersistentConnection {
    stream: Framed<TcpStream, GameMessageCodec>,
    sender: Sender<ServerMessage>,
    receiver: Receiver<ClientMessage>,
    ping: PingState,
}

impl PersistentConnection {
    pub async fn new(
        server_addr: &str,
        sender: Sender<ServerMessage>,
        receiver: Receiver<ClientMessage>,
        ping: PingState,
    ) -> Result<Self, io::Error> {
        let stream = TcpStream::connect(server_addr).await?;
        // Movement frames are a few bytes each. Nagle would hold them until the
        // previous segment is acknowledged, adding up to a round trip of variable
        // delay to every walk — which the server's walk cooldown has no margin for.
        if let Err(e) = stream.set_nodelay(true) {
            warn!("could not disable Nagle: {e}");
        }
        let stream = Framed::new(stream, GameMessageCodec {});
        Ok(Self {
            stream,
            sender,
            receiver,
            ping,
        })
    }

    pub async fn run(mut self) -> Result<(), io::Error> {
        // Both ends of the round trip are timed here rather than in Bevy, so the
        // sample spans the socket write to the frame decode and nothing else. A
        // reading taken in the schedule would carry a command flush outbound and a
        // frame boundary inbound — see `PingState`.
        let mut ping_sent_at: Option<Instant> = None;

        loop {
            futures::select! {
                msg = self.receiver.recv().fuse() => {
                    match msg {
                        Ok(ClientMessage::Ping) => {
                            // Stamped as late as possible: everything after this is
                            // encode plus the write syscall, which is microseconds.
                            ping_sent_at = Some(Instant::now());
                            self.stream.send(ClientMessage::Ping).await?;
                        }
                        Ok(msg) => {
                            info!("sending msg: {:?}", msg);
                            self.stream.send(msg).await?;
                        }
                        Err(_) => break,
                    }
                },
                msg = self.stream.next().fuse() => {
                    match msg {
                        // Answered entirely in here; nothing downstream consumes it.
                        Some(Ok(ServerMessage::Pong)) => {
                            if let Some(sent_at) = ping_sent_at.take() {
                                self.ping.record(sent_at.elapsed());
                            }
                        }
                        Some(Ok(msg)) => {
                            info!("receiveing msg: {}", msg);
                            if self.sender.send(msg).await.is_err() {
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            error!("Error reading from server: {e}");
                            break;
                        }
                        None => break,
                    }
                }
            };
        }

        info!("loop ended");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentId;
    use crate::agent::{FacingDirection, Health, Mana};
    use crate::core::OutfitColors;
    use crate::core::OutfitId;
    use crate::core::{EndGameSession, SessionEndReason};
    use crate::items::ContainerId;
    use crate::map::Position;
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    #[derive(Resource, Default)]
    struct Routed(Vec<&'static str>);

    fn describe_player() -> ServerMessage {
        ServerMessage::DescribePlayer {
            agent_id: AgentId(1),
            position: Position { x: 0, y: 0, z: 7 },
            facing: FacingDirection::South,
            name: "Rizael".to_string(),
            level: 1,
            health: Health { current: 1, max: 1 },
            mana: Mana { current: 1, max: 1 },
            outfit: (OutfitId(128), OutfitColors::default()),
            speed: 100,
            capacity: 0,
            inventory_head: None,
            inventory_amulet: None,
            inventory_backpack: None,
            inventory_chest: None,
            inventory_right_hand: None,
            inventory_left_hand: None,
            inventory_legs: None,
            inventory_feet: None,
            inventory_ring: None,
            inventory_trinket: None,
        }
    }

    /// The buffer exists because the world can't be built before the player is
    /// described — but the server's own order still has to survive the wait, or
    /// the client applies, say, a SpawnAgent before the map it stands on.
    #[test]
    fn buffered_startup_messages_replay_in_the_order_they_arrived() {
        let mut world = World::new();
        world.init_resource::<Routed>();
        world.add_observer(|_: On<events::SpawnPlayer>, mut r: ResMut<Routed>| r.0.push("player"));
        world.add_observer(|_: On<events::PlayerPosition>, mut r: ResMut<Routed>| {
            r.0.push("position")
        });
        world.add_observer(|_: On<events::ContainerClosed>, mut r: ResMut<Routed>| {
            r.0.push("container")
        });

        let (cli_send, _cli_recv) = bounded::<ClientMessage>(5);
        let (srv_send, srv_recv) = bounded(5);
        srv_send
            .send_blocking(ServerMessage::PlayerPosition {
                position: Position { x: 1, y: 2, z: 7 },
            })
            .unwrap();
        srv_send
            .send_blocking(ServerMessage::ContainerClosed {
                container_id: ContainerId(3),
            })
            .unwrap();
        srv_send.send_blocking(describe_player()).unwrap();

        world.insert_resource(ConnectionState {
            startup_messages: Some(Vec::new()),
            sender: cli_send,
            receiver: srv_recv,
        });

        world.run_system_once(receive_messages).unwrap();

        assert_eq!(
            world.resource::<Routed>().0,
            vec!["player", "position", "container"],
            "DescribePlayer routes first, then the buffer in arrival order"
        );
    }

    #[derive(Resource, Default)]
    struct EndedWith(Option<SessionEndReason>);

    fn world_watching_for_session_end() -> World {
        let mut world = World::new();
        world.init_resource::<EndedWith>();
        world.add_observer(|e: On<EndGameSession>, mut r: ResMut<EndedWith>| {
            r.0 = Some(e.reason);
        });
        world
    }

    /// The server closing the socket is what normally ends a logout. If it accepts
    /// the request and never closes, nothing else would ever fire — `receive_messages`
    /// stops running the moment `ConnectionState` is gone — so the timeout has to end
    /// the session itself.
    #[test]
    fn the_logout_timeout_ends_the_session_on_its_own() {
        let mut world = world_watching_for_session_end();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(LogoutRequested {
            timer: Timer::new(LOGOUT_TIMEOUT, TimerMode::Once),
        });

        world
            .resource_mut::<Time<()>>()
            .advance_by(LOGOUT_TIMEOUT + Duration::from_millis(1));
        world.run_system_once(tick_logout_timeout).unwrap();
        world.flush();

        assert_eq!(
            world.resource::<EndedWith>().0,
            Some(SessionEndReason::Logout)
        );
    }

    /// Before the timeout there is nothing to do — the socket may still close
    /// normally at any moment.
    #[test]
    fn the_logout_timeout_waits_for_the_server_first() {
        let mut world = world_watching_for_session_end();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(LogoutRequested {
            timer: Timer::new(LOGOUT_TIMEOUT, TimerMode::Once),
        });

        world
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_millis(100));
        world.run_system_once(tick_logout_timeout).unwrap();
        world.flush();

        assert_eq!(world.resource::<EndedWith>().0, None);
    }
}
