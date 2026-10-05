use std::sync::Arc;

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::storage::ShaderStorageBuffer;

use crate::agent::{AgentInstance, AgentMaterial, LoadedMaterials, MoveQueue, spawn_agent};
use crate::conf::agent::{ADDON_1_FLAG, ADDON_2_FLAG};
use crate::core::{Appearances, GameState, InstanceManager, ItemConfigs};

use crate::game_ui::GameUiAssets;
use crate::items::{InventorySlot, Item};
use crate::map::Map;
use crate::network::events::{
    ClientOutdated, IventorySlotUpdated, PlayerCapacityUpdated, SpawnPlayer,
};
use crate::player::components::{Player, PlayerInventory};

pub fn check_game_ready(mut commands: Commands, player_q: Query<&Player>) {
    if !player_q.is_empty() {
        commands.set_state(GameState::InGame);
    }
}

pub fn spawn_player(
    event: On<SpawnPlayer>,
    mut commands: Commands,
    mut loaded_materials: ResMut<LoadedMaterials>,
    mut materials: ResMut<Assets<AgentMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut instances: ResMut<InstanceManager<AgentInstance>>,
    mut map: ResMut<Map>,
    ui_assets: Res<GameUiAssets>,
    appearances: Res<Appearances>,
    item_configs: Res<ItemConfigs>,
) {
    // Resolve everything the server named before touching the world, so a
    // client that turns out to be outdated leaves nothing half-spawned.
    let mut inventory = HashMap::new();
    for (slot, item_id) in [
        (InventorySlot::Head, event.inventory_head),
        (InventorySlot::Amulet, event.inventory_amulet),
        (InventorySlot::Backpack, event.inventory_backpack),
        (InventorySlot::Chest, event.inventory_chest),
        (InventorySlot::RightHand, event.inventory_right_hand),
        (InventorySlot::LeftHand, event.inventory_left_hand),
        (InventorySlot::Legs, event.inventory_legs),
        (InventorySlot::Feet, event.inventory_feet),
        (InventorySlot::Ring, event.inventory_ring),
        (InventorySlot::Trinket, event.inventory_trinket),
    ] {
        let Some(item_id) = item_id else {
            continue;
        };
        let Some(config) = item_configs.items.get(&item_id) else {
            commands.trigger(ClientOutdated);
            return;
        };
        inventory.insert(
            slot,
            Arc::new(Item {
                config: config.clone(),
                amount: 1,
            }),
        );
    }

    let Some(entity) = spawn_agent(
        &mut commands,
        &mut loaded_materials,
        &mut materials,
        &mut meshes,
        &mut buffers,
        &mut instances,
        &ui_assets.name_font,
        &appearances,
        event.outfit.0,
        &map,
        event.outfit.1,
        event.facing,
        event.speed,
        ADDON_1_FLAG | ADDON_2_FLAG,
        event.position.clone(),
        event.name.clone(),
        Some(event.health.clone()),
        Some(event.mana.clone()),
        event.agent_id,
    ) else {
        commands.trigger(ClientOutdated);
        return;
    };

    map.add_agent(event.agent_id, entity);
    commands
        .entity(entity)
        .insert(Player {
            agent_id: event.agent_id,
        })
        .remove::<MoveQueue>();

    commands.insert_resource(PlayerInventory {
        items: inventory,
        capacity: event.capacity,
    });
}

pub fn on_slot_update(
    event: On<IventorySlotUpdated>,
    mut commands: Commands,
    mut inventory: ResMut<PlayerInventory>,
    item_configs: Res<ItemConfigs>,
) {
    if let Some(item_id) = event.item_id {
        let Some(config) = item_configs.items.get(&item_id) else {
            commands.trigger(ClientOutdated);
            return;
        };
        let item = Arc::new(Item {
            config: config.clone(),
            amount: 1,
        });
        inventory.items.insert(event.slot, item);
    } else {
        inventory.items.remove(&event.slot);
    }
}

pub fn on_capacity_update(
    event: On<PlayerCapacityUpdated>,
    mut inventory: ResMut<PlayerInventory>,
) {
    inventory.capacity = event.capacity;
}
