use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::{
    conf::ui::{
        SIDE_PANEL_WIDTH,
        ui_colors::{self, DARK_BORDER_COLOR, LIGHT_BORDER_COLOR},
        z_index::Z_WINDOW,
    },
    core::GameState,
    game_ui::{GameUiAssets, Index, RightPanelDock, UIWindow, UIWindowDock, UiWindowRef, WindowId},
    map::{
        Position,
        minimap::{CHUNK_SIZE, ChunkKey, MinimapData, MinimapTile},
    },
    player::{InteractionIntent, components::Player, is_drag},
};

const IMAGE_SIZE: u16 = 2048;
/// How close to the texture edge the player gets before the window re-centres.
const RECENTRE_MARGIN: u16 = 256;
/// Tiles visible per axis at each zoom level (index 0 = most zoomed in).
const ZOOM_LEVELS: [u8; 4] = [20, 40, 80, 160];
const DEFAULT_ZOOM: usize = 2; // 80×80 tiles

/// A tile no chunk has written yet.
const UNEXPLORED: [u8; 4] = [0, 0, 0, 255];

#[derive(Resource)]
pub struct MinimapImageHandle(pub Handle<Image>);

#[derive(Resource)]
pub struct MinimapZoom(pub usize);

/// The slice of the world the texture holds: `origin` is the world tile drawn at
/// pixel (0, 0), and `painted_z` the floor those pixels came from. The texture is a
/// few thousand tiles across and the map is tens of thousands, so it is a window
/// that follows the player rather than a picture of the world.
#[derive(Resource, Default)]
pub struct MinimapWindow {
    origin: UVec2,
    painted_z: Option<u8>,
}

impl MinimapWindow {
    /// Where the window should be for `player`, or `None` when the one it already
    /// holds will do.
    fn retarget(&self, player: &Position) -> Option<UVec2> {
        let target = centred_origin(player);
        let stale = self.painted_z != Some(player.z)
            || (!self.holds_with_margin(player) && target != self.origin);
        stale.then_some(target)
    }

    fn holds_with_margin(&self, player: &Position) -> bool {
        let inside = |p: u16, o: u32| {
            let p = p as i64 - o as i64;
            p >= RECENTRE_MARGIN as i64 && p < (IMAGE_SIZE - RECENTRE_MARGIN) as i64
        };
        inside(player.x, self.origin.x) && inside(player.y, self.origin.y)
    }
}

fn centred_origin(player: &Position) -> UVec2 {
    let half = IMAGE_SIZE / 2;
    UVec2::new(
        player.x.saturating_sub(half) as u32,
        player.y.saturating_sub(half) as u32,
    )
}

#[derive(Component)]
struct MinimapImageNode;

/// On the image node while a primary press on it has travelled far enough to be a drag.
#[derive(Component)]
struct MinimapDragged;

pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(GameState::InGame),
            setup_minimap
                .after(crate::game_ui::spawn_main_ui)
                .before(crate::items::inventory::spawn_inventory_ui),
        )
        .add_systems(
            Update,
            update_minimap_image.run_if(in_state(GameState::InGame)),
        );
    }
}

