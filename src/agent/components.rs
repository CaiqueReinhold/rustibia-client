use std::sync::Arc;

use bevy::prelude::*;

use crate::conf::{
    agent::{DIAGONAL_STEP_FACTOR, SPEED_PARAM_A, SPEED_PARAM_B, SPEED_PARAM_C},
    server::TICK_DURATION_MS,
};
use crate::core::{OutfitColors, SpriteConfig};

/// An agent as this player's session names it. Session-local to the server, which reuses it — never a stable identity, and never comparable with a chat or container id.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct AgentId(pub u16);

impl Default for AgentId {
    /// Only for `Agent`'s `Default`, which exists for spawning before the id lands.
    fn default() -> Self {
        Self(0)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Mounted {
    #[default]
    Unmounted = 0,
    Mounted = 1,
}

impl From<Mounted> for u32 {
    fn from(value: Mounted) -> Self {
        value as u32
    }
}

#[derive(Component, Debug, Default)]
pub struct Agent {
    // pub outfit_id: u32,
    pub agent_id: AgentId,
    pub direction: FacingDirection,
    pub addons: u8,
    pub mounted: Mounted,
    pub outfit_colors: OutfitColors,
    pub speed: u16,
    pub boxes: [[Rect; 4]; 2],
    pub shift: Vec2,
}

impl Agent {
    pub fn get_step_duration(&self, tile_friction: u16, is_diagonal: bool) -> u32 {
        let move_speed = (SPEED_PARAM_A * ((self.speed as f32) + SPEED_PARAM_B).ln()
            + SPEED_PARAM_C)
            .round()
            .max(1.0);

        let tile_speed = (1000.0 * (tile_friction as f32) / move_speed).floor();
        let step_ms =
            ((tile_speed / (TICK_DURATION_MS as f32)).ceil() * (TICK_DURATION_MS as f32)) as u32;

        if is_diagonal {
            step_ms * DIAGONAL_STEP_FACTOR
        } else {
            step_ms
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FacingDirection {
    #[default]
    North = 0,
    East = 1,
    South = 2,
    West = 3,
}

impl From<FacingDirection> for u32 {
    fn from(value: FacingDirection) -> Self {
        value as u32
    }
}

impl From<FacingDirection> for usize {
    fn from(value: FacingDirection) -> Self {
        value as usize
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WalkingDirection {
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl WalkingDirection {
    pub fn is_diagonal(self) -> bool {
        matches!(
            self,
            WalkingDirection::NorthEast
                | WalkingDirection::NorthWest
                | WalkingDirection::SouthEast
                | WalkingDirection::SouthWest
        )
    }

    pub fn facing(&self) -> FacingDirection {
        match self {
            WalkingDirection::North => FacingDirection::North,
            WalkingDirection::East => FacingDirection::East,
            WalkingDirection::South => FacingDirection::South,
            WalkingDirection::West => FacingDirection::West,
            WalkingDirection::NorthEast => FacingDirection::East,
            WalkingDirection::SouthEast => FacingDirection::East,
            WalkingDirection::NorthWest => FacingDirection::West,
            WalkingDirection::SouthWest => FacingDirection::West,
        }
    }
}

// --- HUD components ---
#[derive(Component, Debug, Clone)]
pub enum HealthState {
    Lowest,
    Low,
    Half,
    AmostFull,
    Full,
}

impl HealthState {
    pub fn color(&self) -> Color {
        match self {
            HealthState::Full => Srgba::rgb(0.0, 0.7372549, 0.0).into(),
            HealthState::AmostFull => Srgba::rgb(0.6039216, 0.8039216, 0.19607843).into(),
            HealthState::Half => Srgba::rgb(0.98039216, 0.92156863, 0.0).into(),
            HealthState::Low => Srgba::rgb(1.0, 0.5, 0.0).into(),
            HealthState::Lowest => Srgba::rgb(1.0, 0.0, 0.0).into(),
        }
    }

    pub fn from_ratio(ratio: f32) -> Self {
        if ratio >= 0.90 {
            HealthState::Full
        } else if ratio >= 0.6 {
            HealthState::AmostFull
        } else if ratio >= 0.5 {
            HealthState::Half
        } else if ratio >= 0.3 {
            HealthState::Low
        } else {
            HealthState::Lowest
        }
    }
}

#[derive(Component, Debug, Clone)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Health {
    pub fn ratio(&self) -> f32 {
        self.current as f32 / self.max as f32
    }
}

#[derive(Component, Debug, Clone)]
pub struct Mana {
    pub current: u32,
    pub max: u32,
}

impl Mana {
    pub fn ratio(&self) -> f32 {
        self.current as f32 / self.max as f32
    }
}

#[derive(Component, Debug, Clone)]
pub struct Hud;

#[derive(Component, Debug, Clone)]
pub struct AgentHud {
    pub main_entity: Entity,
    pub health_bar: Option<Entity>,
    pub mana_bar: Option<Entity>,
    pub display_name: Entity,
    pub world_y_offset: f32,
}

#[derive(Component, Debug, Clone)]
pub struct DisplayName;

#[derive(Component, Debug, Clone)]
pub struct HudBar {
    pub ratio: f32,
}
// --- HUD components ---

#[derive(Component)]
pub struct AgentAnimConfigs {
    pub still: Arc<SpriteConfig>,
    pub moving: Arc<SpriteConfig>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paired with `walk_ticks_match_the_client` in the server's
    /// `entities/agent.rs`. The two formulas are deliberate duplicates across two
    /// repositories: a divergence compiles cleanly on both sides and simply
    /// desyncs movement, so each side pins the same three answers.
    ///
    /// The server asserts ticks; these are the same numbers times the 50ms tick.
    /// Speed 120 matches the server fixture's Speed skill.
    #[test]
    fn step_duration_matches_the_server() {
        let agent = Agent {
            speed: 120,
            ..Default::default()
        };

        assert_eq!(agent.get_step_duration(150, false), 500, "10 ticks");
        assert_eq!(
            agent.get_step_duration(150, true),
            1500,
            "30 ticks diagonal"
        );
        // 260 is the friction of `ornamented stone floor` (id 21718), one of the
        // ten values this side used to truncate through a `u8`.
        assert_eq!(agent.get_step_duration(260, false), 900, "18 ticks");
        // Rounding before the multiply, not after: 2600 here would mean the
        // diagonal had been scaled first and is no whole multiple of the step.
        assert_eq!(
            agent.get_step_duration(260, true),
            2700,
            "54 ticks diagonal"
        );
    }
}
