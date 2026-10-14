use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    picking::hover::Hovered,
    picking::pointer::PointerLocation,
    prelude::*,
    ui_widgets::{ControlOrientation, CoreScrollbarThumb, Scrollbar, ScrollbarPlugin},
    window::CursorIcon,
};
use std::cmp::Reverse;
use std::sync::Mutex;

use crate::{
    conf::ui::{SIDE_PANEL_WIDTH, ui_colors, z_index::Z_WINDOW},
    game_ui::assets::GameUiAssets,
    game_ui::scaling::{logical_center, logical_rect, logical_size, to_logical},
    player::InteractionMode,
};

const Z_DRAGGING_WINDOW: i32 = 20;

const WINDOW_MIN_HEIGHT: f32 = 32.0;
const WINDOW_TITLE_HEIGHT: f32 = 12.0;
const STIFFNESS: f32 = 40.0;
const DAMPING: f32 = 12.0;
const SNAP_THRESHOLD: f32 = 0.5;
const LINE_HEIGHT: f32 = 21.;
const MINIMIZE_VELOCITY: f32 = 1000.0;

static DOCK_ID_COUNTER: Mutex<u8> = Mutex::new(1);
static WINDOW_ID_COUNTER: Mutex<u64> = Mutex::new(1);

pub struct UIWindowPlugin;

impl Plugin for UIWindowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentDockHover>()
            .add_plugins(ScrollbarPlugin)
            .add_systems(
                Update,
                (
                    check_dock_hover,
                    (
                        update_preview_order,
                        apply_preview_order,
                        check_window_position_changed,
                        snap_window_to_place,
                        window_follow_pointer,
                    )
                        .chain(),
                    resize_window,
                    on_window_scroll,
                    animate_minimize_window,
                ),
            )
            .add_observer(on_add_window)
            .add_observer(on_close_window)
            .add_observer(on_minimize_window)
            .add_observer(on_replace_window_content)
            .add_observer(on_commit_order)
            .add_observer(on_rollback_order)
            .add_observer(on_transfer_window)
            .add_observer(on_pointer_enter_dock)
            .add_observer(on_pointer_leave_dock);
    }
}

fn new_window_id() -> u64 {
    let id: u64;
    {
        let mut counter = WINDOW_ID_COUNTER.lock().unwrap();
        id = *counter;
        *counter += 1;
    }
    id
}

