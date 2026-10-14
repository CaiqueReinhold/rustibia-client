pub mod map {
    pub const TILE_SIZE: f32 = 32.0;
    pub const VIEW_TILES_X: f32 = 15.0;
    pub const VIEW_TILES_Y: f32 = 11.0;
    pub const VIEW_LEFT: usize = 8;
    pub const VIEW_RIGHT: usize = 9;
    pub const VIEW_TOP: usize = 6;
    pub const VIEW_BOTTOM: usize = 7;
    pub const TILES_X: usize = VIEW_LEFT + VIEW_RIGHT + 1;
    pub const TILES_Y: usize = VIEW_TOP + VIEW_BOTTOM + 1;
    pub const STACK_MAX_VISIBLE_ITEMS: usize = 10;
    pub const CONTAINER_COORD_FLAG: u16 = 0xFFFF;
    pub const INVENTORY_COORD_FLAG: u16 = 0xFFFE;
    /// In `y` beside `INVENTORY_COORD_FLAG`. The server's `constants::items::CARRIED_SEARCH_FLAG`
    /// must equal it.
    pub const CARRIED_SEARCH_FLAG: u16 = 0xFFFF;
    pub const MIN_FLOOR: u8 = 0;
    pub const MAX_FLOOR: u8 = 15;
    pub const BASE_FLOOR: u8 = 7;
    pub const UNDERGROUND_REACH: u8 = 2;
}

pub mod target {
    pub const SQUARE_THICKNESS: f32 = 2.0;
    pub const SQUARE_COLOR: bevy::color::Color = bevy::color::Color::srgb(1.0, 0.0, 0.0);
}

pub mod agent {
    // pub const ADDONS_NONE: u8 = 0;
    pub const ADDON_1_FLAG: u8 = 0b1;
    pub const ADDON_2_FLAG: u8 = 0b10;
}

pub mod viewport {
    use super::map;
    pub const GAME_VIEW_WIDTH: f32 = map::VIEW_TILES_X * map::TILE_SIZE;
    pub const GAME_VIEW_HEIGHT: f32 = map::VIEW_TILES_Y * map::TILE_SIZE;
    // pub const GAME_VIEW_MIN_SIZE: f32 = 400.0;
}

pub mod ui {
    pub const TOP_BAR_HEIGHT: f32 = 50.0;
    pub const SIDE_PANEL_WIDTH: f32 = 180.0;
    pub const CHAT_BOX_MIN_HEIGHT: f32 = 80.0;
    pub const GAME_VIEWPORT_MIN_WIDTH: f32 = 300.0;
    pub const GAME_VIEWPORT_MIN_HEIGHT: f32 = 220.0;
    /// Border, padding and margin around the game viewport, both sides of one axis.
    pub const GAME_VIEWPORT_FRAME: f32 = 8.0;
    pub const WINDOW_MIN_WIDTH: f32 =
        2.0 * SIDE_PANEL_WIDTH + GAME_VIEWPORT_MIN_WIDTH + GAME_VIEWPORT_FRAME;
    pub const WINDOW_MIN_HEIGHT: f32 = TOP_BAR_HEIGHT
        + GAME_VIEWPORT_MIN_HEIGHT
        + GAME_VIEWPORT_FRAME
        + SEPARATOR_HEIGHT
        + action_bar::HEIGHT
        + CHAT_BOX_MIN_HEIGHT;
    pub const UI_ITEM_SIZE: f32 = 32.0;
    pub const ITEM_SLOT_SIZE: f32 = 36.0;
    pub const SEPARATOR_HEIGHT: f32 = 5.0;

    pub mod z_index {
        pub const Z_MAIN_UI: i32 = 10;
        pub const Z_WINDOW: i32 = 11;
    }

    pub mod ui_colors {
        use bevy::color::Srgba;
        pub const DARK_BORDER_COLOR: Srgba = Srgba::new(0.145098, 0.145098, 0.145098, 1.0);
        pub const LIGHT_BORDER_COLOR: Srgba = Srgba::new(0.4588235, 0.4588235, 0.4588235, 1.0);

        pub const ITEM_SLOT_OUTLINE_HOVERED: Srgba = Srgba::new(0.8, 0.8, 0.8, 1.0);

        pub const FONT_COLOR_CONTENT: Srgba = Srgba::new(0.75294, 0.75294, 0.75294, 1.0);
        pub const FONT_COLOR_LOOK_MSG: Srgba = Srgba::rgb(0.0, 0.7372549, 0.0);

