use bevy::prelude::*;

use std::fmt::Display;
use std::ops::{Add, Sub};

use crate::agent::WalkingDirection;
use crate::conf::map::TILE_SIZE;

#[derive(Component, Hash, PartialEq, Eq, Clone, Debug)]
pub struct Position {
    pub x: u16,
    pub y: u16,
    pub z: u8,
}

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tile({}, {}, {})", self.x, self.y, self.z)
    }
}

impl Position {
    pub fn new(x: u16, y: u16, z: u8) -> Self {
        Position { x, y, z }
    }

    pub fn from_world(world_pos: Vec2, z: u8) -> Self {
        let floor_offset = ((7 - z as i32) * 32) as f32;
        Position {
            x: ((world_pos.x + floor_offset) / TILE_SIZE).floor() as u16,
            y: ((floor_offset - world_pos.y) / TILE_SIZE).floor() as u16,
            z,
        }
    }

    /// The tile's top-left corner in world space. **`z` is always zero**: draw
    /// order is not a function of position, it is a `DrawOrder` component that
    /// `map::draw_order::apply_draw_order` turns into a z. A `Vec3` is still
    /// returned because every caller feeds it straight to a `Transform`.
    pub fn to_world(&self) -> Vec3 {
        let floor_offset = ((7 - self.z as i32) * 32) as f32;
        Vec3::new(
            ((self.x as f32) * TILE_SIZE) - floor_offset,
            (-(self.y as f32) * TILE_SIZE) + floor_offset,
            0.0,
        )
    }

    pub fn to_world_with_elevation(&self, elevation: u8) -> Vec3 {
        self.to_world() + Vec3::new(-(elevation as f32), elevation as f32, 0.0)
    }

    pub fn delta(&self, x: i32, y: i32) -> Self {
        Position {
            x: ((self.x as i32) + x) as u16,
            y: ((self.y as i32) + y) as u16,
            z: self.z,
        }
    }
}

impl Add<WalkingDirection> for Position {
    type Output = Position;

    fn add(self, rhs: WalkingDirection) -> Self::Output {
        match rhs {
            WalkingDirection::North => self.delta(0, -1),
            WalkingDirection::East => self.delta(1, 0),
            WalkingDirection::South => self.delta(0, 1),
            WalkingDirection::West => self.delta(-1, 0),
            WalkingDirection::NorthEast => self.delta(1, -1),
            WalkingDirection::SouthEast => self.delta(1, 1),
            WalkingDirection::NorthWest => self.delta(-1, -1),
            WalkingDirection::SouthWest => self.delta(-1, 1),
        }
    }
}

impl Sub<WalkingDirection> for Position {
    type Output = Position;

    fn sub(self, rhs: WalkingDirection) -> Self::Output {
        match rhs {
            WalkingDirection::North => self.delta(0, 1),
            WalkingDirection::East => self.delta(-1, 0),
            WalkingDirection::South => self.delta(0, -1),
            WalkingDirection::West => self.delta(1, 0),
            WalkingDirection::NorthEast => self.delta(-1, 1),
            WalkingDirection::SouthEast => self.delta(-1, -1),
            WalkingDirection::NorthWest => self.delta(1, 1),
            WalkingDirection::SouthWest => self.delta(1, -1),
        }
    }
}
