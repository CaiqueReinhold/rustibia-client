use std::fmt::Display;

use asynchronous_codec::{Decoder, Encoder};
use bevy::log::warn;
use bytes::{Buf, BufMut, BytesMut};
use thiserror::Error;

use crate::{
    agent::{AgentId, FacingDirection, Health, Mana, WalkingDirection},
    conf::map::{STACK_MAX_VISIBLE_ITEMS, TILES_X, TILES_Y},
    core::{
        ChatMessageType, EffectId, FloatingTextType, MissileId, OutfitColors, OutfitId, SayTarget,
        SpellGroup, SpellId, SpellInfo, SpellTarget, TextMessageType,
    },
    game_ui::{SkillProgress, SkillType},
    items::{ContainerId, InventorySlot, ItemId},
    map::Position,
};

pub type ItemStack = [Option<(ItemId, u8)>; STACK_MAX_VISIBLE_ITEMS];

// client
const CLI_PING: u8 = 0;
const CLI_LOGIN: u8 = 1;
const CLI_MOVE_PLAYER: u8 = 2;
const CLI_GET_PLAYER_POS: u8 = 3;
const CLI_MOVE_ITEM: u8 = 4;
const CLI_USE_ITEM: u8 = 5;
const CLI_CLOSE_CONTAINER: u8 = 6;
const CLI_OPEN_PARENT_CONTAINER: u8 = 7;
const CLI_CHANGE_DIRECTION: u8 = 8;
const CLI_LOGOUT: u8 = 9;
const CLI_USE_ITEM_WITH: u8 = 10;
const CLI_LOOK: u8 = 11;
const CLI_SAY: u8 = 12;
const CLI_REQUEST_CHANNELS: u8 = 13;
const CLI_OPEN_CHANNEL: u8 = 14;
const CLI_CLOSE_CHANNEL: u8 = 15;
const CLI_OPEN_PM_CHAT: u8 = 16;
const CLI_SET_TARGET: u8 = 17;
const CLI_CAST_SPELL: u8 = 18;

#[derive(Clone, Debug)]
pub enum ClientMessage {
    Ping,
    Login {
        auth_token: String,
    },
    MovePlayer {
        direction: WalkingDirection,
    },
    GetPlayerPosition,
    MoveItem {
        from: Position,
        item_id: ItemId,
        amount: u8,
        stack_index: u8,
        to: Position,
    },
    UseItem {
        position: Position,
        item_id: ItemId,
        stack_index: u8,
    },
    CloseContainer {
        container_id: ContainerId,
    },
    OpenParentContainer {
        container_id: ContainerId,
    },
    ChangeDirection {
        direction: FacingDirection,
    },
    Logout,
    UseItemWith {
        source: Position,
        source_item_id: ItemId,
        source_index: u8,
        target: Position,
        target_item_id: ItemId,
        target_index: u8,
        target_agent: Option<AgentId>,
    },
    Look {
        position: Position,
    },
    Say {
        message: String,
        target: SayTarget,
    },
    RequestChannels,
    OpenChannel {
        channel: u16,
    },
    CloseChannel {
        channel: u16,
    },
    OpenPmChat {
        name: String,
    },
    SetTarget {
        agent_id: Option<AgentId>,
        seq: u32,
    },
    CastSpell {
        spell_id: SpellId,
        target: SpellTarget,
    },
}

// server
const SRV_PONG: u8 = 0;
const SRV_LOGIN_ERROR: u8 = 1;
const SRV_DESCRIBE_MAP: u8 = 2;
const SRV_TILE_UPDATED: u8 = 3;
const SRV_PLAYER_WALK_ACK: u8 = 4;
const SRV_PLAYER_POS: u8 = 5;
const SRV_DESCRIBE_PLAYER: u8 = 6;
const SRV_TEXT_MESSAGE: u8 = 7;
const SRV_OPEN_CONTAINER: u8 = 8;
const SRV_UPDATE_CONTAINER: u8 = 9;
const SRV_CONTAINER_CLOSED: u8 = 10;
const SRV_PLAYER_WALK_DENIED: u8 = 11;
const SRV_INVETORY_SLOT_UPDATED: u8 = 12;
const SRV_PLAYER_CAPACITY_UPDATED: u8 = 13;
const SRV_AGENT_DIRECTION_CHANGED: u8 = 14;
const SRV_REMOVE_AGENT: u8 = 15;
const SRV_MOVE_AGENT: u8 = 16;
const SRV_SPAWN_AGENT: u8 = 17;
const SRV_TELEPORT_AGENT: u8 = 18;
const SRV_CHAT_MESSAGE: u8 = 19;
const SRV_CHANNEL_LIST: u8 = 20;
const SRV_PRIVATE_CHAT_OPENED: u8 = 21;
const SRV_FLOATING_TEXT: u8 = 22;
const SRV_TARGET_LOST: u8 = 23;
const SRV_AGENT_LIFE_UPDATED: u8 = 24;
const SRV_SHOW_EFFECT: u8 = 25;
const SRV_LAUNCH_MISSILE: u8 = 26;
const SRV_AGENT_MANA_UPDATED: u8 = 27;
const SRV_PLAYER_SKILLS: u8 = 28;
const SRV_SKILL_UPDATED: u8 = 29;
const SRV_EXPERIENCE_UPDATED: u8 = 30;
const SRV_SPELL_CAST: u8 = 31;
const SRV_SPELL_LIST: u8 = 32;
const SRV_AGENT_SPEED_UPDATED: u8 = 33;

#[derive(Clone, Debug)]
pub enum ServerMessage {
    Pong,
    LoginError,
    DescribePlayer {
        agent_id: AgentId,
        position: Position,
        facing: FacingDirection,
        name: String,
        level: u16,
        health: Health,
        mana: Mana,
        outfit: (OutfitId, OutfitColors),
        speed: u16,
        capacity: u32,
        inventory_head: Option<ItemId>,
        inventory_amulet: Option<ItemId>,
        inventory_backpack: Option<ItemId>,
        inventory_chest: Option<ItemId>,
        inventory_right_hand: Option<ItemId>,
        inventory_left_hand: Option<ItemId>,
        inventory_legs: Option<ItemId>,
        inventory_feet: Option<ItemId>,
        inventory_ring: Option<ItemId>,
        inventory_trinket: Option<ItemId>,
    },
    DescribeMap {
        tiles: Box<[ItemStack; TILES_X * TILES_Y]>,
        floor: u8,
        center: Position,
    },
    TileUpdated {
        position: Position,
        items: Box<ItemStack>,
    },
    PlayerWalkAck {
        position: Position,
        tiles: Vec<(u8, Box<[ItemStack]>)>,
    },
    PlayerPosition {
        position: Position,
    },
    TextMessage {
        text: String,
        message_type: TextMessageType,
    },
    OpenContainer {
        container_id: ContainerId,
        capacity: u8,
        has_parent: bool,
        title: String,
        items: Box<[Option<(ItemId, u8)>]>,
    },
    UpdateContainer {
        container_id: ContainerId,
        items: Box<[Option<(ItemId, u8)>]>,
    },
    ContainerClosed {
        container_id: ContainerId,
    },
    PlayerWalkDenied,
    IventorySlotUpdated {
        slot: InventorySlot,
        item_id: Option<ItemId>,
    },
    PlayerCapacityUpdated {
        capacity: u32,
    },
    AgentChangedDirection {
        agent_id: AgentId,
        facing: FacingDirection,
    },
    RemoveAgent {
        agent_id: AgentId,
    },
    MoveAgent {
        agent_id: AgentId,
        direction: WalkingDirection,
        from: Position,
    },
    SpawnAgent {
        agent_id: AgentId,
        outfit: (OutfitId, OutfitColors),
        position: Position,
        facing: FacingDirection,
        name: String,
        health: u32,
        speed: u16,
    },
    TeleportAgent {
        agent_id: AgentId,
        position: Position,
    },
    ChatMessage {
        author: String,
        message_type: ChatMessageType,
        channel: u16,
        position: Option<Position>,
        text: String,
    },
    ChannelList {
        channels: Vec<(u16, String)>,
    },
    PrivateChatOpened {
        name: String,
    },
    FloatingText {
        text: String,
        position: Position,
        text_type: FloatingTextType,
        color: Option<(u8, u8, u8)>,
    },
    TargetLost {
        seq: u32,
    },
    ShowEffect {
        effect_id: EffectId,
        position: Position,
        delta: Vec<(i8, i8)>,
    },
    AgentLifeUpdated {
        agent_id: AgentId,
        current: u32,
        max: u32,
    },
    AgentManaUpdated {
        agent_id: AgentId,
        current: u32,
        max: u32,
    },
    PlayerSkills {
        experience: u64,
        skills: Vec<(SkillType, SkillProgress)>,
    },
    SkillUpdated {
        skill: SkillType,
        progress: SkillProgress,
    },
    ExperienceUpdated {
        experience: u64,
    },
    LaunchMissile {
        from: Position,
        to: Position,
        missile_id: MissileId,
    },
    SpellCast {
        spell_id: SpellId,
        spell_cooldown_ms: u32,
        group_cooldown_ms: u32,
    },
    SpellList {
        spells: Vec<SpellInfo>,
    },
    AgentSpeedUpdated {
        agent_id: AgentId,
        speed: u16,
    },
}