fn new_dock_id() -> u8 {
    let id: u8;
    {
        let mut counter = DOCK_ID_COUNTER.lock().unwrap();
        id = *counter;
        *counter += 1;
    }
    id
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

impl WindowId {
    pub fn new() -> Self {
        WindowId(new_window_id())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DockId(pub u8);

impl DockId {
    pub fn new() -> Self {
        DockId(new_dock_id())
    }
}

#[derive(Component, Clone, Copy, Ord, PartialOrd, PartialEq, Eq, Debug)]
pub struct Index(pub usize);

#[derive(Component, Clone, Copy, Ord, PartialOrd, PartialEq, Eq, Debug)]
struct PreviewIndex(pub usize);

#[derive(Component)]
pub struct UiWindowRef {
    pub window_id: WindowId,
}

#[derive(Component)]
pub struct UIWindowDock {
    pub id: DockId,
}

#[derive(Component, Debug)]
pub struct UIWindow {
    pub id: WindowId,
    pub dock_id: DockId,
}

#[derive(Component)]
struct UIWindowTitleBar;

#[derive(Component)]
struct UIScrollableView;

#[derive(Component, Debug)]
struct DraggingIntent {
    window: WindowId,
    start_position: Vec2,
    start_pointer_location: Vec2,
    start_dock: DockId,
    current_dock: Option<DockId>,
    current_index: usize,
    prev_index: usize,
}

#[derive(Component)]
struct ResizeIntent {
    start_height: f32,
    new_height: f32,
}

#[derive(Component)]
struct SnapToPlace;

#[derive(Component)]
struct PreviousPosition {
    position: Vec2,
}

#[derive(Component, Debug)]
struct UIWindowPreview {
    index: usize,
}

#[derive(Event)]
pub struct AddUIWindow {
    pub content: Entity,
    pub default_height: usize,
    pub title: String,
    pub custom_buttons: Vec<Entity>,
}

#[derive(Event)]
pub struct ReplaceUIWindowContent {
    pub window_id: WindowId,
    pub content: Entity,
    pub title: String,
    pub custom_buttons: Vec<Entity>,
}

#[derive(Event)]
struct CommitOrder {
    dock: DockId,
}

#[derive(Event)]
struct RollbackOrder {
    dock: DockId,
}

#[derive(Event)]
struct TransferWindow {
    target_dock: DockId,
    window: WindowId,
}

#[derive(Event)]
struct OverDock {
    dock_id: DockId,
}

#[derive(Event)]
struct LeftDock {
    dock_id: DockId,
}

#[derive(Event)]
pub struct CloseUIWindow {
    pub window_id: WindowId,
}

#[derive(Event)]
pub struct MinimizeUIWindow {
    pub window_id: WindowId,
}

#[derive(Component)]
struct UIWindowMinimized {
    original_height: f32,
}

#[derive(Component)]
struct MinimizeAnimation {
    target_height: f32,
    is_minimizing: bool,
}

#[derive(Resource, Default)]
pub(super) struct CurrentDockHover {
    dock: Option<DockId>,
}

fn on_pointer_enter_dock(
    event: On<OverDock>,
    mut commands: Commands,
    mut drag_q: Query<&mut DraggingIntent>,
    dock_q: Query<(Entity, &UIWindowDock)>,
) {
    if drag_q.is_empty() {
        return;
    }

    let drag = drag_q.single().unwrap();
    let dock = get_dock_by_id(event.dock_id, &dock_q);
    if drag.start_dock != event.dock_id {
        commands.entity(dock).insert(Outline {
            width: px(3),
            offset: px(3),
            color: Color::WHITE,
        });
    }

    if drag.current_dock != Some(event.dock_id) {
        let mut drag = drag_q.single_mut().unwrap();
        drag.current_dock = Some(event.dock_id);
    }
}

fn on_pointer_leave_dock(
    event: On<LeftDock>,
    mut commands: Commands,
    mut drag_q: Query<&mut DraggingIntent>,
    dock_q: Query<(Entity, &UIWindowDock)>,
) {
    let dock = get_dock_by_id(event.dock_id, &dock_q);
    commands.entity(dock).remove::<Outline>();

    if drag_q.is_empty() {
        return;
    }

    let drag = drag_q.single().unwrap();

    if drag.current_dock.is_some() {
        let mut drag = drag_q.single_mut().unwrap();
        drag.current_dock = None;
    }
}

fn check_dock_hover(
    mut commands: Commands,
    pointer_q: Query<&PointerLocation>,
    dock_q: Query<(&UIWindowDock, &UiGlobalTransform, &ComputedNode)>,
    mut current: ResMut<CurrentDockHover>,
) {
    let Ok(pointer) = pointer_q.single() else {
        return;
    };
    let Some(location) = pointer.location() else {
        return;
    };
    let position = location.position;

    for (dock, transform, node) in dock_q.iter() {
        // The pointer is logical and the dock's layout is physical.
        let rect = logical_rect(node, transform);

        if position.x > rect.min.x
            && position.y > rect.min.y
            && position.x < rect.max.x
            && position.y < rect.max.y
        {
            if current.dock != Some(dock.id) {
                current.dock = Some(dock.id);
                commands.trigger(OverDock { dock_id: dock.id });
                break;
            }
        } else if current.dock == Some(dock.id) {
            current.dock = None;
            commands.trigger(LeftDock { dock_id: dock.id });
        }
    }
}

fn set_window_title_bar_content(
    bar: &mut bevy::ecs::relationship::RelatedSpawnerCommands<'_, ChildOf>,
    title: &str,
    ui_assets: &GameUiAssets,
    window_id: WindowId,
    custom_buttons: &[Entity],
) {
    bar.spawn((
        Text::new(title),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: 9.0,
            weight: FontWeight::EXTRA_BOLD,
            ..default()
        },
        Node {
            flex_grow: 1.0,
            ..default()
        },
    ));

    for &button_entity in custom_buttons {
        let parent = bar.target_entity();
        bar.commands_mut()
            .entity(parent)
            .add_one_related::<ChildOf>(button_entity);
    }
    bar.spawn((
        Node {
            width: Val::Px(10.0),
            height: Val::Px(10.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor {
            top: ui_colors::LIGHT_BORDER_COLOR.into(),
            right: ui_colors::DARK_BORDER_COLOR.into(),
            bottom: ui_colors::DARK_BORDER_COLOR.into(),
            left: ui_colors::LIGHT_BORDER_COLOR.into(),
        },
    ))
    .with_child((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode {
            image: ui_assets.window.minimize_button.clone(),
            ..default()
        },
    ))
    .observe(
        move |mut event: On<Pointer<Click>>, mut commands: Commands| {
            event.propagate(false);
            commands.trigger(MinimizeUIWindow { window_id });
        },
    )
    .observe(|mut event: On<Pointer<DragStart>>| {
        event.propagate(false);
    });

    bar.spawn((
        Node {
            width: Val::Px(10.0),
            height: Val::Px(10.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BorderColor {
            top: ui_colors::LIGHT_BORDER_COLOR.into(),
            right: ui_colors::DARK_BORDER_COLOR.into(),
            bottom: ui_colors::DARK_BORDER_COLOR.into(),
            left: ui_colors::LIGHT_BORDER_COLOR.into(),
        },
    ))
    .with_child((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        ImageNode {
            image: ui_assets.window.close_button.clone(),
            ..default()
        },
    ))
    .observe(
        move |mut event: On<Pointer<Click>>, mut commands: Commands| {
            event.propagate(false);
            commands.trigger(CloseUIWindow { window_id });
        },
    )
    .observe(|mut event: On<Pointer<DragStart>>| {
        event.propagate(false);
    });
}

fn on_add_window(
    event: On<AddUIWindow>,
    mut commands: Commands,
    container_q: Query<(
        Entity,
        &UIWindowDock,
        &Index,
        &ComputedNode,
        Option<&Children>,
    )>,
    c_node_q: Query<&ComputedNode>,
    ui_assets: Res<GameUiAssets>,
) {
    let total_height = (event.default_height as f32) + WINDOW_TITLE_HEIGHT;
    let container_entity = find_available_container(&container_q, &c_node_q, total_height);
    let (_, dock, _, _, container_children) = container_q.get(container_entity).unwrap();
    let window_index = match container_children {
        Some(c) => c.len(),
        None => 0,
    };

    let window_id = WindowId::new();
    let window = commands
        .spawn((
            Node {
                left: Val::Px(-2.0),
                width: Val::Px(SIDE_PANEL_WIDTH),
                height: Val::Px(total_height),
                border: UiRect::all(Val::Px(2.0)),
                min_height: Val::Px(WINDOW_MIN_HEIGHT),
                flex_direction: FlexDirection::Column,
                // overflow: Overflow::hidden(),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
            },
            ZIndex(Z_WINDOW),
            UIWindow {
                id: window_id,
                dock_id: dock.id,
            },
            Index(window_index),
        ))
        .with_children(|window| {
            window
                .spawn((
                    Node {
                        min_height: Val::Px(WINDOW_TITLE_HEIGHT),
                        max_height: Val::Px(WINDOW_TITLE_HEIGHT),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(2.0),
                        padding: UiRect::horizontal(Val::Px(2.0)),
                        ..default()
                    },
                    ImageNode {
                        image: ui_assets.background_dark.clone(),
                        image_mode: NodeImageMode::Tiled {
                            tile_x: true,
                            tile_y: true,
                            stretch_value: 1.0,
                        },
                        ..default()
                    },
                    UIWindowTitleBar,
                ))
                .with_children(
                    |bar: &mut bevy::ecs::relationship::RelatedSpawnerCommands<'_, ChildOf>| {
                        set_window_title_bar_content(
                            bar,
                            &event.title,
                            &ui_assets,
                            window_id,
                            &event.custom_buttons,
                        );
                    },
                )
                .observe(start_drag_window)
                .observe(on_drag_window)
                .observe(stop_drag_window);

            window
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Row,
                        overflow: Overflow::hidden(),
                        padding: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    ImageNode {
                        image: ui_assets.background_dark.clone(),
                        image_mode: NodeImageMode::Tiled {
                            tile_x: true,
                            tile_y: true,
                            stretch_value: 1.0,
                        },
                        ..default()
                    },
                ))
                .with_children(|scroll_view| {
                    scroll_view
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                display: Display::Flex,
                                flex_direction: FlexDirection::Row,
                                overflow: Overflow::hidden(),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor {
                                top: ui_colors::DARK_BORDER_COLOR.into(),
                                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                                left: ui_colors::DARK_BORDER_COLOR.into(),
                            },
                            Hovered::default(),
                            UIScrollableView,
                        ))
                        .with_children(|inner| {
                            let scroll_area_id = inner
                                .spawn((
                                    Node {
                                        width: Val::Percent(100.0),
                                        display: Display::Flex,
                                        flex_direction: FlexDirection::Column,
                                        overflow: Overflow::scroll_y(),
                                        ..default()
                                    },
                                    ImageNode {
                                        image: ui_assets.background_light.clone(),
                                        image_mode: NodeImageMode::Tiled {
                                            tile_x: true,
                                            tile_y: true,
                                            stretch_value: 1.0,
                                        },
                                        ..default()
                                    },
                                    ScrollPosition(Vec2::ZERO),
                                ))
                                .with_children(|scroll_content| {
                                    scroll_content
                                        .spawn((Node {
                                            width: Val::Percent(100.0),
                                            overflow: Overflow::clip(),
                                            ..default()
                                        },))
                                        .add_child(event.content);
                                })
                                .id();

                            inner.spawn((
                                Node {
                                    min_width: px(10),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                Scrollbar {
                                    orientation: ControlOrientation::Vertical,
                                    target: scroll_area_id,
                                    min_thumb_length: 10.0,
                                },
                                Children::spawn(Spawn((
                                    Node {
                                        position_type: PositionType::Absolute,
                                        border_radius: BorderRadius::all(px(4)),
                                        ..default()
                                    },
                                    Hovered::default(),
                                    BackgroundColor(Srgba::new(0.486, 0.486, 0.529, 1.0).into()),
                                    CoreScrollbarThumb,
                                ))),
                            ));
                        });
                });

            window
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(5.0),
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(-5.0),
                        left: Val::Px(0.0),
                        ..default()
                    },
                    // BackgroundColor(Color::WHITE),
                ))
                .observe(on_over_resize_handle)
                .observe(on_out_resize_handle)
                .observe(on_resize_start)
                .observe(on_resize_change)
                .observe(on_resize_end);
        })
        .id();
    commands
        .entity(event.content)
        .insert(UiWindowRef { window_id });
    commands.entity(container_entity).add_child(window);
}

