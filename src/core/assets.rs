use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::futures_lite::future;
use bevy::tasks::{IoTaskPool, Task};

use crate::core::items::{ItemConfigs, read_item_configs};
use crate::core::sprite::{Appearances, SpriteConfigs, read_sprite_sheets, read_sprites_config};
use crate::core::{GameState, SpriteSheet};
use crate::items::{ItemConfig, ItemId};

#[derive(Resource)]
pub struct LoadTasks {
    sprite_conf_task: Task<SpriteConfigs>,
    sprite_sheet_task: Task<HashMap<String, SpriteSheet>>,
    items_task: Task<HashMap<ItemId, Arc<ItemConfig>>>,
}

#[derive(Resource, Debug, Default)]
pub struct GameAssetsLoaded {
    pub items_loaded: bool,
    pub sheets_loaded: bool,
}

pub fn start_load_tasks(mut commands: Commands) {
    let pool = IoTaskPool::get();
    let sprite_conf_task = pool.spawn(async { read_sprites_config() });
    let sprite_sheet_task = pool.spawn(async { read_sprite_sheets() });
    let items_task = pool.spawn(async { read_item_configs() });

    commands.insert_resource(LoadTasks {
        sprite_conf_task,
        sprite_sheet_task,
        items_task,
    });
}

pub fn pool_load_task(
    mut commands: Commands,
    mut tasks: ResMut<LoadTasks>,
    mut game_assets: ResMut<GameAssetsLoaded>,
    asset_server: Res<AssetServer>,
) {
    if tasks.sprite_conf_task.is_finished()
        && tasks.sprite_sheet_task.is_finished()
        && !game_assets.sheets_loaded
    {
        let results = (
            future::block_on(future::poll_once(&mut tasks.sprite_conf_task)),
            future::block_on(future::poll_once(&mut tasks.sprite_sheet_task)),
        );
        match results {
            (Some(configs), Some(sprite_sheet)) => {
                commands.insert_resource(Appearances::new(
                    sprite_sheet,
                    configs,
                    asset_server.clone(),
                ));
                game_assets.sheets_loaded = true;
                info!("Sprites loaded");
            }
            (None, ..) | (.., None) => panic!("Failed to load sprites"),
        }
    }

    if tasks.items_task.is_finished() && !game_assets.items_loaded {
        let result = future::block_on(future::poll_once(&mut tasks.items_task));
        match result {
            Some(items) => {
                commands.insert_resource(ItemConfigs { items });
                game_assets.items_loaded = true;
                info!("Items loaded");
            }
            None => {
                panic!("Failed to load map");
            }
        }
    }
}

pub fn pool_all_assets_loaded(mut commands: Commands, game_assets: Res<GameAssetsLoaded>) {
    if game_assets.items_loaded && game_assets.sheets_loaded {
        commands.set_state(GameState::LoginScreen);
        commands.remove_resource::<GameAssetsLoaded>();
        commands.remove_resource::<LoadTasks>();
    }
}