impl Display for ServerMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerMessage::DescribeMap { center, floor, .. } => {
                write!(f, "DescribeMap {{ center: {}, floor: {} }}", center, floor)
            }
            ServerMessage::PlayerWalkAck { position, tiles } => {
                write!(
                    f,
                    "PlayerWalkAck {{ position: {}, floors: {:?} }}",
                    position,
                    tiles.iter().map(|i| i.0).collect::<Vec<u8>>()
                )
            }
            ServerMessage::TileUpdated { position, .. } => {
                write!(f, "TileUpdated {{ position: {} }}", position)
            }
            ServerMessage::OpenContainer { container_id, .. } => {
                write!(f, "OpenContainer {{ container_id: {container_id:?} }}")
            }
            ServerMessage::UpdateContainer { container_id, .. } => {
                write!(f, "UpdateContainer {{ container_id: {container_id:?} }}")
            }
            msg => {
                write!(f, "{:?}", msg)
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum MessageDecodeError {
    #[error("Read error")]
    ReadError(#[from] std::io::Error),
    #[error("Wrong sequence")]
    WrongSequence,
    #[error("Payload ended mid-field")]
    Truncated,
    #[error("{0} unread byte(s) left in the payload")]
    TrailingBytes(usize),
}

/// A bounds-checked cursor over a single frame's payload.
///
/// Every read returns [`MessageDecodeError::Truncated`] instead of panicking
/// when the payload runs out, which is what keeps a malformed or truncated
/// packet from killing the connection task.
struct Reader<'a> {
    buf: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    fn remaining(&self) -> usize {
        self.buf.len()
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], MessageDecodeError> {
        if self.buf.len() < len {
            return Err(MessageDecodeError::Truncated);
        }
        let (head, tail) = self.buf.split_at(len);
        self.buf = tail;
        Ok(head)
    }

    fn read_u8(&mut self) -> Result<u8, MessageDecodeError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_i8(&mut self) -> Result<i8, MessageDecodeError> {
        Ok(self.read_bytes(1)?[0] as i8)
    }

    fn read_u16_le(&mut self) -> Result<u16, MessageDecodeError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn read_u32_le(&mut self) -> Result<u32, MessageDecodeError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_u64_le(&mut self) -> Result<u64, MessageDecodeError> {
        let b = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Reads a `len`-byte string. Invalid UTF-8 is replaced rather than
    /// rejected — a mangled name is not worth dropping the connection over.
    fn read_string(&mut self, len: usize) -> Result<String, MessageDecodeError> {
        Ok(String::from_utf8_lossy(self.read_bytes(len)?).into_owned())
    }
}

pub struct GameMessageCodec {}

impl Decoder for GameMessageCodec {
    type Item = ServerMessage;
    type Error = MessageDecodeError;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if buf.len() < 2 {
            return Ok(None);
        }

        let payload_len = u16::from_le_bytes([buf[0], buf[1]]) as usize;

        if buf.len() < 2 + payload_len {
            return Ok(None);
        }

        // Split the frame off before decoding anything: every read below is
        // then bounded by this payload, so a corrupt message can neither run
        // into the next frame's bytes nor leave the stream misaligned. The
        // frame is consumed either way, so an error here is reportable
        // without desyncing the framing.
        buf.advance(2);
        let payload = buf.split_to(payload_len);

        let mut reader = Reader::new(&payload);
        let message = decode_message(&mut reader)?;
        if reader.remaining() != 0 {
            return Err(MessageDecodeError::TrailingBytes(reader.remaining()));
        }

        Ok(Some(message))
    }
}

fn decode_message(buf: &mut Reader) -> Result<ServerMessage, MessageDecodeError> {
    match buf.read_u8()? {
        SRV_PONG => Ok(ServerMessage::Pong),
        SRV_LOGIN_ERROR => Ok(ServerMessage::LoginError),
        SRV_DESCRIBE_PLAYER => {
            let agent_id = AgentId(buf.read_u16_le()?);
            let position = decode_position(buf)?;
            let facing = decode_facing(buf)?;
            let name_len = buf.read_u16_le()? as usize;
            let name = buf.read_string(name_len)?;
            let level = buf.read_u16_le()?;
            let health = Health {
                current: buf.read_u32_le()?,
                max: buf.read_u32_le()?,
            };
            let mana = Mana {
                current: buf.read_u32_le()?,
                max: buf.read_u32_le()?,
            };
            let outfit = OutfitId(buf.read_u16_le()?);
            let colors = OutfitColors::new(
                buf.read_u8()?,
                buf.read_u8()?,
                buf.read_u8()?,
                buf.read_u8()?,
            );
            let speed = buf.read_u16_le()?;
            let capacity = buf.read_u32_le()?;
            let inventory_head = decode_optional_item(buf)?;
            let inventory_amulet = decode_optional_item(buf)?;
            let inventory_backpack = decode_optional_item(buf)?;
            let inventory_chest = decode_optional_item(buf)?;
            let inventory_right_hand = decode_optional_item(buf)?;
            let inventory_left_hand = decode_optional_item(buf)?;
            let inventory_legs = decode_optional_item(buf)?;
            let inventory_feet = decode_optional_item(buf)?;
            let inventory_ring = decode_optional_item(buf)?;
            let inventory_trinket = decode_optional_item(buf)?;
            Ok(ServerMessage::DescribePlayer {
                agent_id,
                position,
                facing,
                name,
                level,
                health,
                mana,
                outfit: (outfit, colors),
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
            })
        }
        SRV_DESCRIBE_MAP => {
            let center = decode_position(buf)?;
            let floor = buf.read_u8()?;
            let mut tiles = Box::new([[None; STACK_MAX_VISIBLE_ITEMS]; TILES_X * TILES_Y]);
            for tile in tiles.iter_mut() {
                *tile = decode_tile(buf)?;
            }
            Ok(ServerMessage::DescribeMap {
                tiles,
                floor,
                center,
            })
        }
        SRV_TILE_UPDATED => {
            let position = decode_position(buf)?;
            let items = Box::new(decode_tile(buf)?);
            Ok(ServerMessage::TileUpdated { position, items })
        }
        SRV_PLAYER_WALK_ACK => {
            let position = decode_position(buf)?;
            let mut floor_tiles: Vec<(u8, Box<[ItemStack]>)> = Vec::new();
            let mut floor = buf.read_u8()?;
            while floor != 0xFF {
                let tiles_len = buf.read_u8()?;
                let mut tiles = Vec::with_capacity(tiles_len as usize);
                for _ in 0..tiles_len {
                    tiles.push(decode_tile(buf)?);
                }
                floor_tiles.push((floor, tiles.into_boxed_slice()));
                floor = buf.read_u8()?;
            }
            Ok(ServerMessage::PlayerWalkAck {
                position,
                tiles: floor_tiles,
            })
        }
        SRV_PLAYER_POS => {
            let position = decode_position(buf)?;
            Ok(ServerMessage::PlayerPosition { position })
        }
        SRV_TEXT_MESSAGE => {
            let text_len = buf.read_u16_le()? as usize;
            let text = buf.read_string(text_len)?;
            let message_type = decode_text_type(buf.read_u8()?)?;
            Ok(ServerMessage::TextMessage { text, message_type })
        }
        SRV_OPEN_CONTAINER => {
            let container_id = ContainerId(buf.read_u16_le()?);
            let capacity = buf.read_u8()?;
            let has_parent = buf.read_u8()? != 0;
            let title_len = buf.read_u8()? as usize;
            let title = buf.read_string(title_len)?;
            let items = decode_items(buf)?;
            Ok(ServerMessage::OpenContainer {
                container_id,
                capacity,
                has_parent,
                title,
                items,
            })
        }
        SRV_UPDATE_CONTAINER => {
            let container_id = ContainerId(buf.read_u16_le()?);
            let items = decode_items(buf)?;
            Ok(ServerMessage::UpdateContainer {
                container_id,
                items,
            })
        }
        SRV_CONTAINER_CLOSED => {
            let container_id = ContainerId(buf.read_u16_le()?);
            Ok(ServerMessage::ContainerClosed { container_id })
        }
        SRV_PLAYER_WALK_DENIED => Ok(ServerMessage::PlayerWalkDenied),
        SRV_INVETORY_SLOT_UPDATED => {
            let slot = InventorySlot::from_id(buf.read_u8()?);
            let Some(slot) = slot else {
                return Err(MessageDecodeError::WrongSequence);
            };
            let item_id = decode_optional_item(buf)?;
            Ok(ServerMessage::IventorySlotUpdated { slot, item_id })
        }
        SRV_PLAYER_CAPACITY_UPDATED => {
            let capacity = buf.read_u32_le()?;
            Ok(ServerMessage::PlayerCapacityUpdated { capacity })
        }
        SRV_AGENT_DIRECTION_CHANGED => Ok(ServerMessage::AgentChangedDirection {
            agent_id: AgentId(buf.read_u16_le()?),
            facing: decode_facing(buf)?,
        }),
        SRV_REMOVE_AGENT => Ok(ServerMessage::RemoveAgent {
            agent_id: AgentId(buf.read_u16_le()?),
        }),
        SRV_TARGET_LOST => Ok(ServerMessage::TargetLost {
            seq: buf.read_u32_le()?,
        }),
        SRV_MOVE_AGENT => Ok(ServerMessage::MoveAgent {
            agent_id: AgentId(buf.read_u16_le()?),
            direction: decode_direction(buf)?,
            from: decode_position(buf)?,
        }),
        SRV_SPAWN_AGENT => {
            let agent_id = AgentId(buf.read_u16_le()?);
            let position = decode_position(buf)?;
            let facing = decode_facing(buf)?;
            let name_len = buf.read_u16_le()? as usize;
            let name = buf.read_string(name_len)?;
            let health = buf.read_u32_le()?;
            let outfit_id = OutfitId(buf.read_u16_le()?);
            let colors = OutfitColors::new(
                buf.read_u8()?,
                buf.read_u8()?,
                buf.read_u8()?,
                buf.read_u8()?,
            );
            let speed = buf.read_u16_le()?;
            Ok(ServerMessage::SpawnAgent {
                agent_id,
                outfit: (outfit_id, colors),
                position,
                facing,
                name,
                health,
                speed,
            })
        }
        SRV_TELEPORT_AGENT => Ok(ServerMessage::TeleportAgent {
            agent_id: AgentId(buf.read_u16_le()?),
            position: decode_position(buf)?,
        }),
        SRV_CHAT_MESSAGE => {
            let author_len = buf.read_u16_le()? as usize;
            let author = buf.read_string(author_len)?;
            let message_type = decode_chat_message_type(buf.read_u8()?)?;
            let channel = buf.read_u16_le()?;
            let position = decode_optional_position(buf)?;
            let text_len = buf.read_u16_le()? as usize;
            let text = buf.read_string(text_len)?;
            Ok(ServerMessage::ChatMessage {
                author,
                message_type,
                channel,
                position,
                text,
            })
        }
        SRV_CHANNEL_LIST => {
            let count = buf.read_u16_le()? as usize;
            let mut channels = Vec::new();
            for _ in 0..count {
                let id = buf.read_u16_le()?;
                let name_len = buf.read_u16_le()? as usize;
                let name = buf.read_string(name_len)?;
                channels.push((id, name));
            }
            Ok(ServerMessage::ChannelList { channels })
        }
        SRV_PRIVATE_CHAT_OPENED => {
            let name_len = buf.read_u16_le()? as usize;
            let name = buf.read_string(name_len)?;
            Ok(ServerMessage::PrivateChatOpened { name })
        }
        SRV_FLOATING_TEXT => {
            let text_len = buf.read_u16_le()? as usize;
            let text = buf.read_string(text_len)?;
            let position = decode_position(buf)?;
            let text_type = decode_floating_text_type(buf.read_u8()?)?;
            let color = decode_optional_color(buf)?;
            Ok(ServerMessage::FloatingText {
                text,
                position,
                text_type,
                color,
            })
        }
        SRV_AGENT_LIFE_UPDATED => {
            let agent_id = AgentId(buf.read_u16_le()?);
            let current = buf.read_u32_le()?;
            let max = buf.read_u32_le()?;
            Ok(ServerMessage::AgentLifeUpdated {
                agent_id,
                current,
                max,
            })
        }
        SRV_AGENT_MANA_UPDATED => {
            let agent_id = AgentId(buf.read_u16_le()?);
            let current = buf.read_u32_le()?;
            let max = buf.read_u32_le()?;
            Ok(ServerMessage::AgentManaUpdated {
                agent_id,
                current,
                max,
            })
        }
        SRV_PLAYER_SKILLS => {
            let experience = buf.read_u64_le()?;
            let count = buf.read_u8()?;
            let mut skills = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let id = buf.read_u8()?;
                let level = buf.read_u16_le()?;
                let percent_bp = buf.read_u16_le()?;
                match SkillType::from_id(id) {
                    Some(skill) => skills.push((skill, SkillProgress { level, percent_bp })),
                    None => warn!("ignoring a skill id this build does not know: {id}"),
                }
            }
            Ok(ServerMessage::PlayerSkills { experience, skills })
        }
        SRV_SKILL_UPDATED => {
            let id = buf.read_u8()?;
            let level = buf.read_u16_le()?;
            let percent_bp = buf.read_u16_le()?;
            let Some(skill) = SkillType::from_id(id) else {
                return Err(MessageDecodeError::WrongSequence);
            };
            Ok(ServerMessage::SkillUpdated {
                skill,
                progress: SkillProgress { level, percent_bp },
            })
        }
        SRV_EXPERIENCE_UPDATED => Ok(ServerMessage::ExperienceUpdated {
            experience: buf.read_u64_le()?,
        }),
        SRV_SHOW_EFFECT => {
            let effect_id = EffectId(buf.read_u16_le()?);
            let position = decode_position(buf)?;
            let delta = decode_position_delta(buf)?;
            Ok(ServerMessage::ShowEffect {
                effect_id,
                position,
                delta,
            })
        }
        SRV_SPELL_CAST => Ok(ServerMessage::SpellCast {
            spell_id: SpellId(buf.read_u16_le()?),
            spell_cooldown_ms: buf.read_u32_le()?,
            group_cooldown_ms: buf.read_u32_le()?,
        }),
        SRV_SPELL_LIST => {
            let count = buf.read_u16_le()?;
            let mut spells = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let id = SpellId(buf.read_u16_le()?);
                let name_len = buf.read_u16_le()? as usize;
                let name = buf.read_string(name_len)?;
                let words_len = buf.read_u16_le()? as usize;
                let words = buf.read_string(words_len)?;
                let level = buf.read_u16_le()?;
                let icon = buf.read_u16_le()?;
                let aimable = match buf.read_u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(MessageDecodeError::WrongSequence),
                };
                let group =
                    SpellGroup::from_id(buf.read_u8()?).ok_or(MessageDecodeError::WrongSequence)?;
                spells.push(SpellInfo {
                    id,
                    name,
                    words,
                    level,
                    icon,
                    aimable,
                    group,
                });
            }
            Ok(ServerMessage::SpellList { spells })
        }
        SRV_LAUNCH_MISSILE => {
            let from = decode_position(buf)?;
            let to = decode_position(buf)?;
            let missile_id = MissileId(buf.read_u16_le()?);
            Ok(ServerMessage::LaunchMissile {
                from,
                to,
                missile_id,
            })
        }
        SRV_AGENT_SPEED_UPDATED => Ok(ServerMessage::AgentSpeedUpdated {
            agent_id: AgentId(buf.read_u16_le()?),
            speed: buf.read_u16_le()?,
        }),
        _ => Err(MessageDecodeError::WrongSequence),
    }
}

