#![allow(clippy::type_complexity)]
pub mod component;
pub mod extract;

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, RenderApp};
use bevy::sprite_render::{SpriteSystems, extract_text2d_sprite};
use bevy::ui_render::{RenderUiSystems, extract_text_sections};

pub use component::TextOutline;
use extract::{extract_text2d_outlines, extract_ui_text_outlines};

/// Outlines UI [`Text`] and [`Text2d`] entities that carry a [`TextOutline`].
///
/// The outline is four copies of the text's glyphs in the outline colour, offset on the
/// cardinals by the outline width in physical pixels and drawn behind the fill. They are
/// made at extraction, so there are no extra entities to keep in sync with the text.
pub struct TextOutlinePlugin;

impl Plugin for TextOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TextOutline>();

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                ExtractSchedule,
                (
                    extract_ui_text_outlines
                        .in_set(RenderUiSystems::ExtractText)
                        .before(extract_text_sections),
                    extract_text2d_outlines
                        .after(SpriteSystems::ExtractSprites)
                        .before(extract_text2d_sprite),
                ),
            );
        }
    }
}