fn on_replace_window_content(
    event: On<ReplaceUIWindowContent>,
    mut commands: Commands,
    window_q: Query<(Entity, &UIWindow)>,
    children_q: Query<&Children>,
    ui_assets: Res<GameUiAssets>,
) {
    let Some((window_entity, window)) = window_q.iter().find(|(_, w)| w.id == event.window_id)
    else {
        return;
    };

    let Ok(children) = children_q.get(window_entity) else {
        return;
    };
    let Some(title_entity) = children.first() else {
        return;
    };

    commands.entity(*title_entity).despawn_children();
    commands.entity(*title_entity).with_children(|title_bar| {
        set_window_title_bar_content(
            title_bar,
            &event.title,
            &ui_assets,
            event.window_id,
            &event.custom_buttons,
        );
    });

    let Ok(scroll_content) = children_q
        .get(*children.get(1).unwrap())
        .and_then(|c| children_q.get(*c.first().unwrap()))
        .and_then(|c| children_q.get(*c.first().unwrap()))
        .map(|c| *c.first().unwrap())
    else {
        return;
    };
    commands.entity(scroll_content).despawn_children();
    commands.entity(scroll_content).add_child(event.content);

    commands.entity(event.content).insert(UiWindowRef {
        window_id: window.id,
    });
}