fn setup_minimap(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    dock_q: Query<(Entity, &UIWindowDock), With<RightPanelDock>>,
    ui_assets: Res<GameUiAssets>,
) {
    let Ok((dock_entity, dock)) = dock_q.single() else {
        return;
    };

    let mut image = Image::new_fill(
        Extent3d {
            width: IMAGE_SIZE as u32,
            height: IMAGE_SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &UNEXPLORED,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    let handle = images.add(image);

    commands.insert_resource(MinimapImageHandle(handle.clone()));
    commands.insert_resource(MinimapZoom(DEFAULT_ZOOM));
    commands.insert_resource(MinimapWindow::default());

    let window_id = WindowId::new();
    let zoom_tiles = ZOOM_LEVELS[DEFAULT_ZOOM] as f32;
    let window_height = 120.0 + 20.0 + 4.0; // image + button row + border

    let zoom_in_btn = commands
        .spawn((
            Node {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
            },
        ))
        .with_child((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ImageNode::new(ui_assets.background_light.clone()).with_mode(NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            }),
            children![(
                Text::new("+"),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::from(ui_colors::FONT_COLOR_CONTENT)),
            )],
        ))
        .observe(|mut e: On<Pointer<Click>>, mut zoom: ResMut<MinimapZoom>| {
            e.propagate(false);
            if zoom.0 > 0 {
                zoom.0 -= 1;
            }
        })
        .id();

    let zoom_out_btn = commands
        .spawn((
            Node {
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
            },
        ))
        .with_child((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ImageNode::new(ui_assets.background_light.clone()).with_mode(NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            }),
            children![(
                Text::new("−"),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 10.0,
                    ..default()
                },
                TextColor(Color::from(ui_colors::FONT_COLOR_CONTENT)),
            )],
        ))
        .observe(|mut e: On<Pointer<Click>>, mut zoom: ResMut<MinimapZoom>| {
            e.propagate(false);
            if zoom.0 < ZOOM_LEVELS.len() - 1 {
                zoom.0 += 1;
            }
        })
        .id();

    let button_row = commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::End,
            top: Val::Px(7.0),
            right: Val::Px(22.0),
            ..default()
        })
        .add_children(&[zoom_in_btn, zoom_out_btn])
        .id();

    let cross_h = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(7.0),
                height: Val::Px(1.0),
                left: Val::Px(143.0 / 2.0 - 3.0),
                top: Val::Px(143.0 / 2.0),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            Pickable::IGNORE,
        ))
        .id();

    let cross_v = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(1.0),
                height: Val::Px(7.0),
                left: Val::Px(143.0 / 2.0),
                top: Val::Px(143.0 / 2.0 - 3.0),
                ..default()
            },
            BackgroundColor(Color::WHITE),
            Pickable::IGNORE,
        ))
        .id();

    let image_node = commands
        .spawn((
            MinimapImageNode,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                overflow: Overflow::visible(),
                ..default()
            },
            ImageNode {
                image: handle,
                rect: Some(Rect {
                    min: Vec2::ZERO,
                    max: Vec2::new(zoom_tiles, zoom_tiles),
                }),
                ..default()
            },
        ))
        .add_children(&[cross_h, cross_v])
        .observe(on_minimap_click)
        .observe(on_minimap_drag)
        .observe(on_minimap_drag_end)
        .id();

    let image_node_wrapper = commands
        .spawn((
            Node {
                width: Val::Px(143.0),
                height: Val::Px(143.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: DARK_BORDER_COLOR.into(),
                right: LIGHT_BORDER_COLOR.into(),
                bottom: LIGHT_BORDER_COLOR.into(),
                left: DARK_BORDER_COLOR.into(),
            },
        ))
        .add_child(image_node)
        .id();

    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_items: JustifyItems::Center,
            ..default()
        })
        .add_children(&[image_node_wrapper, button_row])
        .id();

    commands.entity(content).insert(UiWindowRef { window_id });

    let window = commands
        .spawn((
            UIWindow {
                id: window_id,
                dock_id: dock.id,
            },
            Index(0),
            Node {
                left: Val::Px(-2.0),
                width: Val::Px(SIDE_PANEL_WIDTH),
                height: Val::Px(window_height),
                min_height: Val::Px(window_height),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::hidden(),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
            },
            ZIndex(Z_WINDOW),
        ))
        .add_child(content)
        .id();

    commands.entity(dock_entity).add_child(window);
}

fn update_minimap_image(
    mut minimap: ResMut<MinimapData>,
    image_handle: Option<Res<MinimapImageHandle>>,
    mut images: ResMut<Assets<Image>>,
    zoom: Option<Res<MinimapZoom>>,
    window: Option<ResMut<MinimapWindow>>,
    player_q: Single<&Position, With<Player>>,
    mut image_node_q: Query<&mut ImageNode, With<MinimapImageNode>>,
) {
    let (Some(handle), Some(zoom), Some(mut window)) = (image_handle, zoom, window) else {
        return;
    };

    let player = player_q.into_inner();

    let moved = window.retarget(player);
    if let Some(origin) = moved {
        window.origin = origin;
        window.painted_z = Some(player.z);
        minimap.mark_floor_gpu_dirty(player.z);
    }

    if let Ok(mut img) = image_node_q.single_mut() {
        img.rect = Some(player_centered_rect(player, window.origin, &zoom));
    }

    let dirty = minimap.drain_gpu_dirty(player.z);
    if dirty.is_empty() && moved.is_none() {
        return;
    }

    let Some(image) = images.get_mut(&handle.0) else {
        return;
    };
    let Some(ref mut data) = image.data else {
        return;
    };

    // Whatever the repaint below does not cover is another floor or another part of
    // the world, drawn at the pixel some tile here now owns.
    if moved.is_some() {
        for pixel in data.as_chunks_mut::<4>().0 {
            *pixel = UNEXPLORED;
        }
    }

    for (key, tiles) in dirty {
        paint_chunk(data, window.origin, key, &tiles);
    }
}

