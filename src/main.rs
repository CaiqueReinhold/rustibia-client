#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::prelude::*;
use bevy::window::PresentMode;

use bevy_text_outline::TextOutlinePlugin;

// use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

mod agent;
mod camera;
mod conf;
mod config;
mod core;
mod game_ui;
mod items;
mod map;
mod network;
mod overlay;
mod player;

use crate::core::{GameAssetsLoaded, GameState};

fn main() {
    App::new()
        // .add_plugins((
        //     FrameTimeDiagnosticsPlugin::default(),
        //     LogDiagnosticsPlugin::default(),
        // ))
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Rustibia".into(),
                        name: Some("bevy.app".into()),
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(
            Startup,
            (camera::spawn_ui_camera, camera::spawn_game_camera),
        )
        .add_plugins((
            TextOutlinePlugin,
            core::CorePlugin,
            agent::AgentPlugin,
            map::MapPlugin,
            game_ui::GameUiPlugin,
            items::ItemsPlugin,
            player::PlayerPlugin,
            network::NetworkPlugin,
            overlay::OverlayPlugin,
        ))
        .init_state::<GameState>()
        .init_resource::<GameAssetsLoaded>()
        .run();
}