fn on_close_window(
    event: On<CloseUIWindow>,
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock)>,
    window_q: Query<(Entity, &UIWindow, &Index)>,
) {
    let Some((window_entity, window, _)) =
        window_q.iter().find(|(_, w, _)| w.id == event.window_id)
    else {
        return;
    };

    let dock_id = window.dock_id;
    let dock_entity = get_dock_by_id(dock_id, &dock_q);

    commands.entity(dock_entity).detach_child(window_entity);
    commands.entity(window_entity).despawn();

    let mut siblings: Vec<(Entity, usize)> = window_q
        .iter()
        .filter(|(_, w, _)| w.dock_id == dock_id && w.id != event.window_id)
        .map(|(e, _, i)| (e, i.0))
        .collect();
    siblings.sort_by_key(|(_, i)| *i);
    for (new_idx, (e, _)) in siblings.into_iter().enumerate() {
        commands.entity(e).insert(Index(new_idx));
    }
}

fn on_minimize_window(
    event: On<MinimizeUIWindow>,
    mut commands: Commands,
    mut window_q: Query<(
        Entity,
        &UIWindow,
        &mut Node,
        Option<&UIWindowMinimized>,
        &Children,
    )>,
    mut visibility_q: Query<&mut Visibility, With<UIScrollableView>>,
) {
    let Some((window_entity, _, mut node, minimized, children)) = window_q
        .iter_mut()
        .find(|(_, w, _, _, _)| w.id == event.window_id)
    else {
        return;
    };

    if let Some(minimized) = minimized {
        let original_height = minimized.original_height;
        if let Some(&scroll_child) = children.get(1)
            && let Ok(mut vis) = visibility_q.get_mut(scroll_child)
        {
            *vis = Visibility::Inherited;
        }
        node.min_height = Val::Px(WINDOW_MIN_HEIGHT);
        commands.entity(window_entity).insert(MinimizeAnimation {
            target_height: original_height,
            is_minimizing: false,
        });
    } else {
        let current_height = val_to_f32(node.height);
        node.min_height = Val::Px(WINDOW_TITLE_HEIGHT);
        commands.entity(window_entity).insert((
            UIWindowMinimized {
                original_height: current_height,
            },
            MinimizeAnimation {
                target_height: WINDOW_TITLE_HEIGHT,
                is_minimizing: true,
            },
        ));
    }
}

fn animate_minimize_window(
    mut commands: Commands,
    time: Res<Time>,
    mut window_q: Query<(Entity, &mut Node, &MinimizeAnimation, &Children), With<UIWindow>>,
    mut visibility_q: Query<&mut Visibility, With<UIScrollableView>>,
) {
    let dt = time.delta().as_secs_f32();

    for (entity, mut node, anim, children) in window_q.iter_mut() {
        let current_height = val_to_f32(node.height);
        let displacement = current_height - anim.target_height;
        let new_height = current_height - MINIMIZE_VELOCITY * dt;

        if displacement <= SNAP_THRESHOLD {
            node.height = Val::Px(anim.target_height);
            let is_minimizing = anim.is_minimizing;
            commands.entity(entity).remove::<MinimizeAnimation>();

            if let Some(&scroll_child) = children.get(1) {
                if is_minimizing {
                    if let Ok(mut vis) = visibility_q.get_mut(scroll_child) {
                        *vis = Visibility::Hidden;
                    }
                } else {
                    commands.entity(entity).remove::<UIWindowMinimized>();
                }
            }
        } else {
            node.height = Val::Px(new_height);
        }
    }
}

fn find_available_container(
    dock_q: &Query<(
        Entity,
        &UIWindowDock,
        &Index,
        &ComputedNode,
        Option<&Children>,
    )>,
    node_q: &Query<&ComputedNode>,
    height: f32,
) -> Entity {
    let containers = dock_q
        .iter()
        .sort_by_key::<&Index, _>(|i| i.0)
        .map(|e| (e.0, e.3, e.4));
    let default_container = dock_q
        .iter()
        .sort_by_key::<&Index, _>(|i| i.0)
        .map(|e| e.0)
        .next()
        .unwrap();

    for (entity, node, children) in containers {
        // `height` comes from `Val::Px` sizes, so compare in logical pixels.
        let container_height = logical_size(node).y;
        let mut sum_children_height: f32 = 0.0;

        let Some(children) = children else {
            return entity;
        };

        for c in children.iter() {
            let Ok(node) = node_q.get(c) else {
                continue;
            };
            sum_children_height += logical_size(node).y;
        }

        if (container_height - sum_children_height) >= height {
            return entity;
        }
    }

    default_container
}

