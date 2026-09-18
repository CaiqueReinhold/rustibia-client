//! The native-resolution layer over the game view: `HudCamera` draws everything on
//! `HUD_RENDER_LAYER` into `HudRenderTexture`, which the UI shows over the game image.

use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::Extent3d;

use crate::camera::{HUD_RENDER_LAYER, HudCamera, HudRenderTexture};
use crate::conf::viewport::GAME_VIEW_WIDTH;
use crate::core::GameState;
use crate::game_ui::{GameViewport, scaling::logical_size};
use crate::map::DRAW_KEY_MAX;

/// Above every agent HUD, which sits at its agent's draw key.
pub const OVERLAY_TEXT_Z: f32 = DRAW_KEY_MAX + 16.0;

/// A root scaled so that one local unit is one logical pixel on screen.
#[derive(Component, Debug, Default)]
pub struct HudScaled;

/// The root, under `HudCamera`, of text placed relative to the view rather than the world.
#[derive(Component, Debug, Default)]
pub struct ViewportTextRoot;

pub fn on_overlay_layer() -> (RenderLayers, Pickable) {
    (RenderLayers::layer(HUD_RENDER_LAYER), Pickable::IGNORE)
}

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (scale_hud_roots, fit_hud_target_to_viewport)
                .after(bevy::ui::UiSystems::Layout)
                .before(bevy::transform::TransformSystems::Propagate)
                .run_if(in_state(GameState::InGame)),
        );
    }
}

/// World units per logical pixel of the game viewport.
pub fn world_hud_scale(viewport: &ComputedNode) -> Option<f32> {
    let width = logical_size(viewport).x;
    (width > 0.0).then(|| GAME_VIEW_WIDTH / width)
}

pub fn scale_hud_roots(
    viewport_q: Query<&ComputedNode, With<GameViewport>>,
    mut roots_q: Query<&mut Transform, With<HudScaled>>,
) {
    let Some(scale) = viewport_q.single().ok().and_then(world_hud_scale) else {
        return;
    };
    let scale = Vec3::new(scale, scale, 1.0);
    for mut transform in &mut roots_q {
        if transform.scale != scale {
            transform.scale = scale;
        }
    }
}

pub fn hud_target_size(viewport: &ComputedNode) -> Option<UVec2> {
    let size = viewport.size().round().as_uvec2();
    (size.x > 0 && size.y > 0).then_some(size)
}

pub fn fit_hud_target_to_viewport(
    viewport_q: Query<&ComputedNode, With<GameViewport>>,
    hud_texture: Res<HudRenderTexture>,
    mut images: ResMut<Assets<Image>>,
    mut camera_q: Query<(&mut RenderTarget, &mut Transform), With<HudCamera>>,
) {
    let Ok(viewport) = viewport_q.single() else {
        return;
    };
    let Some(size) = hud_target_size(viewport) else {
        return;
    };
    let Ok((mut target, mut transform)) = camera_q.single_mut() else {
        return;
    };
    let scale_factor = 1.0 / viewport.inverse_scale_factor();
    if matches!(&*target, RenderTarget::Image(image) if image.scale_factor != scale_factor)
        && let RenderTarget::Image(image) = &mut *target
    {
        image.scale_factor = scale_factor;
    }
    // HUD edges sit on a half-pixel lattice, where nearest sampling rounds by
    // float noise; a quarter pixel off it, every edge rounds the same way.
    let quarter_pixel = GAME_VIEW_WIDTH / size.x as f32 / 4.0;
    let bias = Vec3::new(quarter_pixel, quarter_pixel, 0.0);
    if transform.translation != bias {
        transform.translation = bias;
    }
    let extent = Extent3d {
        width: size.x,
        height: size.y,
        depth_or_array_layers: 1,
    };
    if images
        .get(&hud_texture.0)
        .is_some_and(|image| image.texture_descriptor.size != extent)
        && let Some(image) = images.get_mut(&hud_texture.0)
    {
        image.resize(extent);
    }
}

#[cfg(test)]
pub(crate) fn a_viewport(logical: Vec2, scale_factor: f32) -> ComputedNode {
    ComputedNode {
        size: logical * scale_factor,
        inverse_scale_factor: 1.0 / scale_factor,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use bevy::render::render_resource::TextureFormat;

    #[test]
    fn a_hud_keeps_its_logical_size_whatever_the_viewport() {
        for (width, scale_factor) in [(480.0, 1.0), (960.0, 1.0), (700.0, 1.25), (1100.0, 2.0)] {
            let viewport = a_viewport(Vec2::new(width, width * 11.0 / 15.0), scale_factor);
            let scale = world_hud_scale(&viewport).unwrap();
            let world_units_per_logical_px = GAME_VIEW_WIDTH / width;
            assert_eq!(
                scale, world_units_per_logical_px,
                "{width} at {scale_factor}x"
            );
        }
    }

    #[test]
    fn an_unlaid_viewport_has_no_scale() {
        assert_eq!(world_hud_scale(&ComputedNode::default()), None);
    }

    #[test]
    fn every_scaled_root_takes_the_viewport_scale() {
        let mut world = World::new();
        world.spawn((GameViewport, a_viewport(Vec2::new(960.0, 704.0), 1.5)));
        let root = world.spawn((HudScaled, Transform::default())).id();

        world.run_system_once(scale_hud_roots).unwrap();

        assert_eq!(
            world.get::<Transform>(root).unwrap().scale,
            Vec3::new(0.5, 0.5, 1.0)
        );
    }

    #[test]
    fn the_hud_target_matches_the_viewport_pixel_for_pixel() {
        let viewport = a_viewport(Vec2::new(700.0, 513.0), 1.25);
        assert_eq!(hud_target_size(&viewport), Some(UVec2::new(875, 641)));
        assert_eq!(hud_target_size(&ComputedNode::default()), None);
    }

    #[test]
    fn the_hud_target_follows_the_viewport() {
        let mut world = World::new();
        world.init_resource::<Assets<Image>>();
        let handle = world
            .resource_mut::<Assets<Image>>()
            .add(Image::new_target_texture(
                1,
                1,
                TextureFormat::Rgba8Unorm,
                Some(TextureFormat::Rgba8UnormSrgb),
            ));
        world.insert_resource(HudRenderTexture(handle.clone()));
        let camera = world
            .spawn((
                HudCamera,
                RenderTarget::Image(handle.clone().into()),
                Transform::default(),
            ))
            .id();
        world.spawn((GameViewport, a_viewport(Vec2::new(960.0, 704.0), 1.5)));

        world.run_system_once(fit_hud_target_to_viewport).unwrap();

        let size = world
            .resource::<Assets<Image>>()
            .get(&handle)
            .unwrap()
            .size();
        assert_eq!(size, UVec2::new(1440, 1056));
        let RenderTarget::Image(target) = world.get::<RenderTarget>(camera).unwrap() else {
            panic!("the HUD camera renders to an image");
        };
        assert_eq!(target.scale_factor, 1.5);

        let quarter_pixel = GAME_VIEW_WIDTH / 1440.0 / 4.0;
        let bias = world.get::<Transform>(camera).unwrap().translation;
        assert!(
            (bias.truncate() - Vec2::splat(quarter_pixel))
                .abs()
                .max_element()
                < 1e-6,
            "the HUD camera sits a quarter pixel off the grid, got {bias}"
        );
    }
}
