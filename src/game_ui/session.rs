use bevy::prelude::*;

use crate::core::SessionEnding;
use crate::game_ui::chat::{ChatMode, ChatState};
use crate::game_ui::modal::ModalOrder;
use crate::game_ui::skills::SkillsState;
use crate::game_ui::toppanel::BarEntities;
use crate::game_ui::window::CurrentDockHover;

/// Resets the UI state that outlives its entities.
///
/// The entities themselves — the whole `MainUI` tree and every modal root — carry
/// `DespawnOnExit(GameState::InGame)` and are collected by Bevy's own sweep in the
/// same transition, so nothing is despawned here. What is left is the state that
/// pointed at them.
pub(super) fn cleanup_session(mut commands: Commands) {
    commands.insert_resource(ChatState::default());
    commands.insert_resource(ChatMode::default());
    commands.insert_resource(ModalOrder::default());
    commands.insert_resource(CurrentDockHover::default());
    commands.insert_resource(SkillsState::default());
    commands.remove_resource::<BarEntities>();
    commands.remove_resource::<SessionEnding>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn seeded_world() -> World {
        let mut world = World::new();
        world.init_resource::<ChatState>();
        world.init_resource::<ChatMode>();
        world.init_resource::<ModalOrder>();
        world.init_resource::<CurrentDockHover>();
        world.init_resource::<SkillsState>();
        world
    }

    /// Chat is per character: the channels the server offered and the private tabs
    /// minted for correspondents both describe the session that just ended.
    #[test]
    fn cleanup_empties_the_chat_state() {
        let mut world = seeded_world();
        {
            let mut state = world.resource_mut::<ChatState>();
            state.pm_tab("Rizael");
            state
                .available
                .push(crate::game_ui::chat::state::ChannelConfig {
                    id: crate::game_ui::chat::state::ChannelId::Server(7),
                    name: "Help".to_string(),
                    closeable: true,
                    text_color: Color::WHITE,
                });
        }
        world.resource_mut::<ChatMode>().active = true;

        world.run_system_once(cleanup_session).unwrap();

        let state = world.resource::<ChatState>();
        assert!(
            state
                .pm_name(crate::game_ui::chat::state::ChannelId::Private(0))
                .is_none()
        );
        assert!(state.available.is_empty());
        assert!(!world.resource::<ChatMode>().active);
    }

    /// `BarEntities` caches four entity ids from the top panel, which is despawned
    /// with `MainUI`. Defaulting it is not an option — it has no default — so the
    /// only correct handling is removal.
    #[test]
    fn cleanup_drops_the_dangling_bar_entities() {
        let mut world = seeded_world();
        let dead = world.spawn_empty().id();
        world.insert_resource(BarEntities {
            health_bar: dead,
            health_text: dead,
            mana_bar: dead,
            mana_text: dead,
        });

        world.run_system_once(cleanup_session).unwrap();

        assert!(world.get_resource::<BarEntities>().is_none());
    }

    /// Skills are per character. Leaving them behind would show the previous
    /// character's bars until the next login's snapshot arrived.
    #[test]
    fn cleanup_empties_the_skills_state() {
        let mut world = seeded_world();
        {
            let mut state = world.resource_mut::<SkillsState>();
            state.experience = 4231;
            state.skills.insert(
                crate::game_ui::skills::SkillType::Sword,
                crate::game_ui::skills::SkillProgress {
                    level: 12,
                    percent_bp: 4909,
                },
            );
        }

        world.run_system_once(cleanup_session).unwrap();

        let state = world.resource::<SkillsState>();
        assert_eq!(state.experience, 0);
        assert!(state.skills.is_empty());
    }
}
