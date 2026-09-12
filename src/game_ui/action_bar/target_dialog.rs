use bevy::prelude::*;

use crate::conf::ui::{dialog, ui_colors};
use crate::core::{Appearances, ItemConfigs, SpellBook, SpellId};
use crate::game_ui::{
    DialogButton, DialogButtonId, DialogButtonPressed, GameUiAssets, ModalDialog, ModalOrder,
};
use crate::items::ItemId;

use super::bar::{ActionBarAssets, action_image, icon_bundle};
use super::state::{ActionBar, Aim, SlotAction};

const PREVIEW_SIZE: f32 = 40.0;
const AIM_OPTIONS: [(Aim, &str); 3] = [
    (Aim::Yourself, "Use on yourself"),
    (Aim::Target, "Use on target"),
    (Aim::Crosshair, "With crosshair"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingAssignment {
    Spell(SpellId),
    Item(ItemId),
}

impl PendingAssignment {
    pub fn into_action(self, aim: Option<Aim>) -> SlotAction {
        match self {
            PendingAssignment::Spell(id) => SlotAction::Spell { id, aim },
            PendingAssignment::Item(item_id) => SlotAction::Item { item_id, aim },
        }
    }
}

#[derive(Event, Debug, Clone, Copy)]
pub struct OpenTargetDialog {
    pub slot: u16,
    pub pending: PendingAssignment,
}

#[derive(Component, Debug)]
pub(super) struct TargetDialog {
    slot: u16,
    pending: PendingAssignment,
    aim: Aim,
}

#[derive(Component)]
pub(super) struct AimIndicator(Aim);

pub(super) fn on_open_target_dialog(
    event: On<OpenTargetDialog>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    existing: Query<(), With<TargetDialog>>,
    book: Res<SpellBook>,
    items: Res<ItemConfigs>,
    appearances: Res<Appearances>,
    mut assets: ResMut<ActionBarAssets>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    if !existing.is_empty() {
        return;
    }

    let handle = ModalDialog::new(format!("Assign Object to Action Button {}", event.slot + 1))
        .with_buttons([DialogButton::ok(), DialogButton::cancel()])
        .spawn(&mut commands, &ui_assets, &mut order);
    let dialog = handle.root;
    commands.entity(dialog).insert(TargetDialog {
        slot: event.slot,
        pending: event.pending,
        aim: Aim::Yourself,
    });

    let preview = commands
        .spawn((
            Node {
                width: Val::Px(PREVIEW_SIZE),
                height: Val::Px(PREVIEW_SIZE),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(dialog::FIELD_BG_COLOR.into()),
        ))
        .id();
    if let Some(image) = action_image(
        &event.pending.into_action(None),
        &book,
        &items,
        &appearances,
        &mut assets,
        &mut layouts,
    ) {
        let icon = commands.spawn(icon_bundle(image)).id();
        commands.entity(preview).add_child(icon);
    }

    let options = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    for (aim, label) in AIM_OPTIONS {
        let row = spawn_aim_option(&mut commands, &ui_assets, dialog, aim, label);
        commands.entity(options).add_child(row);
    }

    let body = commands
        .spawn(Node {
            column_gap: Val::Px(10.0),
            ..default()
        })
        .add_children(&[preview, options])
        .id();
    commands.entity(handle.content).add_child(body);
}

fn spawn_aim_option(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    dialog: Entity,
    aim: Aim,
    label: &'static str,
) -> Entity {
    let indicator = commands
        .spawn((
            AimIndicator(aim),
            Node {
                width: Val::Px(10.0),
                height: Val::Px(10.0),
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::right(Val::Px(6.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            Pickable::IGNORE,
        ))
        .id();
    commands
        .spawn(Node {
            align_items: AlignItems::Center,
            ..default()
        })
        .add_children(&[indicator, text])
        .observe(
            move |click: On<Pointer<Click>>, mut dialogs: Query<&mut TargetDialog>| {
                if click.button != PointerButton::Primary {
                    return;
                }
                if let Ok(mut state) = dialogs.get_mut(dialog) {
                    state.aim = aim;
                }
            },
        )
        .id()
}

pub(super) fn update_aim_options(
    dialogs: Query<&TargetDialog, Changed<TargetDialog>>,
    mut indicators: Query<(&AimIndicator, &mut BackgroundColor)>,
) {
    let Ok(dialog) = dialogs.single() else {
        return;
    };
    for (indicator, mut color) in &mut indicators {
        *color = if indicator.0 == dialog.aim {
            BackgroundColor(ui_colors::FONT_COLOR_CONTENT.into())
        } else {
            BackgroundColor(Color::NONE)
        };
    }
}

pub(super) fn on_target_dialog_button(
    event: On<DialogButtonPressed>,
    mut commands: Commands,
    dialogs: Query<&TargetDialog>,
    mut bar: ResMut<ActionBar>,
) {
    let Ok(dialog) = dialogs.get(event.dialog) else {
        return;
    };
    if event.button == DialogButtonId::Ok {
        bar.set_action(dialog.slot, dialog.pending.into_action(Some(dialog.aim)));
    }
    commands.entity(event.dialog).despawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_world_with_an_open_dialog(aim: Aim) -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<ActionBar>();
        world.add_observer(on_target_dialog_button);
        let dialog = world
            .spawn(TargetDialog {
                slot: 6,
                pending: PendingAssignment::Item(ItemId(266)),
                aim,
            })
            .id();
        (world, dialog)
    }

    #[test]
    fn a_pending_assignment_takes_the_chosen_aim() {
        assert_eq!(
            PendingAssignment::Spell(SpellId(9)).into_action(Some(Aim::Crosshair)),
            SlotAction::Spell {
                id: SpellId(9),
                aim: Some(Aim::Crosshair)
            }
        );
        assert_eq!(
            PendingAssignment::Item(ItemId(266)).into_action(None),
            SlotAction::Item {
                item_id: ItemId(266),
                aim: None
            }
        );
    }

    #[test]
    fn ok_assigns_with_the_chosen_aim_and_closes() {
        let (mut world, dialog) = a_world_with_an_open_dialog(Aim::Target);

        world.trigger(DialogButtonPressed {
            dialog,
            button: DialogButtonId::Ok,
        });
        world.flush();

        assert_eq!(
            world.resource::<ActionBar>().slot(6).action,
            Some(SlotAction::Item {
                item_id: ItemId(266),
                aim: Some(Aim::Target)
            })
        );
        assert!(world.get_entity(dialog).is_err());
    }

    #[test]
    fn cancel_assigns_nothing_and_closes() {
        let (mut world, dialog) = a_world_with_an_open_dialog(Aim::Target);

        world.trigger(DialogButtonPressed {
            dialog,
            button: DialogButtonId::Cancel,
        });
        world.flush();

        assert!(world.resource::<ActionBar>().slot(6).is_empty());
        assert!(world.get_entity(dialog).is_err());
    }
}
