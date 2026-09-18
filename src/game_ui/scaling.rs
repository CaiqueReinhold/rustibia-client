//! Conversions between the UI's two pixel spaces.
//!
//! Bevy computes UI layout in **physical** pixels: `ComputedNode::size` and
//! `UiGlobalTransform::translation` are physical. Almost everything else the game
//! touches is **logical**: `Window::cursor_position`, `PointerLocation::position`,
//! and every `Val::Px` — which covers `Node` sizes and `UiTransform` translations.
//!
//! At a display scale factor of 1.0 the two spaces are numerically identical, so
//! mixing them is invisible on a 100% display and silently wrong everywhere else:
//! a Windows box at 125% or 150% offsets anything placed over the map and lands map
//! clicks on the wrong tile. Convert with these helpers instead of comparing a
//! `ComputedNode` against a cursor position directly.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

/// Converts a physical-pixel vector — a `UiGlobalTransform` translation, or a
/// delta between two of them — into logical pixels.
pub fn to_logical(node: &ComputedNode, physical: Vec2) -> Vec2 {
    physical * node.inverse_scale_factor()
}

/// The node's size in logical pixels.
pub fn logical_size(node: &ComputedNode) -> Vec2 {
    to_logical(node, node.size())
}

/// The node's rect in logical pixels, with the origin at the window's top-left —
/// the same space a cursor or pointer position arrives in.
pub fn logical_rect(node: &ComputedNode, transform: &UiGlobalTransform) -> Rect {
    Rect::from_center_size(logical_center(node, transform), logical_size(node))
}

/// The node's centre in logical pixels.
pub fn logical_center(node: &ComputedNode, transform: &UiGlobalTransform) -> Vec2 {
    to_logical(node, transform.translation)
}

#[cfg(test)]
mod tests {
    use bevy::math::Affine2;

    use super::*;

    /// Builds the node a viewport of `logical` size at `center` would produce on a
    /// display with this scale factor — layout multiplies both by the factor.
    fn node_at(
        center: Vec2,
        logical: Vec2,
        scale_factor: f32,
    ) -> (ComputedNode, UiGlobalTransform) {
        let node = ComputedNode {
            size: logical * scale_factor,
            inverse_scale_factor: 1.0 / scale_factor,
            ..Default::default()
        };
        let transform = UiGlobalTransform::from(Affine2::from_translation(center * scale_factor));
        (node, transform)
    }

    /// The invariant the UI code depends on: the same on-screen layout resolves to
    /// the same logical rect no matter what the display scale factor is. Before
    /// this conversion existed, a 150% display reported a rect 1.5× too large and
    /// offset, which is what threw off HUD placement and map picking on Windows.
    #[test]
    fn the_logical_rect_is_the_same_at_every_scale_factor() {
        let center = Vec2::new(400.0, 300.0);
        let size = Vec2::new(480.0, 352.0);

        for scale_factor in [1.0, 1.25, 1.5, 2.0] {
            let (node, transform) = node_at(center, size, scale_factor);
            let rect = logical_rect(&node, &transform);

            assert_eq!(rect.size(), size, "size at {scale_factor}×");
            assert_eq!(rect.center(), center, "centre at {scale_factor}×");
            assert_eq!(logical_size(&node), size, "logical_size at {scale_factor}×");
            assert_eq!(
                logical_center(&node, &transform),
                center,
                "centre at {scale_factor}×"
            );
        }
    }

    /// How the hover systems use the rect: a cursor at a known fraction of
    /// the viewport must produce the same UV on every display.
    #[test]
    fn a_cursor_maps_to_the_same_uv_at_every_scale_factor() {
        let center = Vec2::new(400.0, 300.0);
        let size = Vec2::new(480.0, 352.0);
        // Three quarters across, one quarter down, in logical window coordinates.
        let cursor = center - size * 0.5 + size * Vec2::new(0.75, 0.25);

        for scale_factor in [1.0, 1.25, 1.5, 2.0] {
            let (node, transform) = node_at(center, size, scale_factor);
            let rect = logical_rect(&node, &transform);

            assert!(rect.contains(cursor), "cursor inside at {scale_factor}×");
            let uv = (cursor - rect.min) / rect.size();
            assert!(
                uv.abs_diff_eq(Vec2::new(0.75, 0.25), 1e-5),
                "uv {uv} at {scale_factor}×"
            );
        }
    }
}