fn decode_tile(buf: &mut Reader) -> Result<ItemStack, MessageDecodeError> {
    let mut tile = [None; STACK_MAX_VISIBLE_ITEMS];
    let mut i = 0;
    loop {
        let id = buf.read_u16_le()?;
        if id == 0xFFFF {
            break;
        }
        let amount = buf.read_u8()?;
        if i < STACK_MAX_VISIBLE_ITEMS {
            tile[i] = Some((ItemId(id), amount));
            i += 1;
        }
    }
    Ok(tile)
}

fn decode_items(buf: &mut Reader) -> Result<Box<[Option<(ItemId, u8)>]>, MessageDecodeError> {
    let mut items = Vec::new();
    loop {
        let id = buf.read_u16_le()?;
        if id == 0xFFFF {
            break;
        }
        let amount = buf.read_u8()?;
        items.push(Some((ItemId(id), amount)));
    }
    Ok(items.into())
}

/// A flag byte, then a position only if the flag is set -- the same shape as
/// `decode_optional_color`, and for the same reason: reading five bytes that were
/// never written misaligns the rest of the frame.
fn decode_optional_position(buf: &mut Reader) -> Result<Option<Position>, MessageDecodeError> {
    if buf.read_u8()? == 0 {
        return Ok(None);
    }
    Ok(Some(decode_position(buf)?))
}

