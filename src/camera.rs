use bevy::camera::visibility::RenderLayers;
use bevy::camera::{Camera, ClearColorConfig, OrthographicProjection, RenderTarget, ScalingMode};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

use crate::conf::viewport::{GAME_VIEW_HEIGHT, GAME_VIEW_WIDTH};
use crate::map::DRAW_KEY_MAX;

/// Integer upscale factor for the offscreen render texture.
/// Sprites are rendered nearest-neighbor at this scale inside the texture.
/// The texture is then downscaled to the screen with linear filtering,
/// which eliminates shimmer at any display size while keeping sprites crisp.
const RENDER_UPSCALE: u32 = 2;

#[derive(Component)]
pub struct GameCamera;

/// Handle to the offscreen render texture the game camera draws into.
/// Display it via an ImageNode to get a pixel-perfect, jitter-free result
/// at any window size.
#[derive(Resource)]
pub struct GameRenderTexture(pub Handle<Image>);

pub fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Tonemapping::None,
        RenderLayers::layer(1),
        Msaa::Off,
        IsDefaultUiCamera,
    ));
}

pub fn spawn_game_camera(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // Render to an integer-upscaled offscreen texture.
    // - Sprites are nearest-neighbour sampled *within* the texture (ImagePlugin::default_nearest),
    //   so each game pixel maps to RENDER_UPSCALE×RENDER_UPSCALE texture pixels exactly.
    // - The texture is displayed with linear filtering (sampler below), so the downscale
    //   to any screen size is smooth — no nearest-neighbour shimmer at fractional scales.
    let mut image = Image::new_target_texture(
        GAME_VIEW_WIDTH as u32 * RENDER_UPSCALE,
        GAME_VIEW_HEIGHT as u32 * RENDER_UPSCALE,
        TextureFormat::Rgba8Unorm,
        Some(TextureFormat::Rgba8UnormSrgb),
    );
    image.sampler = ImageSampler::linear();
    let image_handle = images.add(image);
    commands.insert_resource(GameRenderTexture(image_handle.clone()));

    let mut projection = OrthographicProjection::default_2d();
    projection.scaling_mode = ScalingMode::Fixed {
        width: GAME_VIEW_WIDTH,
        height: GAME_VIEW_HEIGHT,
    };
    // z is a draw-order key here, not a depth (see `map::draw_order`), and keys
    // run far past the default 1000-unit planes. Anything outside them is
    // clipped with no error.
    projection.near = -DRAW_KEY_MAX;
    projection.far = DRAW_KEY_MAX;
    commands.spawn((
        Camera2d,
        Camera {
            order: 0,
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Tonemapping::None,
        Projection::Orthographic(projection),
        GameCamera,
        Transform::default(),
        GlobalTransform::default(),
        Msaa::Off,
    ));
}
