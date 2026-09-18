use bevy::{mesh::MeshTag, prelude::*, render::storage::ShaderStorageBuffer};

use crate::{
    agent::{
        Agent, AgentInstance, AgentMaterial, Health, LoadedMaterials, Mana, MoveQueue, Moving,
        StartAgentMove, spawn_agent,
    },
    conf::agent::{ADDON_1_FLAG, ADDON_2_FLAG},
    core::{Appearances, InstanceManager},
    game_ui::GameUiAssets,
    map::{FloorEntities, Map, Position},
    network::events::{
        AgentLifeChanged, AgentManaChanged, AgentSpeedUpdated, ClientOutdated, MoveAgent,
        RemoveAgent, SpawnAgent,
    },
};

pub fn on_spawn_agent(
    event: On<SpawnAgent>,
    mut commands: Commands,
    mut loaded_materials: ResMut<LoadedMaterials>,
    mut materials: ResMut<Assets<AgentMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut buffers: ResMut<Assets<ShaderStorageBuffer>>,
    mut instances: ResMut<InstanceManager<AgentInstance>>,
    mut map: ResMut<Map>,
    ui_assets: Res<GameUiAssets>,
    appearances: Res<Appearances>,
    agent_q: Query<&MeshTag, With<Agent>>,
    floor_ents: Res<FloorEntities>,
) {
    if let Some(entity) = map.get_agent(event.agent_id) {
        if let Ok(tag) = agent_q.get(entity) {
            instances.dealloc_index(tag.0);
        }
        commands.entity(entity).despawn();
        map.unindex_agent(event.agent_id);
        map.remove_agent(event.agent_id);
    }

    let Some(entity) = spawn_agent(
        &mut commands,
        &mut loaded_materials,
        &mut materials,
        &mut meshes,
        &mut buffers,
        &mut instances,
        &ui_assets.font,
        &appearances,
        event.outfit.0,
        &map,
        event.outfit.1,
        event.facing,
        event.speed,
        ADDON_1_FLAG | ADDON_2_FLAG,
        event.position.clone(),
        event.name.clone(),
        Some(Health {
            current: event.health,
            max: 100,
        }),
        None,
        event.agent_id,
    ) else {
        commands.trigger(ClientOutdated);
        return;
    };
    map.add_agent(event.agent_id, entity);

    commands
        .entity(floor_ents.floors[event.position.z as usize])
        .add_child(entity);
}

pub fn on_move_agent(
    event: On<MoveAgent>,
    mut commands: Commands,
    map: Res<Map>,
    pos_q: Query<&Position>,
    mut queue_q: Query<&mut MoveQueue>,
    moving_q: Query<&Moving>,
) {
    let Some(agent_entity) = map.get_agent(event.agent_id) else {
        return;
    };

    if moving_q.get(agent_entity).is_ok() {
        if let Ok(mut queue) = queue_q.get_mut(agent_entity) {
            queue.0.push_back((event.from.clone(), event.direction));
        }
        return;
    }

    let Ok(pos) = pos_q.get(agent_entity) else {
        return;
    };
    if *pos != event.from {
        commands.entity(agent_entity).insert(event.from.clone());
    }

    commands.trigger(StartAgentMove {
        agent_id: event.agent_id,
        direction: event.direction,
    });
}

pub fn on_remove_agent(
    event: On<RemoveAgent>,
    mut commands: Commands,
    mut instances: ResMut<InstanceManager<AgentInstance>>,
    agent_q: Query<&MeshTag, With<Agent>>,
    mut map: ResMut<Map>,
) {
    let Some(agent_entity) = map.get_agent(event.agent_id) else {
        return;
    };

    // Unconditional on knowing the agent existed, not on the GPU cleanup
    // query below matching — otherwise a query miss would leave `Map`'s
    // bookkeeping stale while still failing to despawn the entity.
    map.unindex_agent(event.agent_id);
    map.remove_agent(event.agent_id);

    let Ok(tag) = agent_q.get(agent_entity) else {
        return;
    };
    instances.dealloc_index(tag.0);
    commands.entity(agent_entity).despawn();
}

pub fn on_agent_life_changed(
    event: On<AgentLifeChanged>,
    mut agent_q: Query<&mut Health>,
    map: Res<Map>,
) {
    let Some(agent_entity) = map.get_agent(event.agent_id) else {
        return;
    };
    let Ok(mut health) = agent_q.get_mut(agent_entity) else {
        return;
    };
    // Both fields, not just current. The same message means different things
    // depending on who it is about -- absolute for the player, a percentage
    // against a max of 100 for anyone else -- and writing the pair is what makes
    // one handler correct for both.
    health.current = event.current;
    health.max = event.max;
}

pub fn on_agent_mana_changed(
    event: On<AgentManaChanged>,
    mut agent_q: Query<&mut Mana>,
    map: Res<Map>,
) {
    let Some(agent_entity) = map.get_agent(event.agent_id) else {
        return;
    };
    // Only the local player carries a `Mana` component -- the server sends this
    // for nobody else -- so a miss here is the ordinary case, not an error.
    let Ok(mut mana) = agent_q.get_mut(agent_entity) else {
        return;
    };
    mana.current = event.current;
    mana.max = event.max;
}

pub fn on_agent_speed_updated(
    event: On<AgentSpeedUpdated>,
    mut agent_q: Query<&mut Agent>,
    map: Res<Map>,
) {
    let Some(agent_entity) = map.get_agent(event.agent_id) else {
        return;
    };
    let Ok(mut agent) = agent_q.get_mut(agent_entity) else {
        return;
    };
    agent.speed = event.speed;
}