fn decode_position(buf: &mut Reader) -> Result<Position, MessageDecodeError> {
    Ok(Position {
        x: buf.read_u16_le()?,
        y: buf.read_u16_le()?,
        z: buf.read_u8()?,
    })
}

fn decode_facing(buf: &mut Reader) -> Result<FacingDirection, MessageDecodeError> {
    match buf.read_u8()? {
        1 => Ok(FacingDirection::North),
        2 => Ok(FacingDirection::East),
        3 => Ok(FacingDirection::South),
        4 => Ok(FacingDirection::West),
        _ => Err(MessageDecodeError::WrongSequence),
    }
}

fn decode_text_type(b: u8) -> Result<TextMessageType, MessageDecodeError> {
    match b {
        1 => Ok(TextMessageType::ActionDenied),
        2 => Ok(TextMessageType::Look),
        _ => Err(MessageDecodeError::WrongSequence),
    }
}

fn decode_chat_message_type(b: u8) -> Result<ChatMessageType, MessageDecodeError> {
    match b {
        0x01 => Ok(ChatMessageType::Local),
        0x02 => Ok(ChatMessageType::Private),
        0x03 => Ok(ChatMessageType::Channel),
        _ => Err(MessageDecodeError::WrongSequence),
    }
}

fn decode_floating_text_type(b: u8) -> Result<FloatingTextType, MessageDecodeError> {
    match b {
        0x01 => Ok(FloatingTextType::HitPoints),
        0x02 => Ok(FloatingTextType::CreatureSay),
        _ => Err(MessageDecodeError::WrongSequence),
    }
}

/// A flag byte, then three colour bytes only if the flag is set. Reading the
/// colour unconditionally would consume bytes the sender never wrote and leave the
/// rest of the frame misaligned.
fn decode_optional_color(buf: &mut Reader) -> Result<Option<(u8, u8, u8)>, MessageDecodeError> {
    if buf.read_u8()? == 0 {
        return Ok(None);
    }
    let r = buf.read_u8()?;
    let g = buf.read_u8()?;
    let b = buf.read_u8()?;
    Ok(Some((r, g, b)))
}

fn decode_optional_item(buf: &mut Reader) -> Result<Option<ItemId>, MessageDecodeError> {
    let item_id = buf.read_u16_le()?;
    if item_id == 0xFFFF {
        Ok(None)
    } else {
        Ok(Some(ItemId(item_id)))
    }
}

fn decode_position_delta(buf: &mut Reader) -> Result<Vec<(i8, i8)>, MessageDecodeError> {
    let rem = buf.remaining();

    if rem == 0 {
        return Ok(Vec::with_capacity(0));
    }

    // A delta is an `(i8, i8)` pair, so a well-formed list is an even number of
    // bytes by construction — the even case is the one to decode, not the one to
    // reject. An odd remainder means the message was truncated mid-pair, and
    // decoding `rem / 2` pairs anyway would silently swallow the stray byte.
    if !rem.is_multiple_of(2) {
        return Err(MessageDecodeError::TrailingBytes(rem % 2));
    }
    let size = rem / 2;
    let mut delta = Vec::with_capacity(size);
    for _ in 0..size {
        delta.push((buf.read_i8()?, buf.read_i8()?));
    }
    Ok(delta)
}

fn encode_optional_agent(agent_id: Option<AgentId>, dst: &mut BytesMut) {
    dst.put_u16_le(agent_id.map_or(0xFFFF, |id| id.0));
}

fn decode_direction(buf: &mut Reader) -> Result<WalkingDirection, MessageDecodeError> {
    match buf.read_u8()? {
        0x00 => Ok(WalkingDirection::North),
        0x01 => Ok(WalkingDirection::East),
        0x02 => Ok(WalkingDirection::West),
        0x03 => Ok(WalkingDirection::South),
        0x04 => Ok(WalkingDirection::NorthEast),
        0x05 => Ok(WalkingDirection::NorthWest),
        0x06 => Ok(WalkingDirection::SouthEast),
        0x07 => Ok(WalkingDirection::SouthWest),
        _ => Err(MessageDecodeError::WrongSequence),
    }
}

impl Encoder for GameMessageCodec {
    type Item<'a> = ClientMessage;
    type Error = std::io::Error;

