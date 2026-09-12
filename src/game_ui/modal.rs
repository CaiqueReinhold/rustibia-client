use bevy::camera::visibility::RenderLayers;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;

use crate::conf::ui::{dialog as conf, ui_colors};
use crate::game_ui::GameUiAssets;

/// Identifies which button was pressed in a [`DialogButtonPressed`] event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButtonId {
    Ok,
    Cancel,
    Custom(&'static str),
}

/// Button spec passed to [`ModalDialog::with_buttons`].
#[derive(Debug, Clone, Copy)]
pub struct DialogButton {
    pub id: DialogButtonId,
    pub label: &'static str,
}

impl DialogButton {
    pub fn ok() -> Self {
        Self {
            id: DialogButtonId::Ok,
            label: "Ok",
        }
    }

    pub fn cancel() -> Self {
        Self {
            id: DialogButtonId::Cancel,
            label: "Cancel",
        }
    }

    pub fn custom(id: &'static str, label: &'static str) -> Self {
        Self {
            id: DialogButtonId::Custom(id),
            label,
        }
    }
}

/// Fired when a dialog button is clicked, or via Enter (default button) /
/// Escape (cancel). Callers filter on `dialog` with their own marker
/// components. Closing the dialog is the caller's responsibility.
#[derive(Event, Debug, Clone, Copy)]
pub struct DialogButtonPressed {
    pub dialog: Entity,
    pub button: DialogButtonId,
}

/// Marker + stacking metadata on every modal root (the full-screen backdrop).
#[derive(Component)]
pub struct ModalDialogRoot {
    order: u64,
    default_button: Option<DialogButtonId>,
    has_cancel: bool,
}

impl ModalDialogRoot {
    pub fn order(&self) -> u64 {
        self.order
    }

    #[cfg(test)]
    pub fn for_test(order: u64) -> Self {
        Self {
            order,
            default_button: None,
            has_cancel: false,
        }
    }
}

/// Marker for the content container callers fill with children.
#[derive(Component)]
pub struct ModalContent;

/// Monotonic counter so the most recently spawned modal is topmost
/// (both visually via GlobalZIndex and for keyboard handling).
#[derive(Resource, Default)]
pub struct ModalOrder(u64);

pub struct ModalDialogHandle {
    pub root: Entity,
    pub content: Entity,
}

/// Builder for a Tibia-style modal dialog: full-screen click-blocking
/// backdrop, centered stone panel with title bar, caller-filled content
/// area, bottom-right button row.
pub struct ModalDialog {
    title: String,
    width: Val,
    buttons: Vec<DialogButton>,
}

