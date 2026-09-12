use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::conf::ui::{context_menu as conf, ui_colors, z_index::Z_CONTEXT_MENU};
use crate::core::GameState;
use crate::game_ui::GameUiAssets;
use crate::game_ui::scaling::logical_size;

#[derive(Clone, Copy, Debug)]
pub struct ContextMenuEntry {
    pub label: &'static str,
    pub enabled: bool,
}

impl ContextMenuEntry {
    pub fn new(label: &'static str, enabled: bool) -> Self {
        Self { label, enabled }
    }
}

/// An enabled entry was clicked; the menu is despawned straight after.
#[derive(Event, Debug, Clone, Copy)]
pub struct ContextMenuPicked {
    pub menu: Entity,
    pub index: usize,
}

/// The full-screen backdrop a menu hangs from. Callers tell their menus apart by a marker they
/// insert on it.
#[derive(Component)]
pub struct ContextMenuRoot;

#[derive(Component)]
pub(super) struct ContextMenuPanel;

pub struct ContextMenu {
    entries: Vec<ContextMenuEntry>,
}

impl ContextMenu {
    pub fn new(entries: impl IntoIterator<Item = ContextMenuEntry>) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// `at` is a logical cursor position.
    pub fn spawn(self, commands: &mut Commands, ui_assets: &GameUiAssets, at: Vec2) -> Entity {
        let root = commands
            .spawn((
                ContextMenuRoot,
                DespawnOnExit(GameState::InGame),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                GlobalZIndex(Z_CONTEXT_MENU),
                RenderLayers::layer(1),
            ))
            .id();
        commands.entity(root).observe(
            move |mut click: On<Pointer<Click>>, mut commands: Commands| {
                click.propagate(false);
                commands.entity(root).despawn();
            },
        );

        let panel = commands
            .spawn((
                ContextMenuPanel,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(at.x),
                    top: Val::Px(at.y),
                    min_width: Val::Px(conf::MIN_WIDTH),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(Val::Px(1.0)),
                    padding: UiRect::all(Val::Px(conf::PADDING)),
                    ..default()
                },
                BorderColor {
                    top: ui_colors::LIGHT_BORDER_COLOR.into(),
                    right: ui_colors::DARK_BORDER_COLOR.into(),
                    bottom: ui_colors::DARK_BORDER_COLOR.into(),
                    left: ui_colors::LIGHT_BORDER_COLOR.into(),
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
            ))
            .observe(|mut click: On<Pointer<Click>>| click.propagate(false))
            .id();
        commands.entity(root).add_child(panel);

        for (index, entry) in self.entries.into_iter().enumerate() {
            let row = spawn_row(commands, ui_assets, root, index, entry);
            commands.entity(panel).add_child(row);
        }

        root
    }
}

fn spawn_row(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    menu: Entity,
    index: usize,
    entry: ContextMenuEntry,
) -> Entity {
    let label_color = if entry.enabled {
        ui_colors::FONT_COLOR_CONTENT
    } else {
        conf::DISABLED_COLOR
    };
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(conf::ROW_PADDING_X), Val::Px(conf::ROW_PADDING_Y)),
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .with_child((
            Text::new(entry.label),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextColor(label_color.into()),
            Pickable::IGNORE,
        ))
        .id();

    if entry.enabled {
        commands
            .entity(row)
            .observe(
                move |_: On<Pointer<Over>>, mut colors: Query<&mut BackgroundColor>| {
                    if let Ok(mut color) = colors.get_mut(row) {
                        *color = BackgroundColor(conf::HOVER_COLOR.into());
                    }
                },
            )
            .observe(
                move |_: On<Pointer<Out>>, mut colors: Query<&mut BackgroundColor>| {
                    if let Ok(mut color) = colors.get_mut(row) {
                        *color = BackgroundColor(Color::NONE);
                    }
                },
            )
            .observe(
                move |mut click: On<Pointer<Click>>, mut commands: Commands| {
                    click.propagate(false);
                    if click.button != PointerButton::Primary {
                        return;
                    }
                    commands.trigger(ContextMenuPicked { menu, index });
                    commands.entity(menu).despawn();
                },
            );
    }
    row
}

pub(super) fn close_other_context_menus(
    event: On<Add, ContextMenuRoot>,
    roots: Query<Entity, With<ContextMenuRoot>>,
    mut commands: Commands,
) {
    for root in &roots {
        if root != event.entity {
            commands.entity(root).despawn();
        }
    }
}

pub(super) fn close_context_menus_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    roots: Query<Entity, With<ContextMenuRoot>>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    for root in &roots {
        commands.entity(root).despawn();
    }
}

fn clamp_to_window(at: Vec2, size: Vec2, window: Vec2) -> Vec2 {
    at.min(window - size).max(Vec2::ZERO)
}

pub(super) fn keep_context_menus_on_screen(
    window: Single<&Window>,
    mut panels: Query<(&mut Node, &ComputedNode), (With<ContextMenuPanel>, Changed<ComputedNode>)>,
) {
    let bounds = Vec2::new(window.width(), window.height());
    for (mut node, computed) in &mut panels {
        let (Val::Px(left), Val::Px(top)) = (node.left, node.top) else {
            continue;
        };
        let at = Vec2::new(left, top);
        let clamped = clamp_to_window(at, logical_size(computed), bounds);
        if clamped != at {
            node.left = Val::Px(clamped.x);
            node.top = Val::Px(clamped.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn a_menu_that_would_leave_the_window_is_pulled_back_in() {
        let window = Vec2::new(800.0, 600.0);
        let size = Vec2::new(120.0, 80.0);

        assert_eq!(
            clamp_to_window(Vec2::new(100.0, 100.0), size, window),
            Vec2::new(100.0, 100.0)
        );
        assert_eq!(
            clamp_to_window(Vec2::new(750.0, 590.0), size, window),
            Vec2::new(680.0, 520.0)
        );
        assert_eq!(
            clamp_to_window(Vec2::new(-5.0, 10.0), size, window),
            Vec2::new(0.0, 10.0)
        );
    }

    #[test]
    fn opening_a_menu_closes_the_one_already_open() {
        let mut world = World::new();
        world.add_observer(close_other_context_menus);

        let first = world.spawn(ContextMenuRoot).id();
        world.flush();
        let second = world.spawn(ContextMenuRoot).id();
        world.flush();

        assert!(world.get_entity(first).is_err());
        assert!(world.get_entity(second).is_ok());
    }

    #[test]
    fn escape_closes_every_open_menu() {
        let mut world = World::new();
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Escape);
        world.insert_resource(keyboard);
        let menu = world.spawn(ContextMenuRoot).id();

        world
            .run_system_once(close_context_menus_on_escape)
            .unwrap();

        assert!(world.get_entity(menu).is_err());
    }
}