    fn encode(&mut self, item: ClientMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let len_offset = dst.len();
        dst.put_u16_le(0); // placeholder for payload length

        match item {
            ClientMessage::Ping => {
                dst.put_u8(CLI_PING);
            }
            ClientMessage::Login { auth_token } => {
                dst.put_u8(CLI_LOGIN);
                dst.put_slice(auth_token.as_bytes());
            }
            ClientMessage::MovePlayer { direction } => {
                dst.put_u8(CLI_MOVE_PLAYER);
                encode_direction(&direction, dst);
            }
            ClientMessage::MoveItem {
                from,
                item_id,
                amount,
                stack_index,
                to,
            } => {
                dst.put_u8(CLI_MOVE_ITEM);
                encode_position(from, dst);
                dst.put_u16_le(item_id.0);
                dst.put_u8(amount);
                dst.put_u8(stack_index);
                encode_position(to, dst);
            }
            ClientMessage::GetPlayerPosition => {
                dst.put_u8(CLI_GET_PLAYER_POS);
            }
            ClientMessage::UseItem {
                position,
                item_id,
                stack_index,
            } => {
                dst.put_u8(CLI_USE_ITEM);
                encode_position(position, dst);
                dst.put_u16_le(item_id.0);
                dst.put_u8(stack_index);
            }
            ClientMessage::CloseContainer { container_id } => {
                dst.put_u8(CLI_CLOSE_CONTAINER);
                dst.put_u16_le(container_id.0);
            }
            ClientMessage::OpenParentContainer { container_id } => {
                dst.put_u8(CLI_OPEN_PARENT_CONTAINER);
                dst.put_u16_le(container_id.0);
            }
            ClientMessage::ChangeDirection { direction } => {
                dst.put_u8(CLI_CHANGE_DIRECTION);
                encode_facing(&direction, dst);
            }
            ClientMessage::Logout => dst.put_u8(CLI_LOGOUT),
            ClientMessage::UseItemWith {
                source,
                source_item_id,
                source_index,
                target,
                target_item_id,
                target_index,
                target_agent,
            } => {
                dst.put_u8(CLI_USE_ITEM_WITH);
                encode_position(source, dst);
                dst.put_u16_le(source_item_id.0);
                dst.put_u8(source_index);
                encode_position(target, dst);
                dst.put_u16_le(target_item_id.0);
                dst.put_u8(target_index);
                encode_optional_agent(target_agent, dst);
            }
            ClientMessage::Look { position } => {
                dst.put_u8(CLI_LOOK);
                encode_position(position, dst);
            }
            ClientMessage::Say { message, target } => {
                dst.put_u8(CLI_SAY);
                dst.put_u8(encode_chat_message_type(target.message_type()));
                match &target {
                    SayTarget::Local => {}
                    SayTarget::Channel(channel) => dst.put_u16_le(*channel),
                    SayTarget::Player(name) => {
                        let name_bytes = name.as_bytes();
                        dst.put_u16_le(name_bytes.len() as u16);
                        dst.put_slice(name_bytes);
                    }
                }
                // Trailing: the server derives the length from the frame, matching
                // CLI_LOGIN. Do not write an inline length here.
                dst.put_slice(message.as_bytes());
            }
            ClientMessage::RequestChannels => dst.put_u8(CLI_REQUEST_CHANNELS),
            ClientMessage::OpenChannel { channel } => {
                dst.put_u8(CLI_OPEN_CHANNEL);
                dst.put_u16_le(channel);
            }
            ClientMessage::CloseChannel { channel } => {
                dst.put_u8(CLI_CLOSE_CHANNEL);
                dst.put_u16_le(channel);
            }
            ClientMessage::SetTarget { agent_id, seq } => {
                dst.put_u8(CLI_SET_TARGET);
                encode_optional_agent(agent_id, dst);
                dst.put_u32_le(seq);
            }
            ClientMessage::OpenPmChat { name } => {
                dst.put_u8(CLI_OPEN_PM_CHAT);
                dst.put_slice(name.as_bytes());
            }
            ClientMessage::CastSpell { spell_id, target } => {
                dst.put_u8(CLI_CAST_SPELL);
                dst.put_u16_le(spell_id.0);
                encode_spell_target(target, dst);
            }
        }

        let payload_len = (dst.len() - len_offset - 2) as u16;
        dst[len_offset..len_offset + 2].copy_from_slice(&payload_len.to_le_bytes());

        Ok(())
    }
}

fn encode_spell_target(target: SpellTarget, dst: &mut BytesMut) {
    match target {
        SpellTarget::None => dst.put_u8(0x00),
        SpellTarget::Agent(agent_id) => {
            dst.put_u8(0x01);
            dst.put_u16_le(agent_id.0);
        }
        SpellTarget::Position(position) => {
            dst.put_u8(0x02);
            encode_position(position, dst);
        }
    }
}

fn encode_position(pos: Position, dst: &mut BytesMut) {
    dst.put_u16_le(pos.x);
    dst.put_u16_le(pos.y);
    dst.put_u8(pos.z);
}

fn encode_direction(d: &WalkingDirection, dst: &mut BytesMut) {
    let value = match d {
        WalkingDirection::North => 0x00,
        WalkingDirection::East => 0x01,
        WalkingDirection::West => 0x02,
        WalkingDirection::South => 0x03,
        WalkingDirection::NorthEast => 0x04,
        WalkingDirection::NorthWest => 0x05,
        WalkingDirection::SouthEast => 0x06,
        WalkingDirection::SouthWest => 0x07,
    };
    dst.put_u8(value);
}

fn encode_facing(d: &FacingDirection, dst: &mut BytesMut) {
    let value = match d {
        FacingDirection::North => 1,
        FacingDirection::East => 2,
        FacingDirection::South => 3,
        FacingDirection::West => 4,
    };
    dst.put_u8(value);
}

