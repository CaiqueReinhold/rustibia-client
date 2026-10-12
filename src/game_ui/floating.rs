use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use crate::conf::ui::{dialog as conf, ui_colors};
use crate::game_ui::GameUiAssets;
use crate::game_ui::modal::{ModalDialogHandle, spawn_frame};

#[derive(Component)]
pub struct FloatingDialogRoot;

/// The title bar's close button was clicked. Closing is the caller's responsibility.
#[derive(Event, Debug, Clone, Copy)]
pub struct FloatingDialogClosed {
    pub dialog: Entity,
}

/// A panel in the modal's style that leaves the game usable: no backdrop, dragged by its
/// title bar, closed from its top-right corner.
pub struct FloatingDialog {
    title: String,
    width: f32,
    at: Vec2,
}

impl FloatingDialog {
    pub fn new(title: impl Into<String>, width: f32, at: Vec2) -> Self {
        Self {
            title: title.into(),
            width,
            at,
        }
    }

    pub fn spawn(self, commands: &mut Commands, ui_assets: &GameUiAssets) -> ModalDialogHandle {
        let frame = spawn_frame(
            commands,
            ui_assets,
            self.title,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(self.at.x),
                top: Val::Px(self.at.y),
                width: Val::Px(self.width),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
        );
        let root = frame.panel;
        commands.entity(root).insert((
            FloatingDialogRoot,
            DespawnOnExit(crate::core::GameState::InGame),
            GlobalZIndex(conf::Z_FLOATING),
            RenderLayers::layer(1),
        ));

        commands.entity(frame.title_bar).observe(
            move |drag: On<Pointer<Drag>>, mut nodes: Query<&mut Node>| {
                if let Ok(mut node) = nodes.get_mut(root) {
                    node.left = dragged(node.left, drag.delta.x);
                    node.top = dragged(node.top, drag.delta.y);
                }
            },
        );

        let close = commands
            .spawn((
                Node {
                    width: Val::Px(conf::CLOSE_BUTTON_SIZE),
                    height: Val::Px(conf::CLOSE_BUTTON_SIZE),
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
                move |mut click: On<Pointer<Click>>, mut commands: Commands| {
                    click.propagate(false);
                    commands.trigger(FloatingDialogClosed { dialog: root });
                },
            )
            .observe(|mut drag: On<Pointer<DragStart>>| {
                drag.propagate(false);
            })
            .id();
        commands.entity(frame.title_bar).add_child(close);

        ModalDialogHandle {
            root,
            content: frame.content,
        }
    }
}

fn dragged(offset: Val, by: f32) -> Val {
    match offset {
        Val::Px(px) => Val::Px(px + by),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::world::CommandQueue;

    use super::*;
    use crate::game_ui::ModalDialogRoot;
    use crate::game_ui::assets::{UiButtons, UiInventory, UiWindow};

    fn assets() -> GameUiAssets {
        GameUiAssets {
            font: Handle::default(),
            name_font: Handle::default(),
            window: UiWindow::default(),
            inventory: UiInventory::default(),
            background_dark: Handle::default(),
            background_light: Handle::default(),
            bar_overlay: Handle::default(),
            title_background: Handle::default(),
            buttons: UiButtons::default(),
        }
    }

    /// `player::keyboard` stops walking, hotkeys and chat while any `ModalDialogRoot` exists,
    /// so a floating dialog must not carry one.
    #[test]
    fn a_floating_dialog_is_not_a_modal() {
        let mut world = World::new();
        let mut queue = CommandQueue::default();
        let mut commands = Commands::new(&mut queue, &world);
        let handle = FloatingDialog::new("Shop", 400.0, Vec2::new(30.0, 40.0))
            .spawn(&mut commands, &assets());
        queue.apply(&mut world);

        assert!(world.get::<FloatingDialogRoot>(handle.root).is_some());
        assert!(world.get::<ModalDialogRoot>(handle.root).is_none());
        let node = world.get::<Node>(handle.root).unwrap();
        assert_eq!((node.left, node.top), (Val::Px(30.0), Val::Px(40.0)));
    }

    #[test]
    fn dragging_moves_a_pixel_offset_and_leaves_any_other_alone() {
        assert_eq!(dragged(Val::Px(30.0), 5.5), Val::Px(35.5));
        assert_eq!(dragged(Val::Auto, 5.5), Val::Auto);
    }
}
