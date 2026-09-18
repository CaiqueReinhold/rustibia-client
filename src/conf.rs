pub mod map {
    pub const TILE_SIZE: f32 = 32.0;
    pub const VIEW_TILES_X: f32 = 15.0;
    pub const VIEW_TILES_Y: f32 = 11.0;
    pub const TILES_X: usize = 19;
    pub const TILES_Y: usize = 15;
    pub const STACK_MAX_VISIBLE_ITEMS: usize = 8;
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

pub mod draw_order {
    /// Tiles of slack on each side of the drawn window, so a creature or a
    /// missile at the edge still keys to its own tile instead of clamping onto
    /// its neighbour's.
    pub const VIEW_MARGIN_TILES: usize = 2;

    /// Keys reserved per tile for `map::DrawLayer` and for the stack slot
    /// within a layer. `LAYER_COUNT` must exceed the largest `DrawLayer`
    /// discriminant, and their product times the window size must stay under
    /// 2^24 — both pinned by tests in `map::draw_order`.
    pub const LAYER_COUNT: u32 = 16;
    pub const SLOT_COUNT: u32 = 16;
}

pub mod target {
    pub const SQUARE_THICKNESS: f32 = 2.0;
    pub const SQUARE_COLOR: bevy::color::Color = bevy::color::Color::srgb(1.0, 0.0, 0.0);
    pub const MARK_INSET: f32 = 2.0;
    pub const MARK_COLOR: bevy::color::Color = bevy::color::Color::srgb(0.0, 0.0, 0.0);
    pub const MARK_DURATION: std::time::Duration = std::time::Duration::from_secs(1);
}

pub mod agent {
    // pub const ADDONS_NONE: u8 = 0;
    pub const ADDON_1_FLAG: u8 = 0b1;
    pub const ADDON_2_FLAG: u8 = 0b10;
    pub const SPEED_PARAM_A: f32 = 857.36;
    pub const SPEED_PARAM_B: f32 = 261.29;
    pub const SPEED_PARAM_C: f32 = -4795.009;
    pub const DIAGONAL_STEP_FACTOR: u32 = 3;
    pub const HUD_BAR_WIDTH: f32 = 30.0;
    pub const HUD_BAR_HEIGHT: f32 = 4.0;
}

pub mod viewport {
    use super::map;
    pub const GAME_VIEW_WIDTH: f32 = map::VIEW_TILES_X * map::TILE_SIZE;
    pub const GAME_VIEW_HEIGHT: f32 = map::VIEW_TILES_Y * map::TILE_SIZE;
    // pub const GAME_VIEW_MIN_SIZE: f32 = 400.0;
}

pub mod floating_text {
    pub const FONT_SIZE: f32 = 11.0;
    pub const OUTLINE_WIDTH: f32 = 1.0;
    /// Above the agent HUD siblings that share the game viewport.
    pub const Z_INDEX: i32 = 1;

    pub const HP_DURATION_MS: u64 = 1000;
    pub const HP_RISE_PX: f32 = 48.0;
    pub const HP_FADE_START: f32 = 0.83;
    pub const HP_MERGE_WINDOW: f32 = 0.4;
    pub const HP_CLEARANCE_PX: f32 = 12.0;
    pub const HP_MAX_STAGGER_PX: f32 = 36.0;

    // --- PlayerMessage ---
    pub const SPEECH_HEAD_OFFSET_WORLD: f32 = 24.0;
    pub const SPEECH_MS_PER_CHAR: u64 = 60;
    pub const SPEECH_MIN_MS: u64 = 3000;
    pub const SPEECH_MAX_MS: u64 = 8000;
    pub const SPEECH_MAX_LINES: usize = 5;
    pub const SPEECH_MAX_WIDTH_PX: f32 = 180.0;
    pub const SPEECH_GAP_PX: f32 = 2.0;
}

pub mod effects {
    use std::time::Duration;

    /// How long a `Static` effect stays on screen. Effects 200, 211 and 212 have
    /// no animation at all, so nothing else would ever end them. OTClient gives a
    /// static effect one frame tick (75 ms), which at 60 fps reads as a glitch;
    /// 300 ms is a hair over the shortest animated effect's full pass (270 ms).
    pub const STATIC_DURATION: Duration = Duration::from_millis(300);
}

pub mod missiles {
    pub const FLIGHT_MS_PER_ROOT_TILE: f32 = 150.0;
}

pub mod ui {
    pub const TOP_BAR_HEIGHT: f32 = 50.0;
    pub const SIDE_PANEL_WIDTH: f32 = 180.0;
    pub const CHAT_BOX_HEIGHT: f32 = 170.0;
    pub const UI_ITEM_SIZE: f32 = 32.0;
    pub const ITEM_COUNT_FONT_SIZE: f32 = 10.0;
    pub const LOOT_CONTAINER_DEFAULT_HEIGHT: usize = 40;
    pub const SKILLS_WINDOW_HEIGHT: usize = 150;
    pub const INVENTORY_HEIGHT: f32 = 170.0;
    pub const ITEM_SLOT_SIZE: f32 = 36.0;
    pub const UI_BAR_HEIGHT: f32 = 20.0;
    pub const MIN_DRAG_THRESHOLD: f32 = 1.0;
    pub const SEPARATOR_HEIGHT: f32 = 5.0;

    pub mod z_index {
        pub const Z_MAIN_UI: i32 = 10;
        pub const Z_WINDOW: i32 = 11;
        pub const Z_DRAGGING_WINDOW: i32 = 20;
        pub const DRAGGED_ITEM_UI_Z: i32 = 100;
        pub const Z_CONTEXT_MENU: i32 = 99;
    }

    pub mod ui_colors {
        use bevy::color::Srgba;
        pub const DARK_BORDER_COLOR: Srgba = Srgba::new(0.145098, 0.145098, 0.145098, 1.0);
        pub const LIGHT_BORDER_COLOR: Srgba = Srgba::new(0.4588235, 0.4588235, 0.4588235, 1.0);

        pub const ITEM_SLOT_OUTLINE: Srgba = Srgba::new(0.35, 0.35, 0.35, 1.0);
        pub const ITEM_SLOT_OUTLINE_HOVERED: Srgba = Srgba::new(0.8, 0.8, 0.8, 1.0);

        // pub const FONT_COLOR_TITLE: Srgba = Srgba::new(0.564705, 0.564705, 0.564705, 1.0);
        pub const FONT_COLOR_CONTENT: Srgba = Srgba::new(0.75294, 0.75294, 0.75294, 1.0);
        pub const FONT_COLOR_LOOK_MSG: Srgba = Srgba::rgb(0.0, 0.7372549, 0.0);

        pub const MANA_BAR_COLOR: Srgba = Srgba::new(0.0, 0.0, 0.7, 1.0);
        pub const COOLDOWN_SHADE: Srgba = Srgba::new(0.345098, 0.345098, 0.345098, 0.666667);
    }

    pub mod chat {
        use bevy::color::Srgba;

        pub const TAB_HEIGHT: f32 = 22.0;
        pub const TAB_MAX_WIDTH: f32 = 90.0;
        pub const INPUT_HEIGHT: f32 = 24.0;
        pub const HISTORY_CAP_DEFAULT: usize = 500;
        pub const LINE_HEIGHT: f32 = 12.;

        pub const UNREAD_TAB_COLOR: Srgba = Srgba::new(0.85, 0.20, 0.20, 1.0);
        pub const TAB_TITLE_COLOR: Srgba = Srgba::new(0.95, 0.95, 0.95, 1.0);
        pub const TAB_TITLE_COLOR_INACTIVE: Srgba = Srgba::new(0.5, 0.5, 0.5, 1.0);
        pub const INPUT_BG_COLOR: Srgba = Srgba::new(0.098, 0.102, 0.106, 1.0);
        pub const INPUT_PLACEHOLDER_COLOR: Srgba = Srgba::new(1.0, 1.0, 1.0, 0.2);

        pub const LOCAL_CHANNEL_NAME: &str = "Local";
        pub const LOCAL_CHANNEL_COLOR: Srgba = Srgba::new(0.94, 0.94, 0.0, 1.0);
        pub const CREATURE_SAY_COLOR: Srgba = Srgba::new(0.996, 0.396, 0.0, 1.0);

        pub const MAX_MESSAGE_LENGTH: usize = 255;
    }

    pub mod button_row {
        /// One row of buttons plus the window's 2px borders.
        pub const HEIGHT: f32 = 30.0;
        pub const PADDING: f32 = 4.0;
    }

    pub mod skills {
        pub const PADDING: f32 = 4.0;
        pub const ROW_GAP: f32 = 2.0;
        pub const BAR_HEIGHT: f32 = 5.0;
        pub const FONT_SIZE: f32 = 11.0;
    }

    pub mod dialog {
        use bevy::color::Srgba;

        pub const DEFAULT_WIDTH: f32 = 300.0;
        pub const TITLE_BAR_HEIGHT: f32 = 20.0;
        pub const PADDING: f32 = 10.0;
        pub const FIELD_HEIGHT: f32 = 24.0;
        pub const BUTTON_HEIGHT: f32 = 22.0;
        pub const BUTTON_MIN_WIDTH: f32 = 64.0;
        pub const Z_MODAL_BASE: i32 = 100;
        pub const DOUBLE_CLICK_SECS: f32 = 0.4;

        pub const BUTTON_COLOR: Srgba = Srgba::new(0.34, 0.34, 0.34, 1.0);
        pub const BUTTON_HOVER_COLOR: Srgba = Srgba::new(0.42, 0.42, 0.42, 1.0);
        pub const FIELD_BG_COLOR: Srgba = Srgba::new(0.098, 0.102, 0.106, 1.0);
        pub const ROW_SELECTED_COLOR: Srgba = Srgba::new(0.25, 0.32, 0.45, 1.0);

        pub const HOTKEY_DIALOG_WIDTH: f32 = 400.0;
        pub const WARNING_COLOR: Srgba = Srgba::new(0.95, 0.30, 0.30, 1.0);
    }

    pub mod context_menu {
        use bevy::color::Srgba;

        pub const MIN_WIDTH: f32 = 120.0;
        pub const PADDING: f32 = 2.0;
        pub const ROW_PADDING_X: f32 = 6.0;
        pub const ROW_PADDING_Y: f32 = 3.0;
        pub const HOVER_COLOR: Srgba = Srgba::new(0.25, 0.32, 0.45, 1.0);
        pub const DISABLED_COLOR: Srgba = Srgba::new(0.45, 0.45, 0.45, 1.0);
    }

    pub mod action_bar {
        pub const SLOT_SIZE: f32 = 34.0;
        pub const SLOT_GAP: f32 = 2.0;
        pub const PADDING: f32 = 3.0;
        pub const BORDER: f32 = 1.0;
        pub const HEIGHT: f32 = SLOT_SIZE + 2.0 * (PADDING + BORDER);
        pub const HOTKEY_FONT_SIZE: f32 = 9.0;
        pub const SPELL_ICON_SIZE: u32 = 32;
        pub const SPELL_ICON_COLUMNS: u32 = 12;
        pub const SPELL_ICON_ROWS: u32 = 11;
        pub const SPELL_LIST_MAX_HEIGHT: f32 = 220.0;
        pub const SPELL_ROW_HEIGHT: f32 = 36.0;
    }

    pub mod cooldown {
        pub const FONT_SIZE: f32 = 10.0;
        pub const ICON_SIZE: f32 = 20.0;
        pub const ICON_GAP: f32 = 3.0;
        pub const GROUP_SHEET_CELL: u32 = 20;
        pub const GROUP_SHEET_COLUMNS: u32 = 4;
        pub const GROUP_SHEET_ROWS: u32 = 2;
    }

    pub mod status {
        pub const ICON_SIZE: f32 = 9.0;
        pub const ICON_GAP: f32 = 1.0;
        pub const PADDING: f32 = 2.0;
        pub const BORDER: f32 = 1.0;
        pub const SHEET_CELL: u32 = 9;
        pub const SHEET_COLUMNS: u32 = 6;
    }

    pub mod login {
        use bevy::color::Srgba;

        pub const LOGO_COLOR: Srgba = Srgba::new(0.91, 0.78, 0.38, 1.0);
        pub const LOGO_FONT_SIZE: f32 = 56.0;
        pub const LOGO_TOP_MARGIN: f32 = 40.0;
    }
}

pub mod server {
    pub const TICK_DURATION_MS: u32 = 50;
}

pub mod minimap {
    pub const IMAGE_SIZE: u16 = 2048;
    /// Tiles visible per axis at each zoom level (index 0 = most zoomed in).
    pub const ZOOM_LEVELS: [u8; 4] = [20, 40, 80, 160];
    pub const DEFAULT_ZOOM: usize = 2; // 80×80 tiles
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
}