fn paint_chunk(data: &mut [u8], origin: UVec2, key: ChunkKey, tiles: &[MinimapTile]) {
    let chunk = CHUNK_SIZE as i64;
    let size = IMAGE_SIZE as i64;
    let chunk_x = key.cx as i64 * chunk - origin.x as i64;
    let chunk_y = key.cy as i64 * chunk - origin.y as i64;
    if chunk_x <= -chunk || chunk_x >= size || chunk_y <= -chunk || chunk_y >= size {
        return;
    }

    for ly in 0..chunk {
        let py = chunk_y + ly;
        if !(0..size).contains(&py) {
            continue;
        }
        for lx in 0..chunk {
            let px = chunk_x + lx;
            if !(0..size).contains(&px) {
                continue;
            }
            let (r, g, b) = minimap_color_to_rgb(tiles[(ly * chunk + lx) as usize].color);
            let idx = (py * size + px) as usize * 4;
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = 255;
        }
    }
}

fn player_centered_rect(player: &Position, origin: UVec2, zoom: &MinimapZoom) -> Rect {
    let tiles = ZOOM_LEVELS[zoom.0] as f32;
    let half = tiles / 2.0;
    let cx = player.x as f32 - origin.x as f32;
    let cy = player.y as f32 - origin.y as f32;
    let max_origin = (IMAGE_SIZE as f32 - tiles).max(0.0);
    let min_x = (cx - half).clamp(0.0, max_origin);
    let min_y = (cy - half).clamp(0.0, max_origin);
    Rect {
        min: Vec2::new(min_x, min_y),
        max: Vec2::new(min_x + tiles, min_y + tiles),
    }
}

/// The world tile drawn under `hit`, which is normalised to the image node:
/// (-0.5, -0.5) at its top-left, (0.5, 0.5) at its bottom-right.
fn tile_at(hit: Vec2, view: Rect, origin: UVec2, z: u8) -> Position {
    let texel = (view.min + (hit + 0.5) * view.size())
        .floor()
        .clamp(view.min, view.max - 1.0);
    Position {
        x: (origin.x + texel.x as u32) as u16,
        y: (origin.y + texel.y as u32) as u16,
        z,
    }
}

fn on_minimap_click(
    mut click: On<Pointer<Click>>,
    mut commands: Commands,
    window: Res<MinimapWindow>,
    player: Single<&Position, With<Player>>,
    node: Single<(&ImageNode, Has<MinimapDragged>), With<MinimapImageNode>>,
) {
    click.propagate(false);
    let (image, dragged) = *node;
    if click.button != PointerButton::Primary || dragged {
        return;
    }
    let (Some(hit), Some(view)) = (click.hit.position, image.rect) else {
        return;
    };
    commands.trigger(InteractionIntent::WalkTo(tile_at(
        hit.truncate(),
        view,
        window.origin,
        player.z,
    )));
}

fn on_minimap_drag(drag: On<Pointer<Drag>>, mut commands: Commands) {
    if drag.button == PointerButton::Primary && is_drag(drag.distance) {
        commands.entity(drag.entity).insert(MinimapDragged);
    }
}

fn on_minimap_drag_end(drag: On<Pointer<DragEnd>>, mut commands: Commands) {
    commands.entity(drag.entity).remove::<MinimapDragged>();
}

