use bevy::prelude::*;

use crate::{
    agent::{AgentId, FacingDirection, Health, Mana, WalkingDirection},
    conf::map::{TILES_X, TILES_Y},
    core::{
        ChatMessageType, EffectId, FloatingTextType, MissileId, OutfitColors, OutfitId,
        TextMessageType,
    },
    game_ui::{SkillProgress, SkillType},
    items::{ContainerId, InventorySlot, ItemId},
    map::Position,
    network::{ServerMessage, messages::ItemStack},
};

#[derive(Event, Debug)]
pub struct LoginError;

#[derive(Event, Debug)]
pub struct ConnectionLost;

/// The server named an item or outfit id this client's assets don't contain.
///
/// The two sides disagree about the game data, so nothing that follows can be
/// trusted.
#[derive(Event, Debug)]
pub struct ClientOutdated;

#[derive(Event, Debug)]
pub struct SpawnPlayer {
    pub agent_id: AgentId,
    pub position: Position,
    pub facing: FacingDirection,
    pub name: String,
    pub _level: u16,
    pub health: Health,
    pub mana: Mana,
    pub outfit: (OutfitId, OutfitColors),
    pub speed: u16,
    pub capacity: u32,
    pub inventory_head: Option<ItemId>,
    pub inventory_amulet: Option<ItemId>,
    pub inventory_backpack: Option<ItemId>,
    pub inventory_chest: Option<ItemId>,
    pub inventory_right_hand: Option<ItemId>,
    pub inventory_left_hand: Option<ItemId>,
    pub inventory_legs: Option<ItemId>,
    pub inventory_feet: Option<ItemId>,
    pub inventory_ring: Option<ItemId>,
    pub inventory_trinket: Option<ItemId>,
}

#[derive(Event, Debug)]
pub struct DescribeMap {
    pub center: Position,
    pub floor: u8,
    pub tiles: Box<[ItemStack; TILES_X * TILES_Y]>,
}

#[derive(Event, Debug)]
pub struct PlayerWalk {
    pub position: Position,
    pub tiles: Vec<(u8, Box<[ItemStack]>)>,
}

#[derive(Event, Debug)]
pub struct PlayerPosition {
    pub position: Position,
}

#[derive(Event, Debug)]
pub struct TileChanged {
    pub position: Position,
    pub items: Box<ItemStack>,
}

#[derive(Event, Debug)]
pub struct ShowTextMessage {
    pub text: String,
    pub message_type: TextMessageType,
}

#[derive(Event, Debug)]
pub struct ShowFloatingText {
    pub text: String,
    /// The block key, not the anchor: consecutive lines from the same speaker on the
    /// same tile merge into one block. Only ever `Some` for text this client raised
    /// itself off `ChatMessage`; the wire names no speaker on a damage number.
    pub speaker: Option<String>,
    pub position: Position,
    pub text_type: FloatingTextType,
    pub color: Option<(u8, u8, u8)>,
}

#[derive(Event, Debug)]
pub struct OpenContainer {
    pub container_id: ContainerId,
    pub capacity: u8,
    pub has_parent: bool,
    pub title: String,
    pub items: Box<[Option<(ItemId, u8)>]>,
}

#[derive(Event, Debug)]
pub struct UpdateContainer {
    pub container_id: ContainerId,
    pub items: Box<[Option<(ItemId, u8)>]>,
}

#[derive(Event, Debug)]
pub struct ContainerClosed {
    pub container_id: ContainerId,
}

#[derive(Event, Debug)]
pub struct PlayerWalkDenied;

#[derive(Event, Debug)]
pub struct IventorySlotUpdated {
    pub slot: InventorySlot,
    pub item_id: Option<ItemId>,
}

#[derive(Event, Debug)]
pub struct PlayerCapacityUpdated {
    pub capacity: u32,
}

#[derive(Event, Debug)]
pub struct AgentChangedDirection {
    pub agent_id: AgentId,
    pub facing: FacingDirection,
}

