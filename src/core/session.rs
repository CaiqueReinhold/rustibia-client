use bevy::prelude::*;

use crate::core::spells::{SpellBook, SpellCooldowns};
use crate::core::systems::{PingState, PingTimer};

/// Every system that tears down a game session runs here, on
/// `OnExit(GameState::InGame)`.
///
/// Each plugin registers its own `cleanup_session` into this set and resets only
/// state it owns — nothing has to be made public for a central teardown module to
/// reach it, and state added later is cleaned up in the file that introduced it.
/// Members are order-independent *with respect to each other*: by the time they
/// run the message pump is gone and every `InGame`-gated system has stopped, so
/// no member's cleanup can race another's. That does not mean no observer ever
/// fires here — a member's own commands can still trigger one synchronously, the
/// way `effects::cleanup_session`'s despawn triggers `on_remove_effect`; such an
/// observer only ever touches state the same member owns, which is why the
/// between-members guarantee still holds despite it.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionCleanup;

/// Why the session ended. This only decides whether the player was shown a modal
/// on the way out — the cleanup itself is identical either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEndReason {
    /// The player pressed Logout.
    Logout,
    /// The connection dropped and the player dismissed the notice.
    Disconnected,
}

/// The single entry point to teardown. Triggering this is what sends the client
/// back to the character list; nothing else may set `GameState::LoginScreen` from
/// in-game.
#[derive(Event, Debug)]
pub struct EndGameSession {
    pub reason: SessionEndReason,
}

/// Present from the moment the "Connection Lost" modal appears until the cleanup
/// runs. The world is still on screen during that window, so local input has to be
/// gated off it — see the `run_if`s in `PlayerPlugin`.
#[derive(Resource, Debug)]
pub struct SessionEnding;

/// The character chosen on the character list. `id` is the site's character id, not an `AgentId`.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveCharacter {
    pub id: u32,
}

pub(super) fn cleanup_session(mut commands: Commands, ping: Res<PingState>) {
    ping.reset();
    commands.insert_resource(PingTimer::default());
    commands.insert_resource(SpellBook::default());
    commands.insert_resource(SpellCooldowns::default());
    commands.remove_resource::<ActiveCharacter>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::systems::{PingState, PingTimer};
    use bevy::ecs::system::RunSystemOnce;
    use std::time::Duration;

    /// A ping reading is a property of the connection that just died. Leaving it
    /// behind means the next session's HUD shows the previous session's latency
    /// until the first Pong lands.
    #[test]
    fn cleanup_clears_the_last_ping_reading() {
        let mut world = World::new();
        let ping = PingState::default();
        ping.record(Duration::from_millis(42));
        world.insert_resource(ping);
        world.init_resource::<PingTimer>();

        world.run_system_once(cleanup_session).unwrap();

        assert_eq!(
            world.resource::<PingState>().current(),
            Duration::ZERO,
            "the stale reading must not survive into the next session"
        );
    }

    #[test]
    fn cleanup_forgets_the_character_and_its_spells() {
        use crate::core::spells::{SpellBook, SpellCooldowns, SpellGroup, SpellId, SpellInfo};

        let mut world = World::new();
        world.init_resource::<PingState>();
        world.init_resource::<PingTimer>();
        world.insert_resource(ActiveCharacter { id: 7 });
        world.insert_resource(SpellBook::new(vec![SpellInfo {
            id: SpellId(1),
            name: "Light Healing".to_owned(),
            words: "exura".to_owned(),
            level: 8,
            icon: 6,
            aimable: false,
            group: SpellGroup::Healing,
        }]));
        let mut cooldowns = SpellCooldowns::default();
        cooldowns.start_group(SpellGroup::Healing, Duration::ZERO, Duration::from_secs(1));
        world.insert_resource(cooldowns);

        world.run_system_once(cleanup_session).unwrap();

        assert!(world.get_resource::<ActiveCharacter>().is_none());
        assert!(world.resource::<SpellBook>().spells().is_none());
        assert!(
            world
                .resource::<SpellCooldowns>()
                .group(SpellGroup::Healing, Duration::ZERO)
                .is_none()
        );
    }
}
