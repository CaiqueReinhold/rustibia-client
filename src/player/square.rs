use bevy::prelude::*;

use crate::conf::map::TILE_SIZE;
use crate::conf::target::SQUARE_THICKNESS;
use crate::map::TARGET_SQUARE_LOCAL_Z;

/// `to_world()` is the tile's top-left corner; the square must sit on the tile
/// centre. Y is negative because world Y grows upward while tile Y grows down.
pub fn square_centre_offset() -> Vec2 {
    Vec2::new(TILE_SIZE / 2.0, -TILE_SIZE / 2.0)
}

/// The four edges of a `size`-wide outline as `(bar_size, centre_offset_from_tile_centre)`.
pub fn square_bars(size: f32) -> [(Vec2, Vec2); 4] {
    let half = size / 2.0;
    let inset = half - SQUARE_THICKNESS / 2.0;
    [
        (Vec2::new(size, SQUARE_THICKNESS), Vec2::new(0.0, inset)),
        (Vec2::new(size, SQUARE_THICKNESS), Vec2::new(0.0, -inset)),
        (Vec2::new(SQUARE_THICKNESS, size), Vec2::new(-inset, 0.0)),
        (Vec2::new(SQUARE_THICKNESS, size), Vec2::new(inset, 0.0)),
    ]
}

/// Spawns an outline as a **child of `agent`**, which gives walk-offset tracking
/// for free and despawns the square with the agent.
pub fn spawn_square(
    commands: &mut Commands,
    agent: Entity,
    marker: impl Bundle,
    color: Color,
    size: f32,
) {
    let centre = square_centre_offset();
    commands.entity(agent).with_children(|parent| {
        let mut root = parent.spawn((
            marker,
            Transform::from_xyz(centre.x, centre.y, TARGET_SQUARE_LOCAL_Z),
            Visibility::default(),
        ));
        root.with_children(|frame| {
            for (bar, offset) in square_bars(size) {
                frame.spawn((
                    Sprite {
                        color,
                        custom_size: Some(bar),
                        ..Default::default()
                    },
                    Transform::from_xyz(offset.x, offset.y, 0.0),
                ));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bar_spans_one_full_edge_at_the_configured_thickness() {
        for (bar, _) in square_bars(TILE_SIZE) {
            assert!(
                (bar.x - TILE_SIZE).abs() < f32::EPSILON
                    || (bar.y - TILE_SIZE).abs() < f32::EPSILON,
                "every bar spans one full edge, got {bar:?}"
            );
            assert!(
                (bar.x - SQUARE_THICKNESS).abs() < f32::EPSILON
                    || (bar.y - SQUARE_THICKNESS).abs() < f32::EPSILON,
                "every bar is one thickness deep, got {bar:?}"
            );
        }
    }

    /// `to_world()` returns the tile's TOP-LEFT corner and the convention is
    /// size-dependent: 64px sprites centre on it, 32px things do not. This is the
    /// counterpart of OTClient's `- getDisplacement()`; both put the square on the
    /// tile rather than on the artwork.
    #[test]
    fn the_square_is_offset_to_the_tile_centre() {
        assert_eq!(
            square_centre_offset(),
            Vec2::new(TILE_SIZE / 2.0, -TILE_SIZE / 2.0)
        );
    }
}