#[derive(Event, Debug)]
pub struct TeleportAgent {
    pub agent_id: AgentId,
    pub position: Position,
}

#[derive(Event, Debug)]
pub struct RemoveAgent {
    pub agent_id: AgentId,
}

#[derive(Event, Debug)]
pub struct TargetLost {
    pub seq: u32,
}

#[derive(Event, Debug)]
pub struct MoveAgent {
    pub agent_id: AgentId,
    pub from: Position,
    pub direction: WalkingDirection,
}

#[derive(Event, Debug)]
pub struct SpawnAgent {
    pub agent_id: AgentId,
    pub outfit: (OutfitId, OutfitColors),
    pub position: Position,
    pub facing: FacingDirection,
    pub name: String,
    pub health: u32,
    pub speed: u16,
}

#[derive(Event, Debug)]
pub struct ChatMessageReceived {
    pub author: String,
    pub message_type: ChatMessageType,
    pub channel: u16,
    pub position: Option<Position>,
    pub text: String,
}

#[derive(Event, Debug)]
pub struct ChannelListReceived {
    pub channels: Vec<(u16, String)>,
}

#[derive(Event, Debug)]
pub struct PrivateChatOpened {
    pub name: String,
}

#[derive(Event, Debug)]
pub struct AgentLifeChanged {
    pub agent_id: AgentId,
    pub current: u32,
    pub max: u32,
}

#[derive(Event, Debug)]
pub struct AgentManaChanged {
    pub agent_id: AgentId,
    pub current: u32,
    pub max: u32,
}

#[derive(Event, Debug)]
pub struct ShowEffect {
    pub effect_id: EffectId,
    pub position: Position,
    pub delta: Vec<(i8, i8)>,
}

#[derive(Event, Debug)]
pub struct LaunchMissile {
    pub from: Position,
    pub to: Position,
    pub missile_id: MissileId,
}

#[derive(Event, Debug)]
pub struct PlayerSkills {
    pub experience: u64,
    pub skills: Vec<(SkillType, SkillProgress)>,
}

#[derive(Event, Debug)]
pub struct SkillChanged {
    pub skill: SkillType,
    pub progress: SkillProgress,
}

#[derive(Event, Debug)]
pub struct ExperienceChanged {
    pub experience: u64,
}

