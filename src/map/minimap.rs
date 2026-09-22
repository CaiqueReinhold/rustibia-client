use std::{collections::HashMap, path::PathBuf};

use bevy::{prelude::*, tasks::IoTaskPool};

use crate::map::Position;

pub const CHUNK_SIZE: u16 = 64;
const TILES_PER_CHUNK: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

#[derive(Clone, Copy, Default, Debug)]
pub struct MinimapTile {
    pub color: u8,
    /// `None` = unvisited, `Some(0)` = confirmed impassable, `Some(n)` = passable with friction n
    pub friction: Option<u8>,
}

#[derive(Hash, Eq, PartialEq, Clone, Copy, Debug)]
pub struct ChunkKey {
    pub cx: u16,
    pub cy: u16,
    pub z: u8,
}

struct MinimapChunk {
    tiles: Vec<MinimapTile>,
    dirty: bool,
    gpu_dirty: bool,
}

impl MinimapChunk {
    fn new() -> Self {
        MinimapChunk {
            tiles: vec![MinimapTile::default(); TILES_PER_CHUNK],
            dirty: false,
            gpu_dirty: false,
        }
    }
}

#[derive(Resource, Default)]
pub struct MinimapData {
    chunks: HashMap<ChunkKey, MinimapChunk>,
}

impl MinimapData {
    fn key_and_offset(pos: &Position) -> (ChunkKey, usize) {
        let cx = pos.x / CHUNK_SIZE;
        let cy = pos.y / CHUNK_SIZE;
        let lx = (pos.x % CHUNK_SIZE) as usize;
        let ly = (pos.y % CHUNK_SIZE) as usize;
        (ChunkKey { cx, cy, z: pos.z }, ly * CHUNK_SIZE as usize + lx)
    }

    pub fn update_tile(&mut self, pos: &Position, color: u8, friction: u16) {
        // Chunks persist to disk one byte per friction with 0xFF reserved for
        // "unvisited", so values above 254 clamp rather than widening the format
        // and invalidating every player's explored map. The minimap only uses this
        // for A* passability and relative cost, and "very expensive but passable"
        // is the right signal for a slow tile.
        let new_friction = Some(friction.min(254) as u8);
        let (key, offset) = Self::key_and_offset(pos);
        let chunk = self.chunks.entry(key).or_insert_with(MinimapChunk::new);
        let tile = &mut chunk.tiles[offset];
        if tile.color != color || tile.friction != new_friction {
            tile.color = color;
            tile.friction = new_friction;
            chunk.dirty = true;
            chunk.gpu_dirty = true;
        }
    }

    pub fn get_tile(&self, pos: &Position) -> Option<MinimapTile> {
        let (key, offset) = Self::key_and_offset(pos);
        let chunk = self.chunks.get(&key)?;
        Some(chunk.tiles[offset])
    }

    pub fn drain_gpu_dirty(&mut self, z: u8) -> Vec<(ChunkKey, Vec<MinimapTile>)> {
        let keys: Vec<ChunkKey> = self
            .chunks
            .iter()
            .filter(|(k, c)| k.z == z && c.gpu_dirty)
            .map(|(k, _)| *k)
            .collect();

        let mut result = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(chunk) = self.chunks.get_mut(&key) {
                chunk.gpu_dirty = false;
                result.push((key, chunk.tiles.clone()));
            }
        }
        result
    }

    pub fn mark_floor_gpu_dirty(&mut self, z: u8) {
        for (key, chunk) in self.chunks.iter_mut() {
            if key.z == z {
                chunk.gpu_dirty = true;
            }
        }
    }

    fn drain_dirty(&mut self) -> Vec<(PathBuf, Vec<u8>)> {
        let dirty_keys: Vec<ChunkKey> = self
            .chunks
            .iter()
            .filter(|(_, c)| c.dirty)
            .map(|(k, _)| *k)
            .collect();

        let mut result = Vec::with_capacity(dirty_keys.len());
        for key in dirty_keys {
            if let Some(chunk) = self.chunks.get_mut(&key) {
                chunk.dirty = false;
                result.push((chunk_path(&key), serialize_chunk(&chunk.tiles)));
            }
        }
        result
    }
}