fn on_over_resize_handle(
    mut event: On<Pointer<Over>>,
    mut commands: Commands,
    window: Single<Entity, With<Window>>,
    parent_q: Query<&ChildOf>,
    minimized_window_q: Query<&UIWindowMinimized>,
    mode: Res<InteractionMode>,
) {
    event.propagate(false);

    if mode.is_targeting() {
        return;
    }

    let parent = parent_q.get(event.entity).unwrap();
    if minimized_window_q.get(parent.parent()).is_ok() {
        return;
    };

    commands.entity(*window).insert(CursorIcon::System(
        bevy::window::SystemCursorIcon::RowResize,
    ));
}

fn on_out_resize_handle(
    mut event: On<Pointer<Out>>,
    mut commands: Commands,
    window: Single<Entity, With<Window>>,
    parent_q: Query<&ChildOf>,
    minimized_window_q: Query<&UIWindowMinimized>,
    mode: Res<InteractionMode>,
) {
    event.propagate(false);

    if mode.is_targeting() {
        return;
    }

    let parent = parent_q.get(event.entity).unwrap();
    if minimized_window_q.get(parent.parent()).is_ok() {
        return;
    };

    commands
        .entity(*window)
        .insert(CursorIcon::System(bevy::window::SystemCursorIcon::Default));
}

fn on_resize_start(
    mut event: On<Pointer<DragStart>>,
    mut commands: Commands,
    parent_q: Query<&ChildOf>,
    node_q: Query<&ComputedNode>,
    minimized_window_q: Query<&UIWindowMinimized>,
) {
    event.propagate(false);

    let resize_handle = parent_q.get(event.entity).unwrap();

    if minimized_window_q.get(resize_handle.parent()).is_ok() {
        return;
    };

    let node = node_q.get(resize_handle.parent()).unwrap();

    commands
        .entity(resize_handle.parent())
        .insert(ResizeIntent {
            start_height: logical_size(node).y,
            new_height: logical_size(node).y,
        });
}

fn on_resize_change(mut event: On<Pointer<Drag>>, mut intent: Single<&mut ResizeIntent>) {
    event.propagate(false);

    intent.new_height = intent.start_height + event.distance.y;
}

fn on_resize_end(
    mut event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    intent: Single<Entity, With<ResizeIntent>>,
) {
    event.propagate(false);

    commands.entity(*intent).remove::<ResizeIntent>();
}

fn resize_window(
    dock_q: Query<(&UIWindowDock, &ComputedNode)>,
    intent_window: Single<(Entity, &ResizeIntent, &UIWindow, &Index)>,
    window_q: Query<(Entity, &UIWindow, &Index, &ComputedNode)>,
    mut resize_node_q: Query<&mut Node>,
) {
    let (resize_ent, intent, window, index) = *intent_window;

    if intent.new_height < WINDOW_MIN_HEIGHT {
        return;
    }

    let (_, dock_layout) = dock_q.iter().find(|(d, _)| d.id == window.dock_id).unwrap();
    let dock_total_size = logical_size(dock_layout).y;
    let other_windows_allocated_size: f32 = window_q
        .iter()
        .filter(|(_, w, _, _)| w.dock_id == window.dock_id && w.id != window.id)
        .map(|(_, _, _, cn)| logical_size(cn).y)
        .sum();
    let mut need_to_reclaim: f32 = 0.0;

    if (other_windows_allocated_size + intent.new_height) > dock_total_size {
        let mut windows_to_shrink: Vec<(Entity, &Index, f32)> = window_q
            .iter()
            .filter(|(_, w, i, cn)| {
                w.dock_id == window.dock_id
                    && *i > index
                    && (logical_size(cn).y - WINDOW_MIN_HEIGHT) >= 0.0
            })
            .map(|(e, _, i, cn)| (e, i, logical_size(cn).y - WINDOW_MIN_HEIGHT))
            .collect();
        windows_to_shrink.sort_by_key(|(_, i, _)| Reverse(i.0));
        let total_reclaimable_size: f32 = windows_to_shrink.iter().map(|(_, _, size)| *size).sum();

        if total_reclaimable_size <= 0.0 {
            if (other_windows_allocated_size + intent.start_height) < dock_total_size {
                let mut window_node = resize_node_q.get_mut(resize_ent).unwrap();
                window_node.height = Val::Px(dock_total_size - other_windows_allocated_size);
            }
            return;
        }

        need_to_reclaim = (other_windows_allocated_size + intent.new_height) - dock_total_size;

        if windows_to_shrink.is_empty() {
            return;
        }

        for (e, _, reclaimable) in windows_to_shrink.into_iter() {
            let diff = reclaimable - need_to_reclaim;
            let mut window_node = resize_node_q.get_mut(e).unwrap();
            if diff > 0.0 {
                window_node.height = Val::Px(val_to_f32(window_node.height) - need_to_reclaim);
                need_to_reclaim = 0.0;
                break;
            } else {
                window_node.height = Val::Px(val_to_f32(window_node.height) - reclaimable);
                need_to_reclaim = -diff;
            }
        }
    }

    let mut window_node = resize_node_q.get_mut(resize_ent).unwrap();
    window_node.height = Val::Px(intent.new_height - need_to_reclaim);
}

