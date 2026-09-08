use bevy::prelude::*;

/// Adds a stroke outline to a [`Text2d`] entity using the swash rasterizer.
///
/// The outline is rendered by drawing a stroke of `width * 2` pixels behind the text fill.
/// Since the stroke is centered on the glyph path, the inner half is hidden by the fill,
/// leaving only the outer `width` pixels visible as the outline.
///
/// # Example
/// ```
/// # use bevy::prelude::*;
/// # use bevy_text_outline::TextOutline;
/// fn spawn_outlined_label(mut commands: Commands) {
///     commands.spawn((
///         Text2d::new("Hello!"),
///         TextOutline { width: 2.0, color: Color::BLACK },
///     ));
/// }
/// ```
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
pub struct TextOutline {
    /// Outline width in logical pixels.
    pub width: f32,
    /// Outline color.
    pub color: Color,
}

impl Default for TextOutline {
    fn default() -> Self {
        Self {
            width: 1.0,
            color: Color::BLACK,
        }
    }
}
