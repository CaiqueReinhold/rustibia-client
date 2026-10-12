use std::time::Duration;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::conf::ui::options::{KEY_REPEAT_MS_DEFAULT, KEY_REPEAT_MS_MAX, KEY_REPEAT_MS_MIN};

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub show_fps: bool,
    pub show_ping: bool,
    pub vsync: bool,
    pub show_names: bool,
    /// Every agent's health bar and the player's own mana bar.
    pub show_health_bars: bool,
    pub show_damage_numbers: bool,
    pub show_speech: bool,
    pub key_repeat_ms: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_fps: false,
            show_ping: true,
            vsync: false,
            show_names: true,
            show_health_bars: true,
            show_damage_numbers: true,
            show_speech: true,
            key_repeat_ms: KEY_REPEAT_MS_DEFAULT,
        }
    }
}

impl Settings {
    pub fn key_repeat(&self) -> Duration {
        Duration::from_millis(self.key_repeat_ms as u64)
    }

    pub(super) fn clamped(mut self) -> Self {
        self.key_repeat_ms = self
            .key_repeat_ms
            .clamp(KEY_REPEAT_MS_MIN, KEY_REPEAT_MS_MAX);
        self
    }
}