fn start_drag_window(
    mut event: On<Pointer<DragStart>>,
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock)>,
    titles_q: Query<&ChildOf, With<UIWindowTitleBar>>,
    window_q: Query<(Entity, &UIWindow, &Index, &UiGlobalTransform)>,
    computed_q: Query<&ComputedNode>,
    mut window_node_q: Query<&mut Node, With<UIWindow>>,
) {
    event.propagate(false);

    let parent = titles_q.get(event.event_target()).unwrap();
    let (windown_entity, window, index, transform) = window_q.get(parent.parent()).unwrap();
    let computed = computed_q.get(parent.parent()).unwrap();

    commands.entity(parent.parent()).insert((
        DraggingIntent {
            window: window.id,
            start_dock: window.dock_id,
            start_pointer_location: event.pointer_location.position,
            // Logical, so it stays comparable with the pointer location above.
            start_position: logical_center(computed, transform),
            current_index: index.0,
            current_dock: Some(window.dock_id),
            prev_index: index.0,
        },
        GlobalZIndex(Z_DRAGGING_WINDOW),
    ));

    let all_windows_in_dock = window_q
        .iter()
        .filter(|(_, w, _, _)| w.dock_id == window.dock_id);
    all_windows_in_dock.for_each(|(e, _, i, _)| {
        commands.entity(e).insert(PreviewIndex(i.0));
    });

    let mut node = window_node_q.get_mut(windown_entity).unwrap();
    node.position_type = PositionType::Absolute;

    let dock_entity = get_dock_by_id(window.dock_id, &dock_q);
    commands.entity(dock_entity).with_child((
        Node {
            width: Val::Percent(100.0),
            height: node.height,
            ..default()
        },
        UIWindowPreview { index: index.0 },
    ));
}

fn stop_drag_window(
    mut event: On<Pointer<DragEnd>>,
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock)>,
    drag_q: Query<(Entity, &DraggingIntent)>,
    mut node_q: Query<&mut Node, With<UIWindow>>,
    preview_q: Query<Entity, With<UIWindowPreview>>,
) {
    event.propagate(false);

    dock_q.iter().for_each(|(e, _)| {
        commands.entity(e).remove::<Outline>();
    });

    let Ok((drag_ent, drag)) = drag_q.single() else {
        return;
    };

    if Some(drag.start_dock) == drag.current_dock {
        commands.trigger(CommitOrder {
            dock: drag.start_dock,
        });
    } else if drag.current_dock.is_none() {
        commands.trigger(RollbackOrder {
            dock: drag.start_dock,
        });
    } else {
        commands.trigger(TransferWindow {
            target_dock: drag.current_dock.unwrap(),
            window: drag.window,
        });
    }

    commands
        .entity(drag_ent)
        .remove::<(DraggingIntent, GlobalZIndex)>()
        .insert(SnapToPlace);

    let mut node = node_q.get_mut(drag_ent).unwrap();
    node.position_type = PositionType::Relative;

    let Ok(preview) = preview_q.single() else {
        return;
    };
    let dock = get_dock_by_id(drag.start_dock, &dock_q);
    commands.entity(dock).detach_child(preview);
    commands.entity(preview).despawn();
}

fn on_drag_window(
    event: On<Pointer<Drag>>,
    windows_q: Query<(&UIWindow, &PreviewIndex, &ComputedNode)>,
    dock_q: Query<(&UIWindowDock, &UiGlobalTransform, &ComputedNode)>,
    mut drag_q: Query<&mut DraggingIntent>,
) {
    if drag_q.is_empty() {
        return;
    }

    let drag = drag_q.single().unwrap();
    let pointer_position = event.pointer_location.position;

    let (dock_layout, dock_node) = dock_q
        .iter()
        .filter(|(d, _, _)| d.id == drag.start_dock)
        .map(|(_, t, n)| (t, n))
        .next()
        .unwrap();
    // Everything below is logical, to match the pointer position.
    let dock_start_position = logical_rect(dock_node, dock_layout).min;
    let offseted_pointer_position = pointer_position - dock_start_position;

    let windows: Vec<&ComputedNode> = windows_q
        .iter()
        .sort_by_key::<&PreviewIndex, _>(|i| i.0)
        .filter(|(w, _, _)| w.dock_id == drag.start_dock)
        .map(|(_, _, n)| n)
        .collect();

    let mut target_index = windows.len() - 1;
    let mut top = 0.0;
    for (i, node) in windows.into_iter().enumerate() {
        let height = logical_size(node).y;
        let mid = top + height / 2.0;
        let bottom = top + height;

        if i < drag.current_index {
            if offseted_pointer_position.y < mid {
                target_index = i;
                break;
            }
        } else if offseted_pointer_position.y < bottom {
            target_index = i;
            break;
        }

        top += height;
    }

    if target_index != drag.current_index {
        let mut drag = drag_q.single_mut().unwrap();
        drag.prev_index = drag.current_index;
        drag.current_index = target_index;
    }
}