impl ModalDialog {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            width: Val::Px(conf::DEFAULT_WIDTH),
            buttons: Vec::new(),
        }
    }

    pub fn with_width(mut self, width: Val) -> Self {
        self.width = width;
        self
    }

    pub fn with_buttons(mut self, buttons: impl IntoIterator<Item = DialogButton>) -> Self {
        self.buttons = buttons.into_iter().collect();
        self
    }

    pub fn spawn(
        self,
        commands: &mut Commands,
        ui_assets: &GameUiAssets,
        order: &mut ModalOrder,
    ) -> ModalDialogHandle {
        order.0 += 1;

        let root = commands
            .spawn((
                ModalDialogRoot {
                    order: order.0,
                    default_button: self.buttons.first().map(|b| b.id),
                    has_cancel: self.buttons.iter().any(|b| b.id == DialogButtonId::Cancel),
                },
                DespawnOnExit(crate::core::GameState::InGame),
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                GlobalZIndex(conf::Z_MODAL_BASE + order.0 as i32),
                RenderLayers::layer(1),
            ))
            .id();

        let panel = commands
            .spawn((
                Node {
                    width: self.width,
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BorderColor {
                    top: ui_colors::LIGHT_BORDER_COLOR.into(),
                    right: ui_colors::DARK_BORDER_COLOR.into(),
                    bottom: ui_colors::DARK_BORDER_COLOR.into(),
                    left: ui_colors::LIGHT_BORDER_COLOR.into(),
                },
            ))
            .id();
        commands.entity(root).add_child(panel);

        let panel_inner = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
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
            ))
            .id();
        commands.entity(panel).add_child(panel_inner);

        let title_bar = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(conf::TITLE_BAR_HEIGHT),
                    padding: UiRect::horizontal(Val::Px(6.0)),
                    align_items: AlignItems::Center,
                    flex_shrink: 0.0,
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
            .with_child((
                Text::new(self.title),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(Color::WHITE),
            ))
            .id();

        let content_outer = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    border: UiRect::new(Val::Px(2.0), Val::Px(2.0), Val::Px(0.0), Val::Px(2.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BorderColor::all(ui_colors::DARK_BORDER_COLOR),
            ))
            .id();

        let content_inner = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    border: UiRect::all(Val::Px(1.0)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BorderColor {
                    top: ui_colors::DARK_BORDER_COLOR.into(),
                    right: ui_colors::LIGHT_BORDER_COLOR.into(),
                    bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                    left: ui_colors::DARK_BORDER_COLOR.into(),
                },
            ))
            .id();

        commands.entity(content_outer).add_child(content_inner);

        let content = commands
            .spawn((
                ModalContent,
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(conf::PADDING)),
                    ..default()
                },
            ))
            .id();

        let button_row = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::FlexEnd,
                column_gap: Val::Px(6.0),
                padding: UiRect::new(
                    Val::Px(conf::PADDING),
                    Val::Px(conf::PADDING),
                    Val::Px(0.0),
                    Val::Px(conf::PADDING),
                ),
                ..default()
            })
            .id();

        commands
            .entity(content_inner)
            .add_children(&[content, button_row]);

        for button in &self.buttons {
            let id = button.id;
            let button_entity =
                crate::game_ui::button::spawn_panel_button(commands, button.label, ui_assets);
            commands.entity(button_entity).observe(
                move |_: On<Pointer<Click>>, mut commands: Commands| {
                    commands.trigger(DialogButtonPressed {
                        dialog: root,
                        button: id,
                    });
                },
            );
            commands.entity(button_row).add_child(button_entity);
        }

        commands
            .entity(panel_inner)
            .add_children(&[title_bar, content_outer]);

        ModalDialogHandle { root, content }
    }

    /// Convenience: title + wrapped message text + Ok button.
    /// Returns the root entity so callers can add marker components.
    pub fn message(
        title: impl Into<String>,
        text: impl Into<String>,
        commands: &mut Commands,
        ui_assets: &GameUiAssets,
        order: &mut ModalOrder,
    ) -> Entity {
        let handle = ModalDialog::new(title)
            .with_buttons([DialogButton::ok()])
            .spawn(commands, ui_assets, order);
        commands.entity(handle.content).with_child((
            Text::new(text),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
        ));
        handle.root
    }
}

/// Enter fires the topmost dialog's default button — but not while a text
/// input is focused (Enter there submits the input instead). Escape fires
/// Cancel if the dialog has one, else closes Ok-only popups (default button
/// is Ok), else does nothing (e.g. it must not "press Login" on the login
/// form).
pub fn modal_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    dialogs: Query<(Entity, &ModalDialogRoot)>,
    mut commands: Commands,
) {
    let Some((entity, top)) = dialogs.iter().max_by_key(|(_, root)| root.order) else {
        return;
    };
    if keyboard.just_pressed(KeyCode::Escape) {
        let button = if top.has_cancel {
            Some(DialogButtonId::Cancel)
        } else if top.default_button == Some(DialogButtonId::Ok) {
            top.default_button
        } else {
            None
        };
        if let Some(button) = button {
            commands.trigger(DialogButtonPressed {
                dialog: entity,
                button,
            });
        }
    } else if keyboard.just_pressed(KeyCode::Enter)
        && focus.0.is_none()
        && let Some(button) = top.default_button
    {
        commands.trigger(DialogButtonPressed {
            dialog: entity,
            button,
        });
    }
}
