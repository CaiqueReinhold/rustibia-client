use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use super::types::{SkillProgress, SkillType};
use crate::{
    agent::Agent,
    network::events::{
        AgentSpeedUpdated, ExperienceChanged, PlayerSkills, SkillChanged, SpawnPlayer,
    },
    player::components::Player,
};

#[derive(Resource, Default)]
pub struct SkillsState {
    pub experience: u64,
    pub speed: u16,
    pub skills: HashMap<SkillType, SkillProgress>,
}

pub fn on_player_skills(event: On<PlayerSkills>, mut state: ResMut<SkillsState>) {
    state.experience = event.experience;
    state.skills = event.skills.iter().copied().collect();
}

pub fn on_skill_changed(event: On<SkillChanged>, mut state: ResMut<SkillsState>) {
    state.skills.insert(event.skill, event.progress);
}

pub fn on_experience_changed(event: On<ExperienceChanged>, mut state: ResMut<SkillsState>) {
    state.experience = event.experience;
}

pub fn on_player_spawned(event: On<SpawnPlayer>, mut state: ResMut<SkillsState>) {
    state.speed = event.speed;
}

pub fn on_speed_changed(
    event: On<AgentSpeedUpdated>,
    mut state: ResMut<SkillsState>,
    player: Single<&Agent, With<Player>>,
) {
    if event.agent_id == player.agent_id {
        state.speed = event.speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world_with_state() -> World {
        let mut world = World::new();
        world.init_resource::<SkillsState>();
        world.add_observer(on_player_skills);
        world.add_observer(on_skill_changed);
        world.add_observer(on_experience_changed);
        world
    }

    #[test]
    fn a_snapshot_replaces_everything() {
        let mut world = world_with_state();
        world.resource_mut::<SkillsState>().skills.insert(
            SkillType::Axe,
            SkillProgress {
                level: 99,
                percent_bp: 1,
            },
        );

        world.trigger(PlayerSkills {
            experience: 4231,
            skills: vec![(
                SkillType::Sword,
                SkillProgress {
                    level: 12,
                    percent_bp: 4909,
                },
            )],
        });

        let state = world.resource::<SkillsState>();
        assert_eq!(state.experience, 4231);
        assert_eq!(state.skills.len(), 1);
        assert_eq!(state.skills[&SkillType::Sword].level, 12);
    }

    #[test]
    fn a_delta_patches_one_skill_and_leaves_the_others() {
        let mut world = world_with_state();
        world.trigger(PlayerSkills {
            experience: 4231,
            skills: vec![
                (
                    SkillType::Sword,
                    SkillProgress {
                        level: 12,
                        percent_bp: 4909,
                    },
                ),
                (
                    SkillType::Axe,
                    SkillProgress {
                        level: 10,
                        percent_bp: 0,
                    },
                ),
            ],
        });

        world.trigger(SkillChanged {
            skill: SkillType::Sword,
            progress: SkillProgress {
                level: 13,
                percent_bp: 0,
            },
        });

        let state = world.resource::<SkillsState>();
        assert_eq!(state.skills[&SkillType::Sword].level, 13);
        assert_eq!(state.skills[&SkillType::Axe].level, 10);
        assert_eq!(
            state.experience, 4231,
            "a skill delta carries no experience"
        );
    }

    #[test]
    fn experience_arrives_on_its_own_message() {
        let mut world = world_with_state();

        world.trigger(ExperienceChanged { experience: 4255 });

        assert_eq!(world.resource::<SkillsState>().experience, 4255);
    }

    /// A snapshot is authoritative: it replaces the map rather than merging into
    /// it, so a row the server stopped sending stops being drawn.
    #[test]
    fn an_empty_snapshot_clears_what_was_there() {
        let mut world = world_with_state();
        world.trigger(PlayerSkills {
            experience: 4231,
            skills: vec![(
                SkillType::Sword,
                SkillProgress {
                    level: 12,
                    percent_bp: 4909,
                },
            )],
        });

        world.trigger(PlayerSkills {
            experience: 0,
            skills: Vec::new(),
        });

        let state = world.resource::<SkillsState>();
        assert!(state.skills.is_empty());
        assert_eq!(state.experience, 0);
    }

    #[test]
    fn a_delta_for_an_unknown_skill_adds_it() {
        let mut world = world_with_state();

        world.trigger(SkillChanged {
            skill: SkillType::Shielding,
            progress: SkillProgress {
                level: 11,
                percent_bp: 100,
            },
        });

        let state = world.resource::<SkillsState>();
        assert_eq!(state.skills[&SkillType::Shielding].level, 11);
    }
}