fn on_commit_order(
    event: On<CommitOrder>,
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock)>,
    mut window_q: Query<(
        Entity,
        &UIWindow,
        &mut Index,
        &PreviewIndex,
        &UiGlobalTransform,
    )>,
) {
    window_q
        .iter_mut()
        .filter(|(_, w, _, _, _)| w.dock_id == event.dock)
        .for_each(|(e, _, mut idx, pidx, _)| {
            idx.0 = pidx.0;
            commands.entity(e).remove::<PreviewIndex>();
        });

    let dock_entity = get_dock_by_id(event.dock, &dock_q);
    let mut lens = window_q.transmute_lens::<(Entity, &UIWindow, &Index, &UiGlobalTransform)>();
    inner_apply_order(dock_entity, event.dock, &mut commands, &lens.query(), None);
}

fn on_rollback_order(
    event: On<RollbackOrder>,
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock)>,
    window_q: Query<(Entity, &UIWindow, &Index, &UiGlobalTransform)>,
) {
    window_q.iter().for_each(|(e, _, _, _)| {
        commands.entity(e).remove::<PreviewIndex>();
    });
    let dock_entity = get_dock_by_id(event.dock, &dock_q);
    inner_apply_order(dock_entity, event.dock, &mut commands, &window_q, None);
}

fn on_transfer_window(
    event: On<TransferWindow>,
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock)>,
    mut window_q: Query<(Entity, &mut UIWindow, &mut Index)>,
    preview_idx_q: Query<Entity, With<PreviewIndex>>,
) {
    let mut lens = window_q.transmute_lens::<(Entity, &UIWindow)>();
    let q = lens.query();
    let (w_entity, window) = get_window_by_id(event.window, &q);

    let source_dock = get_dock_by_id(window.dock_id, &dock_q);
    commands.entity(source_dock).detach_child(w_entity);

    let ro_q = window_q.as_readonly();
    let last_index = ro_q
        .iter()
        .sort_by_key::<&Index, _>(|i| **i)
        .filter(|(_, w, _)| w.dock_id == event.target_dock)
        .map(|(_, _, i)| i)
        .last();
    let new_index = match last_index {
        Some(i) => i.0,
        None => 0,
    };

    let (_, mut window, mut index) = window_q.get_mut(w_entity).unwrap();
    window.dock_id = event.target_dock;
    index.0 = new_index + 1;

    let target_dock = get_dock_by_id(event.target_dock, &dock_q);
    commands.entity(target_dock).add_child(w_entity);

    preview_idx_q.iter().for_each(|e| {
        commands.entity(e).remove::<PreviewIndex>();
    });
}

fn window_follow_pointer(
    pointer_q: Query<&PointerLocation>,
    mut drag_q: Query<(
        &mut UiTransform,
        &UiGlobalTransform,
        &ComputedNode,
        &DraggingIntent,
    )>,
) {
    if pointer_q.is_empty() || drag_q.is_empty() {
        return;
    }

    let Some(pointer_location) = pointer_q.single().unwrap().location() else {
        return;
    };
    let (mut transform, global_transform, computed, intent) = drag_q.single_mut().unwrap();

    let current_translation = val2_to_vec2(transform.translation);

    // `UiTransform` is logical and `UiGlobalTransform` is physical, so the global
    // position has to come back to logical before the two are subtracted.
    let current_position = logical_center(computed, global_transform) - current_translation;
    let pointer_offset = intent.start_position - intent.start_pointer_location;
    let diff = pointer_location.position - current_position + pointer_offset;
    transform.translation = Val2::px(diff.x, diff.y);
}

fn snap_window_to_place(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut UiTransform), With<SnapToPlace>>,
) {
    let dt = time.delta().as_secs_f32();

    for (e, mut t) in q.iter_mut() {
        let target = Vec2::ZERO;
        let mut current = val2_to_vec2(t.translation);

        let delta = target - current;

        if (delta.x.abs() <= SNAP_THRESHOLD) && (delta.y.abs() <= SNAP_THRESHOLD) {
            t.translation = Val2::ZERO;
            commands.entity(e).remove::<SnapToPlace>();
            return;
        }

        current += delta * STIFFNESS * dt;
        current *= (-DAMPING * dt).exp();

        t.translation = Val2::px(current.x, current.y);
    }
}

fn update_preview_order(
    mut windows_q: Query<(&mut PreviewIndex, &UIWindow, &ZIndex)>,
    mut preview_q: Query<(&mut UIWindowPreview, &mut Node)>,
    drag_q: Query<&DraggingIntent, Changed<DraggingIntent>>,
) {
    if drag_q.is_empty() {
        return;
    }

    let drag = drag_q.single().unwrap();
    let (mut preview, mut preview_node) = preview_q.single_mut().unwrap();

    info!("{:?}", drag);
    preview.index = drag.current_index;
    if drag.current_dock == Some(drag.start_dock) {
        preview_node.position_type = PositionType::Relative;
    } else {
        preview_node.position_type = PositionType::Absolute;
        return;
    }
    info!("{:?}", preview_node.position_type);

    let windows = windows_q
        .iter_mut()
        .filter(|(_, w, _)| w.dock_id == drag.start_dock);
    windows.for_each(|(mut idx, w, z)| {
        info!("{:?} {:?}", w.id, z);
        if idx.0 == drag.prev_index {
            idx.0 = drag.current_index;
        } else if drag.prev_index < drag.current_index {
            if idx.0 > drag.prev_index && idx.0 <= drag.current_index {
                idx.0 -= 1;
            }
        } else if idx.0 < drag.prev_index && idx.0 >= drag.current_index {
            idx.0 += 1;
        }
    });
}

