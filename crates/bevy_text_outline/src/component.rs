use bevy::prelude::*;

/// Outlines a UI [`Text`] or a [`Text2d`] with four copies of its glyphs.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_text_outline::TextOutline;
/// fn spawn_outlined_label(mut commands: Commands) {
///     commands.spawn((Text2d::new("Hello!"), TextOutline::default()));
/// }
/// ```
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component, Default, Debug, Clone)]
pub struct TextOutline {
    /// Outline width in logical pixels, drawn as a whole number of physical pixels.
    pub width: f32,
    /// Multiplied by the fill's alpha, so a fading text fades its outline.
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

impl TextOutline {
    /// The four copy offsets in physical pixels, or none when the width rounds to zero.
    pub fn offsets(&self, scale_factor: f32) -> Option<[Vec2; 4]> {
        let r = (self.width * scale_factor).round();
        (r > 0.0).then(|| [Vec2::X * r, Vec2::NEG_X * r, Vec2::Y * r, Vec2::NEG_Y * r])
    }

    pub fn color_over(&self, fill: Option<&TextColor>) -> LinearRgba {
        let fill_alpha = fill.map_or(1.0, |fill| fill.0.alpha());
        self.color
            .with_alpha(self.color.alpha() * fill_alpha)
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_one_pixel_outline_is_one_physical_pixel_on_each_cardinal() {
        let outline = TextOutline::default();
        assert_eq!(
            outline.offsets(1.0),
            Some([Vec2::X, Vec2::NEG_X, Vec2::Y, Vec2::NEG_Y])
        );
        assert_eq!(outline.offsets(1.25), outline.offsets(1.0));
        assert_eq!(outline.offsets(2.0).unwrap()[0], Vec2::new(2.0, 0.0));
    }

    #[test]
    fn a_zero_width_outline_draws_nothing() {
        let outline = TextOutline {
            width: 0.0,
            ..default()
        };
        assert_eq!(outline.offsets(1.5), None);
    }

    #[test]
    fn the_outline_fades_with_its_fill() {
        let outline = TextOutline::default();
        let fading = TextColor(Color::WHITE.with_alpha(0.25));
        assert_eq!(outline.color_over(Some(&fading)).alpha, 0.25);
        assert_eq!(outline.color_over(None).alpha, 1.0);
    }
}