fn encode_chat_message_type(t: ChatMessageType) -> u8 {
    match t {
        ChatMessageType::Local => 0x01,
        ChatMessageType::Private => 0x02,
        ChatMessageType::Channel => 0x03,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use asynchronous_codec::Encoder;

    /// Encodes `msg` and returns the payload with the 2-byte length prefix stripped,
    /// after asserting the prefix matches the real payload length.
    fn payload_of(msg: ClientMessage) -> Vec<u8> {
        let mut codec = GameMessageCodec {};
        let mut buf = BytesMut::new();
        codec.encode(msg, &mut buf).unwrap();
        let declared = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        assert_eq!(
            declared,
            buf.len() - 2,
            "length prefix must cover the payload"
        );
        buf[2..].to_vec()
    }

    /// The three `SpellTarget` variants, as literal bytes. The server's
    /// `decode_spell_target` reads exactly this frame; nothing else pins the
    /// opcode or the variant tags on either side.
    #[test]
    fn cast_spell_encodes_each_target_variant() {
        assert_eq!(
            payload_of(ClientMessage::CastSpell {
                spell_id: SpellId(4),
                target: SpellTarget::None,
            }),
            vec![CLI_CAST_SPELL, 0x04, 0x00, 0x00],
        );
        assert_eq!(
            payload_of(ClientMessage::CastSpell {
                spell_id: SpellId(4),
                target: SpellTarget::Agent(AgentId(7)),
            }),
            vec![CLI_CAST_SPELL, 0x04, 0x00, 0x01, 0x07, 0x00],
        );
        assert_eq!(
            payload_of(ClientMessage::CastSpell {
                spell_id: SpellId(4),
                target: SpellTarget::Position(Position::new(1000, 1001, 7)),
            }),
            vec![
                CLI_CAST_SPELL,
                0x04,
                0x00,
                0x02,
                0xE8,
                0x03,
                0xE9,
                0x03,
                0x07
            ],
        );
    }

    #[test]
    fn spell_cast_decodes_both_cooldowns() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&[
            11,
            0,
            SRV_SPELL_CAST,
            0x04,
            0x00,
            0xD0,
            0x07,
            0x00,
            0x00,
            0xA0,
            0x0F,
            0x00,
            0x00,
        ]);

        match (GameMessageCodec {}).decode(&mut buf).unwrap().unwrap() {
            ServerMessage::SpellCast {
                spell_id,
                spell_cooldown_ms,
                group_cooldown_ms,
            } => {
                assert_eq!(spell_id, SpellId(4));
                assert_eq!(spell_cooldown_ms, 2000);
                assert_eq!(group_cooldown_ms, 4000);
            }
            other => panic!("expected SpellCast, got {other:?}"),
        }
    }

    #[test]
    fn two_decodes_as_creature_say() {
        assert_eq!(
            decode_floating_text_type(0x02).unwrap(),
            FloatingTextType::CreatureSay
        );
    }

    #[test]
    fn say_encodes_type_then_trailing_message() {
        let payload = payload_of(ClientMessage::Say {
            message: "hello".to_owned(),
            target: SayTarget::Local,
        });
        assert_eq!(payload[0], CLI_SAY);
        assert_eq!(payload[1], 0x01, "Local");
        assert_eq!(
            &payload[2..],
            b"hello",
            "message is trailing, no inline length"
        );
    }

    #[test]
    fn say_carries_the_channel_for_a_channel_line() {
        let payload = payload_of(ClientMessage::Say {
            message: "hi".to_owned(),
            target: SayTarget::Channel(7),
        });
        assert_eq!(payload[1], 0x03, "Channel");
        assert_eq!(u16::from_le_bytes([payload[2], payload[3]]), 7);
        assert_eq!(&payload[4..], b"hi");
    }

    /// The recipient is a length-prefixed name, so the message can only be found after
    /// it. The server decodes this same literal frame in
    /// `decode_private_say_carries_the_recipient_name`.
    #[test]
    fn encodes_a_private_say() {
        let payload = payload_of(ClientMessage::Say {
            message: "hi".to_owned(),
            target: SayTarget::Player("Rizael".to_owned()),
        });
        assert_eq!(payload[0], CLI_SAY);
        assert_eq!(payload[1], 0x02, "Private");
        assert_eq!(
            u16::from_le_bytes([payload[2], payload[3]]),
            6,
            "name length"
        );
        assert_eq!(&payload[4..10], b"Rizael");
        assert_eq!(&payload[10..], b"hi");
    }

    #[test]
    fn channel_control_messages_encode() {
        assert_eq!(
            payload_of(ClientMessage::RequestChannels),
            vec![CLI_REQUEST_CHANNELS]
        );

        let payload = payload_of(ClientMessage::OpenChannel { channel: 2 });
        assert_eq!(payload[0], CLI_OPEN_CHANNEL);
        assert_eq!(u16::from_le_bytes([payload[1], payload[2]]), 2);

        let payload = payload_of(ClientMessage::CloseChannel { channel: 2 });
        assert_eq!(payload[0], CLI_CLOSE_CHANNEL);
        assert_eq!(u16::from_le_bytes([payload[1], payload[2]]), 2);
    }

    #[test]
    fn open_pm_chat_encodes_a_trailing_name() {
        let payload = payload_of(ClientMessage::OpenPmChat {
            name: "Rizael".to_owned(),
        });
        assert_eq!(payload[0], CLI_OPEN_PM_CHAT);
        assert_eq!(&payload[1..], b"Rizael");
    }

    /// The literal frame the server's `set_target_decodes_some_and_none`
    /// (rustibia-server, crates/server/src/messages.rs) reads. The opcode is a
    /// number on purpose, so a constant changed on one side fails a test.
    #[test]
    fn set_target_encodes_some_and_none() {
        let payload = payload_of(ClientMessage::SetTarget {
            agent_id: Some(AgentId(7)),
            seq: 5,
        });
        assert_eq!(payload[0], 17);
        assert_eq!(u16::from_le_bytes([payload[1], payload[2]]), 7);
        assert_eq!(
            u32::from_le_bytes([payload[3], payload[4], payload[5], payload[6]]),
            5
        );

        let payload = payload_of(ClientMessage::SetTarget {
            agent_id: None,
            seq: 6,
        });
        assert_eq!(payload[0], 17);
        assert_eq!(u16::from_le_bytes([payload[1], payload[2]]), 0xFFFF);
    }

    #[test]
    fn use_item_with_encodes_the_target_agent_last() {
        let payload = payload_of(ClientMessage::UseItemWith {
            source: Position { x: 10, y: 11, z: 7 },
            source_item_id: ItemId(1234),
            source_index: 0,
            target: Position { x: 12, y: 13, z: 7 },
            target_item_id: ItemId(5678),
            target_index: 1,
            target_agent: Some(AgentId(42)),
        });

        assert_eq!(&payload[payload.len() - 2..], &[42, 0]);
    }

    #[test]
    fn no_target_agent_encodes_as_the_sentinel() {
        let payload = payload_of(ClientMessage::UseItemWith {
            source: Position { x: 10, y: 11, z: 7 },
            source_item_id: ItemId(1234),
            source_index: 0,
            target: Position { x: 12, y: 13, z: 7 },
            target_item_id: ItemId(5678),
            target_index: 1,
            target_agent: None,
        });

        assert_eq!(&payload[payload.len() - 2..], &[0xFF, 0xFF]);
    }

    use asynchronous_codec::Decoder;

    /// Wraps `payload` in the 2-byte length prefix the codec expects.
    fn frame(payload: &[u8]) -> BytesMut {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        buf.extend_from_slice(payload);
        buf
    }

    #[test]
    fn decodes_a_chat_message() {
        let mut payload = vec![SRV_CHAT_MESSAGE];
        payload.extend_from_slice(&6u16.to_le_bytes()); // author length
        payload.extend_from_slice(b"Rizael");
        payload.push(0x03); // Channel
        payload.extend_from_slice(&7u16.to_le_bytes()); // channel
        payload.push(0x00); // position absent
        payload.extend_from_slice(&5u16.to_le_bytes()); // len
        payload.extend_from_slice(b"hello");

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        match codec.decode(&mut buf).unwrap().unwrap() {
            ServerMessage::ChatMessage {
                author,
                message_type,
                channel,
                position,
                text,
            } => {
                assert_eq!(author, "Rizael");
                assert!(matches!(message_type, ChatMessageType::Channel));
                assert_eq!(channel, 7);
                assert_eq!(position, None, "no position bytes may be consumed");
                assert_eq!(text, "hello");
            }
            other => panic!("expected ChatMessage, got {other:?}"),
        }
        assert!(buf.is_empty(), "the frame must be fully consumed");
    }

    /// The exact byte layout `encode_chat_message_carries_the_speaker_s_tile_for_local_speech`
    /// produces in the server repository. The pair is the pin: local speech is the
    /// only line that carries a tile, and the tile is what the bubble hangs on.
    #[test]
    fn decodes_a_local_chat_message() {
        let mut payload = vec![SRV_CHAT_MESSAGE];
        payload.extend_from_slice(&6u16.to_le_bytes()); // author length
        payload.extend_from_slice(b"Rizael");
        payload.push(0x01); // Local
        payload.extend_from_slice(&0u16.to_le_bytes()); // channel
        payload.push(0x01); // position present
        payload.extend_from_slice(&300u16.to_le_bytes()); // position x
        payload.extend_from_slice(&400u16.to_le_bytes()); // position y
        payload.push(7); // position z
        payload.extend_from_slice(&5u16.to_le_bytes()); // len
        payload.extend_from_slice(b"hello");

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        match codec.decode(&mut buf).unwrap().unwrap() {
            ServerMessage::ChatMessage {
                message_type,
                position,
                text,
                ..
            } => {
                assert!(matches!(message_type, ChatMessageType::Local));
                assert_eq!(position, Some(Position::new(300, 400, 7)));
                assert_eq!(text, "hello");
            }
            other => panic!("expected ChatMessage, got {other:?}"),
        }
        assert!(buf.is_empty(), "the frame must be fully consumed");
    }

    #[test]
    fn decodes_private_chat_opened() {
        let mut payload = vec![SRV_PRIVATE_CHAT_OPENED];
        payload.extend_from_slice(&6u16.to_le_bytes());
        payload.extend_from_slice(b"Rizael");

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        match codec.decode(&mut buf).unwrap().unwrap() {
            ServerMessage::PrivateChatOpened { name } => assert_eq!(name, "Rizael"),
            other => panic!("expected PrivateChatOpened, got {other:?}"),
        }
        assert!(buf.is_empty());
    }

    /// Three entries with different-length names and a non-contiguous third id. A
    /// single-entry test cannot tell a correct loop from one that stops after the
    /// first entry, or that writes an index instead of the real id.
    #[test]
    fn decodes_every_entry_of_a_channel_list() {
        let entries: [(u16, &[u8]); 3] = [(1, b"World Chat"), (2, b"Advertising"), (7, b"Help")];
        let mut payload = vec![SRV_CHANNEL_LIST];
        payload.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        for (id, name) in entries.iter() {
            payload.extend_from_slice(&id.to_le_bytes());
            payload.extend_from_slice(&(name.len() as u16).to_le_bytes());
            payload.extend_from_slice(name);
        }

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        match codec.decode(&mut buf).unwrap().unwrap() {
            ServerMessage::ChannelList { channels } => {
                assert_eq!(channels.len(), 3);
                assert_eq!(channels[0], (1, "World Chat".to_owned()));
                assert_eq!(channels[1], (2, "Advertising".to_owned()));
                assert_eq!(channels[2], (7, "Help".to_owned()));
            }
            other => panic!("expected ChannelList, got {other:?}"),
        }
        assert!(buf.is_empty(), "the frame must be fully consumed");
    }

    /// The exact byte layout `encode_floating_text_with_a_colour` produces in the
    /// server repository. The pair is the pin: a divergence in field order, length
    /// prefix width or discriminant fails one of the two tests.
    #[test]
    fn decodes_a_floating_text_with_a_colour() {
        let mut payload = vec![SRV_FLOATING_TEXT];
        payload.extend_from_slice(&3u16.to_le_bytes()); // text length
        payload.extend_from_slice(b"-25");
        payload.extend_from_slice(&0x0201u16.to_le_bytes()); // position x
        payload.extend_from_slice(&0x0403u16.to_le_bytes()); // position y
        payload.push(7); // position z
        payload.push(0x01); // HitPoints
        payload.push(0x01); // colour present
        payload.extend_from_slice(&[255, 0, 64]);

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        match codec.decode(&mut buf).unwrap().unwrap() {
            ServerMessage::FloatingText {
                text,
                position,
                text_type,
                color,
            } => {
                assert_eq!(text, "-25");
                assert_eq!(position, Position::new(0x0201, 0x0403, 7));
                assert!(matches!(text_type, FloatingTextType::HitPoints));
                assert_eq!(color, Some((255, 0, 64)));
            }
            other => panic!("expected FloatingText, got {other:?}"),
        }
        assert!(buf.is_empty(), "the frame must be fully consumed");
    }

    #[test]
    fn decodes_a_floating_text_without_a_colour() {
        let mut payload = vec![SRV_FLOATING_TEXT];
        payload.extend_from_slice(&2u16.to_le_bytes());
        payload.extend_from_slice(b"hi");
        payload.extend_from_slice(&300u16.to_le_bytes()); // position x
        payload.extend_from_slice(&400u16.to_le_bytes()); // position y
        payload.push(7); // position z
        payload.push(0x02); // CreatureSay
        payload.push(0x00); // colour absent

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        match codec.decode(&mut buf).unwrap().unwrap() {
            ServerMessage::FloatingText {
                position,
                text_type,
                color,
                ..
            } => {
                assert_eq!(position, Position::new(300, 400, 7));
                assert!(matches!(text_type, FloatingTextType::CreatureSay));
                assert_eq!(color, None, "no colour bytes may be consumed");
            }
            other => panic!("expected FloatingText, got {other:?}"),
        }
        assert!(
            buf.is_empty(),
            "reading three colour bytes that were not sent would leave the frame short"
        );
    }

    /// The literal frame the server's `target_lost_encodes_its_seq` builds.
    #[test]
    fn target_lost_decodes_its_seq() {
        let mut codec = GameMessageCodec {};

        let mut buf = frame(&[23, 0x4D, 0x00, 0x00, 0x00]);
        assert!(matches!(
            codec.decode(&mut buf).unwrap().unwrap(),
            ServerMessage::TargetLost { seq: 77 }
        ));
        assert!(buf.is_empty(), "the frame must be fully consumed");
    }

    /// An unknown discriminant must be a decode error, not a silent default — this
    /// is the loud failure that makes a pinning test unnecessary for the enum.
    #[test]
    fn an_unknown_floating_text_type_is_rejected() {
        let mut payload = vec![SRV_FLOATING_TEXT];
        payload.extend_from_slice(&0u16.to_le_bytes()); // empty text
        payload.extend_from_slice(&10u16.to_le_bytes()); // position x
        payload.extend_from_slice(&10u16.to_le_bytes()); // position y
        payload.push(7); // position z
        payload.push(0x09); // not a type
        payload.push(0x00);

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        assert!(matches!(
            codec.decode(&mut buf),
            Err(MessageDecodeError::WrongSequence)
        ));
    }

    /// A length field larger than the payload used to index straight past the
    /// buffer and panic, killing the connection task.
    #[test]
    fn rejects_a_length_field_past_the_end_of_the_payload() {
        let mut payload = vec![SRV_PRIVATE_CHAT_OPENED];
        payload.extend_from_slice(&600u16.to_le_bytes()); // claims 600 bytes of name
        payload.extend_from_slice(b"Rizael");

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        assert!(matches!(
            codec.decode(&mut buf),
            Err(MessageDecodeError::Truncated)
        ));
        assert!(buf.is_empty(), "the bad frame is consumed, not left behind");
    }

    /// A tile whose 0xFFFF terminator never arrives used to read on into the
    /// next frame's bytes; now it stops at the payload boundary.
    #[test]
    fn rejects_a_tile_with_no_terminator() {
        let mut payload = vec![SRV_TILE_UPDATED];
        payload.extend_from_slice(&1u16.to_le_bytes()); // x
        payload.extend_from_slice(&2u16.to_le_bytes()); // y
        payload.push(7); // z
        payload.extend_from_slice(&100u16.to_le_bytes()); // item id
        payload.push(1); // amount, and then nothing

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        buf.extend_from_slice(&frame(&[SRV_PLAYER_WALK_DENIED])); // the next frame

        assert!(matches!(
            codec.decode(&mut buf),
            Err(MessageDecodeError::Truncated)
        ));
        // Framing survives: the following message still decodes.
        assert!(matches!(
            codec.decode(&mut buf).unwrap().unwrap(),
            ServerMessage::PlayerWalkDenied
        ));
    }

    /// Silent drift is worse than a disconnect: a decoder that stops early
    /// would otherwise misread every field of a version-skewed message.
    #[test]
    fn rejects_a_payload_with_unread_bytes() {
        let mut payload = vec![SRV_CONTAINER_CLOSED];
        payload.extend_from_slice(&4u16.to_le_bytes());
        payload.push(0xAB); // one byte too many

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        assert!(matches!(
            codec.decode(&mut buf),
            Err(MessageDecodeError::TrailingBytes(1))
        ));
    }

    #[test]
    fn a_truncated_frame_waits_for_more_bytes() {
        let mut codec = GameMessageCodec {};
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&10u16.to_le_bytes()); // declares 10 bytes
        buf.extend_from_slice(&[SRV_PLAYER_POS, 1, 0]); // only 3 arrived

        assert!(codec.decode(&mut buf).unwrap().is_none());
        assert_eq!(buf.len(), 5, "the partial frame is left buffered");
    }

    #[test]
    fn rejects_an_unknown_chat_message_type() {
        let mut payload = vec![SRV_CHAT_MESSAGE];
        payload.extend_from_slice(&0u16.to_le_bytes()); // author length
        payload.push(0x09); // not a valid type
        payload.extend_from_slice(&0u16.to_le_bytes());
        payload.push(0x00); // position absent
        payload.extend_from_slice(&0u16.to_le_bytes());

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);
        assert!(matches!(
            codec.decode(&mut buf),
            Err(MessageDecodeError::WrongSequence)
        ));
    }

    /// A `ShowEffect` payload: id, position, then the raw delta bytes. The delta
    /// list is trailing and carries no count, so its length is whatever is left
    /// in the frame.
    fn show_effect_payload(effect_id: u16, delta: &[(i8, i8)]) -> Vec<u8> {
        let mut payload = vec![SRV_SHOW_EFFECT];
        payload.extend_from_slice(&effect_id.to_le_bytes());
        payload.extend_from_slice(&1028u16.to_le_bytes()); // x
        payload.extend_from_slice(&1029u16.to_le_bytes()); // y
        payload.push(7); // z
        for (dx, dy) in delta {
            payload.push(*dx as u8);
            payload.push(*dy as u8);
        }
        payload
    }

    fn decoded_deltas(delta: &[(i8, i8)]) -> Vec<(i8, i8)> {
        let mut codec = GameMessageCodec {};
        let mut buf = frame(&show_effect_payload(1, delta));
        match codec.decode(&mut buf) {
            Ok(Some(ServerMessage::ShowEffect {
                effect_id,
                position,
                delta,
            })) => {
                assert_eq!(effect_id, EffectId(1));
                assert_eq!(position, Position::new(1028, 1029, 7));
                delta
            }
            other => panic!("expected a ShowEffect, got {other:?}"),
        }
    }

    /// The area-effect path: one message paints several tiles. This is what the
    /// parity guard in `decode_position_delta` used to reject — a delta is two
    /// bytes, so a well-formed list is always an even number of them, and the
    /// guard had the test the wrong way round. It did not merely drop the
    /// effect: a decode error breaks the IO loop, so the first area effect the
    /// server ever sent would disconnect the player.
    #[test]
    fn a_show_effect_carries_its_deltas() {
        assert_eq!(decoded_deltas(&[(1, 0)]), vec![(1, 0)]);
    }

    /// Negative components are the ordinary case — an area effect spreads in
    /// every direction from its centre — and they are what makes the `i8` cast
    /// on each byte load-bearing.
    #[test]
    fn a_delta_list_survives_negative_components() {
        assert_eq!(
            decoded_deltas(&[(1, 0), (0, -1), (-1, -1), (-128, 127)]),
            vec![(1, 0), (0, -1), (-1, -1), (-128, 127)]
        );
    }

    /// The only shape the server sends today, and the one path that worked
    /// while the guard was inverted. It must keep working.
    #[test]
    fn a_show_effect_without_deltas_decodes_to_an_empty_list() {
        assert_eq!(decoded_deltas(&[]), Vec::new());
    }

    /// An odd remainder means the frame was truncated mid-pair. Decoding
    /// `rem / 2` pairs anyway would silently swallow the stray byte, which is
    /// how a desynchronised stream turns into a wrong effect rather than an
    /// error.
    #[test]
    fn a_truncated_delta_list_is_rejected() {
        let mut payload = show_effect_payload(1, &[(1, 0)]);
        payload.push(0x05); // half of a second delta

        let mut codec = GameMessageCodec {};
        let mut buf = frame(&payload);

        assert!(
            matches!(
                codec.decode(&mut buf),
                Err(MessageDecodeError::TrailingBytes(_))
            ),
            "a half-delta must be an error, not a dropped byte"
        );
    }

    /// These are the exact frames the server's `messages.rs` asserts it emits.
    /// The codec is asymmetric — the server only encodes, the client only
    /// decodes — so this pair of literals is the only thing that catches a
    /// field-order or width divergence between the repositories.
    #[test]
    fn player_skills_decodes_the_servers_frame() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&[
            15,
            0,
            SRV_PLAYER_SKILLS,
            0x87,
            0x10,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            8,
            0,
            0xE1,
            0x10,
        ]);

        match (GameMessageCodec {}).decode(&mut buf).unwrap().unwrap() {
            ServerMessage::PlayerSkills { experience, skills } => {
                assert_eq!(experience, 4231);
                assert_eq!(skills.len(), 1);
                assert_eq!(skills[0].0, SkillType::Level);
                assert_eq!(skills[0].1.level, 8);
                assert_eq!(skills[0].1.percent_bp, 4321);
            }
            other => panic!("expected PlayerSkills, got {other:?}"),
        }
    }

    #[test]
    fn skill_changed_decodes_the_servers_frame() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&[6, 0, SRV_SKILL_UPDATED, 3, 12, 0, 0x2D, 0x13]);

        match (GameMessageCodec {}).decode(&mut buf).unwrap().unwrap() {
            ServerMessage::SkillUpdated { skill, progress } => {
                assert_eq!(skill, SkillType::Sword);
                assert_eq!(progress.level, 12);
                assert_eq!(progress.percent_bp, 4909);
            }
            other => panic!("expected SkillUpdated, got {other:?}"),
        }
    }

    #[test]
    fn experience_changed_decodes_the_servers_frame() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&[9, 0, SRV_EXPERIENCE_UPDATED, 0x87, 0x10, 0, 0, 0, 0, 0, 0]);

        match (GameMessageCodec {}).decode(&mut buf).unwrap().unwrap() {
            ServerMessage::ExperienceUpdated { experience } => assert_eq!(experience, 4231),
            other => panic!("expected ExperienceUpdated, got {other:?}"),
        }
    }

    /// A skill id this build does not know costs one row, not the connection:
    /// a decode error breaks the IO loop and drops the player out of the game.
    #[test]
    fn an_unknown_skill_id_is_skipped_not_fatal() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&[
            20,
            0,
            SRV_PLAYER_SKILLS,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            2,
            99,
            1,
            0,
            1,
            0,
            3,
            12,
            0,
            0x2D,
            0x13,
        ]);

        match (GameMessageCodec {}).decode(&mut buf).unwrap().unwrap() {
            ServerMessage::PlayerSkills { skills, .. } => {
                assert_eq!(skills.len(), 1);
                assert_eq!(skills[0].0, SkillType::Sword);
            }
            other => panic!("expected PlayerSkills, got {other:?}"),
        }
    }

    const SPELL_LIST_FRAME: [u8; 21] = [
        19,
        0,
        SRV_SPELL_LIST,
        1,
        0,
        4,
        0,
        2,
        0,
        b'A',
        b'b',
        2,
        0,
        b'c',
        b'd',
        12,
        0,
        29,
        0,
        1,
        2,
    ];

    #[test]
    fn spell_list_decodes_every_field() {
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&SPELL_LIST_FRAME);

        match (GameMessageCodec {}).decode(&mut buf).unwrap().unwrap() {
            ServerMessage::SpellList { spells } => assert_eq!(
                spells,
                vec![SpellInfo {
                    id: SpellId(4),
                    name: "Ab".to_owned(),
                    words: "cd".to_owned(),
                    level: 12,
                    icon: 29,
                    aimable: true,
                    group: SpellGroup::Support,
                }]
            ),
            other => panic!("expected SpellList, got {other:?}"),
        }
    }

    #[test]
    fn an_aim_flag_other_than_zero_or_one_is_refused() {
        let mut frame = SPELL_LIST_FRAME;
        frame[19] = 2;
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&frame);

        assert!((GameMessageCodec {}).decode(&mut buf).is_err());
    }

    #[test]
    fn a_group_id_the_client_does_not_know_is_refused() {
        let mut frame = SPELL_LIST_FRAME;
        frame[20] = 3;
        let mut buf = BytesMut::new();
        buf.extend_from_slice(&frame);

        assert!((GameMessageCodec {}).decode(&mut buf).is_err());
    }
}