fn apply_preview_order(
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock)>,
    window_q: Query<(Entity, &UIWindow, &PreviewIndex, &UiGlobalTransform)>,
    drag_q: Query<&DraggingIntent, Changed<DraggingIntent>>,
    preview_q: Query<(Entity, &UIWindowPreview)>,
) {
    if drag_q.is_empty() {
        return;
    }

    let preview = preview_q.single().unwrap();

    let drag = drag_q.single().unwrap();

    if drag.current_dock.is_none() || drag.current_dock != Some(drag.start_dock) {
        return;
    }

    let dock_entity = get_dock_by_id(drag.start_dock, &dock_q);
    inner_apply_order(
        dock_entity,
        drag.start_dock,
        &mut commands,
        &window_q,
        Some(preview),
    );
}

fn check_window_position_changed(
    mut commands: Commands,
    mut window_q: Query<
        (
            Entity,
            &UiGlobalTransform,
            &ComputedNode,
            &PreviousPosition,
            &mut UiTransform,
        ),
        Without<DraggingIntent>,
    >,
    drag_q: Query<Entity, With<DraggingIntent>>,
) {
    if drag_q.is_empty() {
        return;
    }

    let drag_window = drag_q.single().unwrap();
    for (entity, gtransform, computed, prev, mut transform) in window_q.iter_mut() {
        if gtransform.translation != prev.position && entity != drag_window {
            // Both positions are physical; the offset they produce is written back
            // as `Val::Px`, so it has to be converted before it is applied.
            let t = to_logical(computed, prev.position - gtransform.translation);
            transform.translation = Val2::px(t.x, t.y);
            commands.entity(entity).insert(SnapToPlace);
        }
        commands.entity(entity).remove::<PreviousPosition>();
    }
}

fn on_window_scroll(
    mut mouse_wheel_reader: MessageReader<MouseWheel>,
    hovered_window_q: Query<(&Hovered, &Children), With<UIScrollableView>>,
    mut scroll_pos_q: Query<(&mut ScrollPosition, &ComputedNode)>,
) {
    let Some((hovered, children)) = hovered_window_q.iter().find(|(h, _)| h.get()) else {
        return;
    };

    if !hovered.0 {
        return;
    }

    let Ok((mut scroll_pos, computed)) = scroll_pos_q.get_mut(*children.first().unwrap()) else {
        return;
    };
    let max_offset = (computed.content_size() - computed.size()) * computed.inverse_scale_factor();

    for mouse_wheel in mouse_wheel_reader.read() {
        let mut delta = -Vec2::new(mouse_wheel.x, mouse_wheel.y);
        info!("{:?}", delta);

        if mouse_wheel.unit == MouseScrollUnit::Line {
            delta *= LINE_HEIGHT;
        }

        if scroll_pos.y + delta.y <= 0.0 {
            scroll_pos.y = 0.0;
        } else if scroll_pos.y + delta.y >= max_offset.y {
            scroll_pos.y = max_offset.y;
        } else {
            scroll_pos.y += delta.y;
        }
    }
}

fn get_dock_by_id(dock_id: DockId, dock_q: &Query<(Entity, &UIWindowDock)>) -> Entity {
    dock_q
        .iter()
        .filter(|(_, d)| d.id == dock_id)
        .map(|(e, _)| e)
        .next()
        .unwrap()
}

fn get_window_by_id<'a>(
    window_id: WindowId,
    window_q: &'a Query<(Entity, &UIWindow)>,
) -> (Entity, &'a UIWindow) {
    window_q.iter().find(|(_, w)| w.id == window_id).unwrap()
}

fn inner_apply_order<T: Ord + Component + Copy + std::fmt::Debug>(
    container: Entity,
    dock_id: DockId,
    commands: &mut Commands,
    window_q: &Query<(Entity, &UIWindow, &T, &UiGlobalTransform)>,
    preview: Option<(Entity, &UIWindowPreview)>,
) {
    commands.entity(container).detach_all_children();

    info!("-------------");
    window_q
        .iter()
        .sort_by_key::<&T, _>(|i| **i)
        .filter(|(_, w, _, _)| w.dock_id == dock_id)
        .for_each(|(e, w, i, _)| {
            info!("{:?} {:?} {:?}", e, w, i);
        });

    let iter = window_q
        .iter()
        .sort_by_key::<&T, _>(|i| **i)
        .filter(|(_, w, _, _)| w.dock_id == dock_id)
        .map(|(e, _, _, t)| (e, t));

    for (i, (e, t)) in iter.enumerate() {
        info!("Added {}", e);
        commands.entity(container).add_child(e);
        commands.entity(e).insert(PreviousPosition {
            position: t.translation,
        });

        if let Some((prev_ent, prev)) = preview
            && i == prev.index
        {
            info!("{}, {:?}", i, prev);
            commands.entity(container).add_child(prev_ent);
        }
    }
}

fn val2_to_vec2(v: Val2) -> Vec2 {
    Vec2::new(
        match v.x {
            Val::Px(x) => x,
            _ => 0.0,
        },
        match v.y {
            Val::Px(y) => y,
            _ => 0.0,
        },
    )
}

fn val_to_f32(v: Val) -> f32 {
    match v {
        Val::Px(x) => x,
        _ => 0.0,
    }
}