        pub const MANA_BAR_COLOR: Srgba = Srgba::new(0.0, 0.0, 0.7, 1.0);

        pub const BG_COLOR: Srgba = Srgba::new(0.098, 0.102, 0.106, 1.0);
    }

    pub mod chat {
        use bevy::color::Srgba;

        pub const TAB_TITLE_COLOR: Srgba = Srgba::new(0.95, 0.95, 0.95, 1.0);
        pub const TAB_TITLE_COLOR_INACTIVE: Srgba = Srgba::new(0.5, 0.5, 0.5, 1.0);
        pub const INPUT_BG_COLOR: Srgba = Srgba::new(0.098, 0.102, 0.106, 1.0);
        pub const INPUT_PLACEHOLDER_COLOR: Srgba = Srgba::new(1.0, 1.0, 1.0, 0.2);

        pub const LOCAL_CHANNEL_COLOR: Srgba = Srgba::new(0.94, 0.94, 0.0, 1.0);

        pub const MAX_MESSAGE_LENGTH: usize = 255;
    }

    pub mod dialog {
        use bevy::color::Srgba;

        pub const FIELD_HEIGHT: f32 = 24.0;
        pub const DOUBLE_CLICK_SECS: f32 = 0.4;

        pub const BUTTON_COLOR: Srgba = Srgba::new(0.34, 0.34, 0.34, 1.0);
        pub const FIELD_BG_COLOR: Srgba = Srgba::new(0.198, 0.202, 0.206, 1.0);
        pub const ROW_SELECTED_COLOR: Srgba = Srgba::new(0.25, 0.32, 0.45, 1.0);
    }

    pub mod options {
        pub const KEY_REPEAT_MS_MIN: u16 = 50;
        pub const KEY_REPEAT_MS_MAX: u16 = 500;
        pub const KEY_REPEAT_MS_DEFAULT: u16 = 200;
    }

    pub mod action_bar {
        pub const SLOT_SIZE: f32 = 34.0;
        pub const PADDING: f32 = 3.0;
        pub const BORDER: f32 = 1.0;
        pub const HEIGHT: f32 = SLOT_SIZE + 2.0 * (PADDING + BORDER);
    }
}

pub mod server {
    pub const TICK_DURATION_MS: u32 = 50;
}

pub mod paths {
    use std::path::PathBuf;

    /// Returns the root data directory for persistent game data.
    ///
    /// - Linux:   `~/.local/share/Rustibia`
    /// - Windows: `%APPDATA%\Rustibia`
    pub fn data_dir() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("Rustibia")
        }
        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(appdata).join("Rustibia")
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            PathBuf::from("data")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The server enforces its own `chat.max_message_length` and the client has no
    /// access to server config, so the constant is duplicated. If they diverge, the
    /// input field lets the player compose messages the server silently refuses —
    /// which compiles fine and only shows up in play. This test is the tripwire.
    ///
    /// Server value: `crates/server/assets/game_conf.yaml`, `chat.max_message_length`.
    #[test]
    fn max_message_length_matches_the_server() {
        assert_eq!(
            ui::chat::MAX_MESSAGE_LENGTH,
            255,
            "must equal chat.max_message_length in the server's game_conf.yaml"
        );
    }

    /// The pin. Its twin is `the_carried_search_flag_matches_the_client` in the server's
    /// `constants::items`. If these two disagree, the server reads an action-bar item use as an
    /// ordinary stale coordinate and drops it in silence — nothing fails to compile, nothing
    /// errors at runtime, and no refusal reaches the player.
    #[test]
    fn carried_search_flag_matches_the_server() {
        assert_eq!(
            map::CARRIED_SEARCH_FLAG,
            0xFFFF,
            "must equal constants::items::CARRIED_SEARCH_FLAG in the server"
        );
    }

    /// The pin. Its twin is `the_viewport_extents_match_the_client` in the server's
    /// `constants::view`. A split that differs but sums to the same width still decodes, and
    /// every tile is drawn a column off.
    #[test]
    fn the_viewport_extents_match_the_server() {
        assert_eq!(
            (
                map::VIEW_LEFT,
                map::VIEW_RIGHT,
                map::VIEW_TOP,
                map::VIEW_BOTTOM
            ),
            (8, 9, 6, 7),
            "must equal constants::view::VIEW_* in the server"
        );
    }
}