fn minimap_color_to_rgb(color: u8) -> (u8, u8, u8) {
    (color / 36 % 6 * 51, color / 6 % 6 * 51, color % 6 * 51)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        camera::NormalizedRenderTarget,
        ecs::system::RunSystemOnce,
        picking::{
            backend::HitData,
            pointer::{Location, PointerId},
        },
    };
    use std::time::Duration;

    /// Where `map1.otbm` puts a player: the real Tibia coordinate space, tens of
    /// thousands of tiles from the origin the texture used to be pinned to.
    const SPAWN: Position = Position {
        x: 32100,
        y: 31100,
        z: 7,
    };

    fn window_at(player: &Position) -> MinimapWindow {
        MinimapWindow {
            origin: centred_origin(player),
            painted_z: Some(player.z),
        }
    }

    fn chunk_of(player: &Position, color: u8) -> (ChunkKey, Vec<MinimapTile>) {
        let mut tiles = vec![MinimapTile::default(); (CHUNK_SIZE * CHUNK_SIZE) as usize];
        let offset = (player.y % CHUNK_SIZE) as usize * CHUNK_SIZE as usize
            + (player.x % CHUNK_SIZE) as usize;
        tiles[offset] = MinimapTile {
            color,
            friction: Some(1),
        };
        (
            ChunkKey {
                cx: player.x / CHUNK_SIZE,
                cy: player.y / CHUNK_SIZE,
                z: player.z,
            },
            tiles,
        )
    }

    fn pixel_at(data: &[u8], x: u16, y: u16) -> [u8; 4] {
        let idx = (y as usize * IMAGE_SIZE as usize + x as usize) * 4;
        data[idx..idx + 4].try_into().unwrap()
    }

    #[test]
    fn a_tile_far_from_the_world_origin_is_painted() {
        let window = window_at(&SPAWN);
        let (key, tiles) = chunk_of(&SPAWN, 200);
        let mut data = UNEXPLORED.repeat((IMAGE_SIZE as usize).pow(2));

        paint_chunk(&mut data, window.origin, key, &tiles);

        let (r, g, b) = minimap_color_to_rgb(200);
        let px = SPAWN.x - window.origin.x as u16;
        let py = SPAWN.y - window.origin.y as u16;
        assert_eq!(pixel_at(&data, px, py), [r, g, b, 255]);
    }

    #[test]
    fn a_chunk_outside_the_window_paints_nothing() {
        let window = window_at(&SPAWN);
        let far = Position {
            x: 100,
            y: 100,
            z: SPAWN.z,
        };
        let (key, tiles) = chunk_of(&far, 200);
        let mut data = UNEXPLORED.repeat((IMAGE_SIZE as usize).pow(2));

        paint_chunk(&mut data, window.origin, key, &tiles);

        assert!(data.as_chunks::<4>().0.iter().all(|p| *p == UNEXPLORED));
    }

    #[test]
    fn the_view_rect_is_centred_on_the_player_within_the_window() {
        let window = window_at(&SPAWN);
        let tiles = ZOOM_LEVELS[DEFAULT_ZOOM] as f32;

        let rect = player_centered_rect(&SPAWN, window.origin, &MinimapZoom(DEFAULT_ZOOM));

        assert_eq!(rect.center(), Vec2::splat(IMAGE_SIZE as f32 / 2.0));
        assert_eq!(rect.max - rect.min, Vec2::splat(tiles));
    }

    #[test]
    fn the_centre_of_the_view_is_the_players_tile() {
        let window = window_at(&SPAWN);
        let view = player_centered_rect(&SPAWN, window.origin, &MinimapZoom(DEFAULT_ZOOM));

        assert_eq!(tile_at(Vec2::ZERO, view, window.origin, SPAWN.z), SPAWN);
    }

    #[test]
    fn the_corners_of_the_view_are_its_first_and_last_tiles() {
        let window = window_at(&SPAWN);
        let view = player_centered_rect(&SPAWN, window.origin, &MinimapZoom(DEFAULT_ZOOM));
        let half = ZOOM_LEVELS[DEFAULT_ZOOM] as u16 / 2;

        assert_eq!(
            tile_at(Vec2::splat(-0.5), view, window.origin, SPAWN.z),
            Position {
                x: SPAWN.x - half,
                y: SPAWN.y - half,
                z: SPAWN.z
            }
        );
        assert_eq!(
            tile_at(Vec2::splat(0.5), view, window.origin, SPAWN.z),
            Position {
                x: SPAWN.x + half - 1,
                y: SPAWN.y + half - 1,
                z: SPAWN.z
            }
        );
    }

    /// At the window's edge the view cannot centre on the player, so the middle of
    /// the image is not where the player is drawn.
    #[test]
    fn a_view_clamped_at_the_window_edge_maps_to_the_tile_drawn() {
        let corner = Position { x: 10, y: 10, z: 7 };
        let window = window_at(&corner);
        let view = player_centered_rect(&corner, window.origin, &MinimapZoom(DEFAULT_ZOOM));
        let tiles = ZOOM_LEVELS[DEFAULT_ZOOM] as f32;
        let over_the_player = Vec2::splat(10.5 / tiles - 0.5);

        assert_eq!(tile_at(over_the_player, view, window.origin, 7), corner);
        assert_eq!(
            tile_at(Vec2::ZERO, view, window.origin, 7),
            Position { x: 40, y: 40, z: 7 }
        );
    }

    #[test]
    fn a_fresh_window_paints_wherever_the_player_is() {
        assert_eq!(
            MinimapWindow::default().retarget(&SPAWN),
            Some(centred_origin(&SPAWN))
        );
    }

    #[test]
    fn a_step_inside_the_window_repaints_nothing() {
        let window = window_at(&SPAWN);
        let stepped = Position {
            x: SPAWN.x + 1,
            ..SPAWN
        };

        assert_eq!(window.retarget(&stepped), None);
    }

    #[test]
    fn walking_towards_the_edge_moves_the_window() {
        let window = window_at(&SPAWN);
        let far = Position {
            x: SPAWN.x + (IMAGE_SIZE / 2 - RECENTRE_MARGIN),
            ..SPAWN
        };

        assert_eq!(window.retarget(&far), Some(centred_origin(&far)));
    }

    /// Near the world origin the window is already as far left as it goes, so the
    /// margin is unreachable and asking for it every frame would repaint the whole
    /// texture every frame.
    #[test]
    fn a_window_pinned_to_the_world_edge_does_not_thrash() {
        let corner = Position { x: 10, y: 10, z: 7 };
        let window = window_at(&corner);

        assert_eq!(window.origin, UVec2::ZERO);
        assert_eq!(window.retarget(&corner), None);
    }

    /// The whole path the black minimap ran through: a tile arrives at a real
    /// `map1.otbm` coordinate, and the system has to place it in the texture and
    /// aim the view rect at it.
    #[test]
    fn the_system_paints_a_tile_at_a_real_map_coordinate() {
        let mut world = World::new();
        let mut images = Assets::<Image>::default();
        let handle = images.add(Image::new_fill(
            Extent3d {
                width: IMAGE_SIZE as u32,
                height: IMAGE_SIZE as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &UNEXPLORED,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::MAIN_WORLD,
        ));
        world.insert_resource(images);
        world.insert_resource(MinimapImageHandle(handle.clone()));
        world.insert_resource(MinimapZoom(DEFAULT_ZOOM));
        world.insert_resource(MinimapWindow::default());

        let mut minimap = MinimapData::default();
        minimap.update_tile(&SPAWN, 200, 150);
        world.insert_resource(minimap);

        world.spawn((
            Player {
                agent_id: crate::agent::AgentId(1),
            },
            SPAWN,
        ));
        let node = world
            .spawn((MinimapImageNode, ImageNode::new(handle.clone())))
            .id();

        world.run_system_once(update_minimap_image).unwrap();

        let origin = world.resource::<MinimapWindow>().origin;
        let data = world
            .resource::<Assets<Image>>()
            .get(&handle)
            .unwrap()
            .data
            .clone()
            .unwrap();
        let (r, g, b) = minimap_color_to_rgb(200);
        assert_eq!(
            pixel_at(&data, SPAWN.x - origin.x as u16, SPAWN.y - origin.y as u16),
            [r, g, b, 255]
        );
        assert_eq!(
            world.get::<ImageNode>(node).unwrap().rect,
            Some(player_centered_rect(
                &SPAWN,
                origin,
                &MinimapZoom(DEFAULT_ZOOM)
            ))
        );
    }

    #[test]
    fn a_floor_change_repaints() {
        let window = window_at(&SPAWN);
        let upstairs = Position {
            z: SPAWN.z - 1,
            ..SPAWN
        };

        assert_eq!(window.retarget(&upstairs), Some(centred_origin(&upstairs)));
    }

    #[derive(Resource, Default)]
    struct Walked(Vec<Position>);

    /// A player at `SPAWN` and a minimap image node showing the default zoom around
    /// them, with every `WalkTo` recorded in `Walked`.
    fn a_world_with_a_minimap() -> (World, Entity) {
        let mut world = World::new();
        let window = window_at(&SPAWN);
        let view = player_centered_rect(&SPAWN, window.origin, &MinimapZoom(DEFAULT_ZOOM));
        world.insert_resource(window);
        world.init_resource::<Walked>();
        world.spawn((
            Player {
                agent_id: crate::agent::AgentId(1),
            },
            SPAWN,
        ));
        let node = world
            .spawn((
                MinimapImageNode,
                ImageNode {
                    rect: Some(view),
                    ..default()
                },
            ))
            .observe(on_minimap_click)
            .observe(on_minimap_drag)
            .observe(on_minimap_drag_end)
            .id();
        world.add_observer(
            |intent: On<InteractionIntent>, mut walked: ResMut<Walked>| {
                if let InteractionIntent::WalkTo(tile) = &*intent {
                    walked.0.push(tile.clone());
                }
            },
        );
        (world, node)
    }

    fn pointer<E: std::fmt::Debug + Clone + Reflect>(entity: Entity, event: E) -> Pointer<E> {
        Pointer {
            entity,
            pointer_id: PointerId::Mouse,
            pointer_location: Location {
                target: NormalizedRenderTarget::None {
                    width: 1,
                    height: 1,
                },
                position: Vec2::ZERO,
            },
            event,
        }
    }

    fn click(world: &mut World, node: Entity, button: PointerButton, hit: Vec2) {
        world.trigger(pointer(
            node,
            Click {
                button,
                hit: HitData::new(Entity::PLACEHOLDER, 0.0, Some(hit.extend(0.0)), None),
                duration: Duration::ZERO,
            },
        ));
        world.flush();
    }

    fn drag(world: &mut World, node: Entity, distance: Vec2) {
        world.trigger(pointer(
            node,
            Drag {
                button: PointerButton::Primary,
                distance,
                delta: distance,
            },
        ));
        world.flush();
    }

    fn release(world: &mut World, node: Entity, distance: Vec2) {
        world.trigger(pointer(
            node,
            DragEnd {
                button: PointerButton::Primary,
                distance,
            },
        ));
        world.flush();
    }

    #[test]
    fn a_click_walks_to_the_tile_under_the_cursor() {
        let (mut world, node) = a_world_with_a_minimap();

        click(
            &mut world,
            node,
            PointerButton::Primary,
            Vec2::new(0.25, -0.25),
        );

        assert_eq!(
            world.resource::<Walked>().0,
            vec![Position {
                x: SPAWN.x + 20,
                y: SPAWN.y - 20,
                z: SPAWN.z
            }]
        );
    }

    #[test]
    fn a_secondary_click_walks_nowhere() {
        let (mut world, node) = a_world_with_a_minimap();

        click(&mut world, node, PointerButton::Secondary, Vec2::ZERO);

        assert!(world.resource::<Walked>().0.is_empty());
    }

    /// Bevy emits `Click` before `DragEnd`, so the click arrives while the drag
    /// still stands. Leftward, because travel is a signed vector.
    #[test]
    fn a_press_that_travelled_is_not_a_click() {
        let (mut world, node) = a_world_with_a_minimap();
        let travel = Vec2::new(-6.0, 0.0);

        drag(&mut world, node, travel);
        click(&mut world, node, PointerButton::Primary, Vec2::ZERO);
        release(&mut world, node, travel);

        assert!(world.resource::<Walked>().0.is_empty());
    }

    #[test]
    fn the_click_after_a_finished_drag_walks() {
        let (mut world, node) = a_world_with_a_minimap();
        let travel = Vec2::new(-6.0, 0.0);
        drag(&mut world, node, travel);
        release(&mut world, node, travel);

        click(&mut world, node, PointerButton::Primary, Vec2::ZERO);

        assert_eq!(world.resource::<Walked>().0, vec![SPAWN]);
    }

    #[test]
    fn a_press_that_barely_moved_is_still_a_click() {
        let (mut world, node) = a_world_with_a_minimap();

        drag(&mut world, node, Vec2::new(3.0, 0.0));
        click(&mut world, node, PointerButton::Primary, Vec2::ZERO);

        assert_eq!(world.resource::<Walked>().0, vec![SPAWN]);
    }
}