fn chunk_path(key: &ChunkKey) -> PathBuf {
    crate::conf::paths::data_dir()
        .join("minimap")
        .join(key.z.to_string())
        .join(format!("{}_{}.bin", key.cx, key.cy))
}

// Interleaved layout: [color_0, friction_0, color_1, friction_1, ...]
// friction byte: 0xFF = None (unvisited), other = Some(value)
fn serialize_chunk(tiles: &[MinimapTile]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(tiles.len() * 2);
    for tile in tiles {
        bytes.push(tile.color);
        bytes.push(tile.friction.unwrap_or(0xFF));
    }
    bytes
}

fn deserialize_chunk(bytes: &[u8]) -> Vec<MinimapTile> {
    let mut tiles = vec![MinimapTile::default(); TILES_PER_CHUNK];
    let count = (bytes.len() / 2).min(TILES_PER_CHUNK);
    for i in 0..count {
        let friction_byte = bytes[i * 2 + 1];
        tiles[i] = MinimapTile {
            color: bytes[i * 2],
            friction: if friction_byte == 0xFF {
                None
            } else {
                Some(friction_byte)
            },
        };
    }
    tiles
}

fn parse_chunk_name(stem: &str) -> Option<(u16, u16)> {
    let mut parts = stem.splitn(2, '_');
    let cx = parts.next()?.parse::<u16>().ok()?;
    let cy = parts.next()?.parse::<u16>().ok()?;
    Some((cx, cy))
}

#[derive(Resource)]
pub(super) struct SaveTimer(Timer);

impl Default for SaveTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(5.0, TimerMode::Repeating))
    }
}

pub(super) fn load_from_disk(mut minimap: ResMut<MinimapData>) {
    let base = crate::conf::paths::data_dir().join("minimap");
    let Ok(floor_entries) = std::fs::read_dir(&base) else {
        // No minimap data yet (first run), nothing to load.
        return;
    };

    for floor_entry in floor_entries.flatten() {
        let file_name = floor_entry.file_name();
        let Ok(z) = file_name.to_string_lossy().parse::<u8>() else {
            continue;
        };

        let Ok(chunk_entries) = std::fs::read_dir(floor_entry.path()) else {
            continue;
        };

        for chunk_entry in chunk_entries.flatten() {
            let chunk_file = chunk_entry.file_name();
            let chunk_name = chunk_file.to_string_lossy();
            if !chunk_name.ends_with(".bin") {
                continue;
            }
            let stem = chunk_name.trim_end_matches(".bin");
            let Some((cx, cy)) = parse_chunk_name(stem) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(chunk_entry.path()) else {
                continue;
            };

            minimap.chunks.insert(
                ChunkKey { cx, cy, z },
                MinimapChunk {
                    tiles: deserialize_chunk(&bytes),
                    dirty: false,
                    gpu_dirty: true,
                },
            );
        }
    }
}

pub(super) fn save_dirty_chunks(
    mut minimap: ResMut<MinimapData>,
    mut timer: ResMut<SaveTimer>,
    time: Res<Time>,
) {
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }
    flush_dirty_chunks(&mut minimap);
}

/// Writes every dirty chunk out now, off the timer. Called on the timer tick and
/// again during session cleanup, where the timer will never fire again.
pub(super) fn flush_dirty_chunks(minimap: &mut MinimapData) {
    let dirty = minimap.drain_dirty();
    if dirty.is_empty() {
        return;
    }

    IoTaskPool::get()
        .spawn(async move {
            for (path, bytes) in dirty {
                if let Some(parent) = path.parent()
                    && let Err(e) = std::fs::create_dir_all(parent)
                {
                    warn!("minimap: failed to create dir {parent:?}: {e}");
                    continue;
                }
                if let Err(e) = std::fs::write(&path, &bytes) {
                    warn!("minimap: failed to write {path:?}: {e}");
                }
            }
        })
        .detach();
}
