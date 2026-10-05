use bevy::prelude::*;

#[derive(Default)]
pub struct UiWindow {
    pub close_button: Handle<Image>,
    pub parent_container: Handle<Image>,
    pub minimize_button: Handle<Image>,
}

#[derive(Default)]
pub struct UiInventory {
    pub no_helmet: Handle<Image>,
    pub no_amulet: Handle<Image>,
    pub no_armor: Handle<Image>,
    pub no_backpack: Handle<Image>,
    pub no_left_hand: Handle<Image>,
    pub no_right_hand: Handle<Image>,
    pub no_ring: Handle<Image>,
    pub no_legs: Handle<Image>,
    pub no_feet: Handle<Image>,
}

#[derive(Default)]
pub struct UiButtons {
    pub logout: Handle<Image>,
    pub skills: Handle<Image>,
}

#[derive(Resource)]
pub struct GameUiAssets {
    pub font: Handle<Font>,
    pub name_font: Handle<Font>,
    pub window: UiWindow,
    pub inventory: UiInventory,
    pub background_dark: Handle<Image>,
    pub background_light: Handle<Image>,
    pub bar_overlay: Handle<Image>,
    pub title_background: Handle<Image>,
    pub buttons: UiButtons,
}

pub fn setup_game_ui_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    let window = UiWindow {
        close_button: asset_server.load("ui/window_close.png"),
        parent_container: asset_server.load("ui/window_parent.png"),
        minimize_button: asset_server.load("ui/window_minimize.png"),
    };

    let inventory = UiInventory {
        no_helmet: asset_server.load("ui/inventory/no_helmet.png"),
        no_amulet: asset_server.load("ui/inventory/no_amulet.png"),
        no_armor: asset_server.load("ui/inventory/no_armor.png"),
        no_backpack: asset_server.load("ui/inventory/no_bag.png"),
        no_left_hand: asset_server.load("ui/inventory/no_weapon_left.png"),
        no_right_hand: asset_server.load("ui/inventory/no_weapon_right.png"),
        no_ring: asset_server.load("ui/inventory/no_ring.png"),
        no_legs: asset_server.load("ui/inventory/no_legs.png"),
        no_feet: asset_server.load("ui/inventory/no_boots.png"),
    };

    let buttons = UiButtons {
        logout: asset_server.load("ui/logout.png"),
        skills: asset_server.load("ui/skills.png"),
    };

    commands.insert_resource(GameUiAssets {
        font: asset_server.load("fonts/VerdanaBd.ttf"),
        name_font: asset_server.load("fonts/Verdana.ttf"),
        window,
        inventory,
        background_dark: asset_server.load("ui/background_dark.png"),
        background_light: asset_server.load("ui/background_light.png"),
        bar_overlay: asset_server.load("ui/bar_overlay.png"),
        title_background: asset_server.load("ui/title_background.png"),
        buttons,
    });
}