pub fn route_event(msg: ServerMessage, commands: &mut Commands) {
    match msg {
        // Consumed by the IO task, which times the round trip and does not forward
        // it. The arm stays so the match is exhaustive if that ever changes.
        ServerMessage::Pong => {}
        ServerMessage::LoginError => commands.trigger(LoginError),
        ServerMessage::DescribePlayer {
            agent_id,
            position,
            facing,
            name,
            level,
            health,
            mana,
            outfit,
            speed,
            capacity,
            inventory_head,
            inventory_amulet,
            inventory_backpack,
            inventory_chest,
            inventory_right_hand,
            inventory_left_hand,
            inventory_legs,
            inventory_feet,
            inventory_ring,
            inventory_trinket,
        } => {
            commands.trigger(SpawnPlayer {
                agent_id,
                position,
                facing,
                name,
                _level: level,
                health,
                mana,
                outfit,
                speed,
                capacity,
                inventory_head,
                inventory_amulet,
                inventory_backpack,
                inventory_chest,
                inventory_right_hand,
                inventory_left_hand,
                inventory_legs,
                inventory_feet,
                inventory_ring,
                inventory_trinket,
            });
        }
        ServerMessage::DescribeMap {
            tiles,
            floor,
            center,
        } => {
            commands.trigger(DescribeMap {
                tiles,
                floor,
                center,
            });
        }
        ServerMessage::PlayerWalkAck { position, tiles } => {
            commands.trigger(PlayerWalk { position, tiles });
        }
        ServerMessage::TileChanged { position, items } => {
            commands.trigger(TileChanged { position, items });
        }
        ServerMessage::PlayerPosition { position } => {
            commands.trigger(PlayerPosition { position });
        }
        ServerMessage::TextMessage { text, message_type } => {
            commands.trigger(ShowTextMessage { text, message_type });
        }
        ServerMessage::OpenContainer {
            container_id,
            capacity,
            has_parent,
            title,
            items,
        } => {
            commands.trigger(OpenContainer {
                container_id,
                capacity,
                has_parent,
                title,
                items,
            });
        }
        ServerMessage::UpdateContainer {
            container_id,
            items,
        } => {
            commands.trigger(UpdateContainer {
                container_id,
                items,
            });
        }
        ServerMessage::ContainerClosed { container_id } => {
            commands.trigger(ContainerClosed { container_id });
        }
        ServerMessage::PlayerWalkDenied => {
            commands.trigger(PlayerWalkDenied);
        }
        ServerMessage::IventorySlotUpdated { slot, item_id } => {
            commands.trigger(IventorySlotUpdated { slot, item_id });
        }
        ServerMessage::PlayerCapacityUpdated { capacity } => {
            commands.trigger(PlayerCapacityUpdated { capacity });
        }
        ServerMessage::AgentChangedDirection { agent_id, facing } => {
            commands.trigger(AgentChangedDirection { agent_id, facing });
        }
        ServerMessage::RemoveAgent { agent_id } => {
            commands.trigger(RemoveAgent { agent_id });
        }
        ServerMessage::TargetLost { seq } => {
            commands.trigger(TargetLost { seq });
        }
        ServerMessage::MoveAgent {
            agent_id,
            direction,
            from,
        } => {
            commands.trigger(MoveAgent {
                agent_id,
                direction,
                from,
            });
        }
        ServerMessage::SpawnAgent {
            agent_id,
            outfit,
            position,
            facing,
            name,
            health,
            speed,
        } => {
            commands.trigger(SpawnAgent {
                agent_id,
                outfit,
                position,
                facing,
                name,
                health,
                speed,
            });
        }
        ServerMessage::TeleportAgent { agent_id, position } => {
            commands.trigger(TeleportAgent { agent_id, position });
        }
        ServerMessage::ChatMessage {
            author,
            message_type,
            channel,
            position,
            text,
        } => {
            commands.trigger(ChatMessageReceived {
                author,
                message_type,
                channel,
                position,
                text,
            });
        }
        ServerMessage::ChannelList { channels } => {
            commands.trigger(ChannelListReceived { channels });
        }
        ServerMessage::PrivateChatOpened { name } => {
            commands.trigger(PrivateChatOpened { name });
        }
        ServerMessage::FloatingText {
            text,
            position,
            text_type,
            color,
        } => {
            commands.trigger(ShowFloatingText {
                text,
                speaker: None,
                position,
                text_type,
                color,
            });
        }
        ServerMessage::AgentLifeChanged {
            agent_id,
            current,
            max,
        } => {
            commands.trigger(AgentLifeChanged {
                agent_id,
                current,
                max,
            });
        }
        ServerMessage::AgentManaChanged {
            agent_id,
            current,
            max,
        } => {
            commands.trigger(AgentManaChanged {
                agent_id,
                current,
                max,
            });
        }
        ServerMessage::ShowEffect {
            effect_id,
            position,
            delta,
        } => {
            commands.trigger(ShowEffect {
                effect_id,
                position,
                delta,
            });
        }
        ServerMessage::LaunchMissile {
            from,
            to,
            missile_id,
        } => {
            commands.trigger(LaunchMissile {
                from,
                to,
                missile_id,
            });
        }
        ServerMessage::PlayerSkills { experience, skills } => {
            commands.trigger(PlayerSkills { experience, skills });
        }
        ServerMessage::SkillChanged { skill, progress } => {
            commands.trigger(SkillChanged { skill, progress });
        }
        ServerMessage::ExperienceChanged { experience } => {
            commands.trigger(ExperienceChanged { experience });
        }
    }
}
