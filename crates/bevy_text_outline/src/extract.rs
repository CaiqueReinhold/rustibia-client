use bevy::math::Affine2;
use bevy::prelude::*;
use bevy::render::Extract;
use bevy::render::sync_world::TemporaryRenderEntity;
use bevy::sprite::Anchor;
use bevy::sprite_render::{
    ExtractedSlice, ExtractedSlices, ExtractedSprite, ExtractedSpriteKind, ExtractedSprites,
};
use bevy::text::{PositionedGlyph, TextBounds, TextLayoutInfo};
use bevy::ui::{CalculatedClip, ComputedNode, ComputedUiTargetCamera};
use bevy::ui_render::{
    ExtractedGlyph, ExtractedUiItem, ExtractedUiNode, ExtractedUiNodes, UiCameraMap,
    stack_z_offsets,
};

use crate::component::TextOutline;

fn glyph_rect(atlases: &Assets<TextureAtlasLayout>, glyph: &PositionedGlyph) -> Rect {
    atlases
        .get(glyph.atlas_info.texture_atlas)
        .unwrap()
        .textures[glyph.atlas_info.location.glyph_index]
        .as_rect()
}

fn ends_batch(glyphs: &[PositionedGlyph], i: usize) -> bool {
    glyphs
        .get(i + 1)
        .is_none_or(|next| next.atlas_info.texture != glyphs[i].atlas_info.texture)
}

/// Runs before `extract_text_sections` at the same z, so the copies are pushed, and
/// drawn, before the fill.
pub fn extract_ui_text_outlines(
    mut commands: Commands,
    mut extracted_uinodes: ResMut<ExtractedUiNodes>,
    texture_atlases: Extract<Res<Assets<TextureAtlasLayout>>>,
    camera_map: Extract<UiCameraMap>,
    query: Extract<
        Query<(
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &InheritedVisibility,
            Option<&CalculatedClip>,
            &ComputedUiTargetCamera,
            &TextLayoutInfo,
            &TextOutline,
            Option<&TextColor>,
        )>,
    >,
) {
    let mut camera_mapper = camera_map.get_mapper();
    for (entity, uinode, transform, visibility, clip, target, layout, outline, fill) in &query {
        if !visibility.get() || uinode.is_empty() {
            continue;
        }
        let Some(extracted_camera_entity) = camera_mapper.map(target) else {
            continue;
        };
        let Some(offsets) = outline.offsets(layout.scale_factor) else {
            continue;
        };
        let color = outline.color_over(fill);

        for offset in offsets {
            let node_transform = Affine2::from(*transform)
                * Affine2::from_translation(-0.5 * uinode.size() + offset);
            let mut start = extracted_uinodes.glyphs.len();
            for (i, glyph) in layout.glyphs.iter().enumerate() {
                extracted_uinodes.glyphs.push(ExtractedGlyph {
                    color,
                    translation: glyph.position,
                    rect: glyph_rect(&texture_atlases, glyph),
                });
                if ends_batch(&layout.glyphs, i) {
                    let end = extracted_uinodes.glyphs.len();
                    extracted_uinodes.uinodes.push(ExtractedUiNode {
                        transform: node_transform,
                        z_order: uinode.stack_index as f32 + stack_z_offsets::TEXT,
                        render_entity: commands.spawn(TemporaryRenderEntity).id(),
                        image: glyph.atlas_info.texture,
                        clip: clip.map(|clip| clip.clip),
                        extracted_camera_entity,
                        item: ExtractedUiItem::Glyphs { range: start..end },
                        main_entity: entity.into(),
                    });
                    start = end;
                }
            }
        }
    }
}

/// Runs before `extract_text2d_sprite` at the same z, so the copies are pushed, and
/// drawn, before the fill.
pub fn extract_text2d_outlines(
    mut commands: Commands,
    mut extracted_sprites: ResMut<ExtractedSprites>,
    mut extracted_slices: ResMut<ExtractedSlices>,
    texture_atlases: Extract<Res<Assets<TextureAtlasLayout>>>,
    query: Extract<
        Query<
            (
                Entity,
                &ViewVisibility,
                &TextLayoutInfo,
                &TextBounds,
                &Anchor,
                &GlobalTransform,
                &TextOutline,
                Option<&TextColor>,
            ),
            With<Text2d>,
        >,
    >,
) {
    for (entity, visibility, layout, bounds, anchor, transform, outline, fill) in &query {
        if !visibility.get() {
            continue;
        }
        let Some(offsets) = outline.offsets(layout.scale_factor) else {
            continue;
        };
        let color = outline.color_over(fill);
        let size = Vec2::new(
            bounds.width.unwrap_or(layout.size.x),
            bounds.height.unwrap_or(layout.size.y),
        );
        let top_left = (Anchor::TOP_LEFT.0 - anchor.as_vec()) * size;
        let scaling =
            GlobalTransform::from_scale(Vec2::splat(layout.scale_factor.recip()).extend(1.));

        for offset in offsets {
            let copy_transform = *transform
                * GlobalTransform::from_translation(top_left.extend(0.))
                * scaling
                * GlobalTransform::from_translation(offset.extend(0.));
            let mut start = extracted_slices.slices.len();
            for (i, glyph) in layout.glyphs.iter().enumerate() {
                let rect = glyph_rect(&texture_atlases, glyph);
                extracted_slices.slices.push(ExtractedSlice {
                    offset: Vec2::new(glyph.position.x, -glyph.position.y),
                    rect,
                    size: rect.size(),
                });
                if ends_batch(&layout.glyphs, i) {
                    let end = extracted_slices.slices.len();
                    extracted_sprites.sprites.push(ExtractedSprite {
                        main_entity: entity,
                        render_entity: commands.spawn(TemporaryRenderEntity).id(),
                        transform: copy_transform,
                        color,
                        image_handle_id: glyph.atlas_info.texture,
                        flip_x: false,
                        flip_y: false,
                        kind: ExtractedSpriteKind::Slices {
                            indices: start..end,
                        },
                    });
                    start = end;
                }
            }
        }
    }
}
