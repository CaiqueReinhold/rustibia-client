use bevy::prelude::*;

use crate::conf::ui::{dialog, ui_colors};
use crate::game_ui::{
    DialogButton, DialogButtonId, DialogButtonPressed, GameUiAssets, ModalDialog, ModalDialogRoot,
    ModalOrder,
};
use crate::player::{Hotkey, Keybinds};

use super::state::ActionBar;

const CLEAR: &str = "clear";

#[derive(Event, Debug, Clone, Copy)]
pub struct OpenHotkeyDialog {
    pub slot: u16,
}

#[derive(Component, Debug)]
pub(super) struct HotkeyDialog {
    slot: u16,
    captured: Option<Hotkey>,
}

#[derive(Component)]
pub(super) struct HotkeyFieldText;

#[derive(Component)]
pub(super) struct HotkeyWarningText;

#[derive(Debug, PartialEq, Eq)]
pub enum HotkeyVerdict {
    Free,
    Reserved,
    Overwrites(u16),
}

impl HotkeyVerdict {
    pub fn warning(&self) -> &'static str {
        match self {
            HotkeyVerdict::Free => "",
            HotkeyVerdict::Reserved => "This hotkey is reserved and cannot be assigned.",
            HotkeyVerdict::Overwrites(_) => {
                "This hotkey is already in use and will be overwritten."
            }
        }
    }
}

pub fn verdict(
    captured: Option<Hotkey>,
    slot: u16,
    bar: &ActionBar,
    keybinds: &Keybinds,
) -> HotkeyVerdict {
    let Some(hotkey) = captured else {
        return HotkeyVerdict::Free;
    };
    if keybinds.reserves(&hotkey) {
        return HotkeyVerdict::Reserved;
    }
    match bar.slot_with_hotkey(&hotkey) {
        Some(other) if other != slot => HotkeyVerdict::Overwrites(other),
        _ => HotkeyVerdict::Free,
    }
}

fn field_label(captured: Option<Hotkey>) -> String {
    captured.map_or_else(|| "None".to_string(), |hotkey| hotkey.long_label())
}

pub(super) fn on_open_hotkey_dialog(
    event: On<OpenHotkeyDialog>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    existing: Query<(), With<HotkeyDialog>>,
    bar: Res<ActionBar>,
    keybinds: Res<Keybinds>,
) {
    if !existing.is_empty() {
        return;
    }

    let slot = event.slot;
    let button = slot + 1;
    let captured = bar.slot(slot).hotkey;
    let handle = ModalDialog::new(format!(
        "Edit Hotkey for \"Action Bar: Action Button {button}\""
    ))
    .with_width(Val::Px(dialog::HOTKEY_DIALOG_WIDTH))
    .with_buttons([
        DialogButton::ok(),
        DialogButton::custom(CLEAR, "Clear"),
        DialogButton::cancel(),
    ])
    .spawn(&mut commands, &ui_assets, &mut order);
    commands
        .entity(handle.root)
        .insert(HotkeyDialog { slot, captured });

    let font = |size: f32| TextFont {
        font: ui_assets.font.clone(),
        font_size: size,
        ..default()
    };

    let field = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(dialog::FIELD_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::bottom(Val::Px(8.0)),
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
        .with_child((
            HotkeyFieldText,
            Text::new(field_label(captured)),
            font(11.0),
            TextColor(Color::WHITE),
        ))
        .id();
    let help = commands
        .spawn((
            Text::new(format!(
                "Click \"Ok\" to assign the hotkey. Click \"Clear\" to remove the hotkey from \"Action Button {button}\"."
            )),
            font(11.0),
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
        ))
        .id();
    let warning = commands
        .spawn((
            HotkeyWarningText,
            Text::new(verdict(captured, slot, &bar, &keybinds).warning()),
            font(11.0),
            TextColor(dialog::WARNING_COLOR.into()),
            Node {
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
        ))
        .id();
    commands
        .entity(handle.content)
        .add_children(&[field, help, warning]);
}

pub(super) fn capture_hotkey(
    keyboard: Res<ButtonInput<KeyCode>>,
    roots: Query<(Entity, &ModalDialogRoot)>,
    mut dialogs: Query<&mut HotkeyDialog>,
) {
    let Some((top, _)) = roots.iter().max_by_key(|(_, root)| root.order()) else {
        return;
    };
    let Ok(mut dialog) = dialogs.get_mut(top) else {
        return;
    };
    let Some(hotkey) = keyboard
        .get_just_pressed()
        .find_map(|key| Hotkey::from_input(*key, &keyboard))
    else {
        return;
    };
    if dialog.captured != Some(hotkey) {
        dialog.captured = Some(hotkey);
    }
}

pub(super) fn refresh_hotkey_dialog(
    dialogs: Query<&HotkeyDialog, Changed<HotkeyDialog>>,
    bar: Res<ActionBar>,
    keybinds: Res<Keybinds>,
    mut field_q: Query<&mut Text, (With<HotkeyFieldText>, Without<HotkeyWarningText>)>,
    mut warning_q: Query<&mut Text, (With<HotkeyWarningText>, Without<HotkeyFieldText>)>,
) {
    let Ok(dialog) = dialogs.single() else {
        return;
    };
    for mut text in &mut field_q {
        text.0 = field_label(dialog.captured);
    }
    for mut text in &mut warning_q {
        text.0 = verdict(dialog.captured, dialog.slot, &bar, &keybinds)
            .warning()
            .to_string();
    }
}

pub(super) fn on_hotkey_dialog_button(
    event: On<DialogButtonPressed>,
    mut commands: Commands,
    dialogs: Query<&HotkeyDialog>,
    mut bar: ResMut<ActionBar>,
    keybinds: Res<Keybinds>,
) {
    let Ok(dialog) = dialogs.get(event.dialog) else {
        return;
    };
    match event.button {
        DialogButtonId::Ok => {
            if verdict(dialog.captured, dialog.slot, &bar, &keybinds) == HotkeyVerdict::Reserved {
                return;
            }
            match dialog.captured {
                Some(hotkey) => bar.set_hotkey(dialog.slot, hotkey),
                None => bar.clear_hotkey(dialog.slot),
            }
        }
        DialogButtonId::Custom(CLEAR) => bar.clear_hotkey(dialog.slot),
        _ => {}
    }
    commands.entity(event.dialog).despawn();
}

#[cfg(test)]
mod tests {
    use super::super::state::SlotAction;
    use super::*;
    use crate::core::SpellId;
    use bevy::ecs::system::RunSystemOnce;

    const F1: Hotkey = Hotkey {
        key: KeyCode::F1,
        ctrl: false,
        shift: false,
        alt: false,
    };

    #[test]
    fn a_verdict_names_what_the_captured_combo_collides_with() {
        let keybinds = Keybinds::default();
        let mut bar = ActionBar::default();
        bar.set_hotkey(4, F1);

        assert_eq!(verdict(None, 0, &bar, &keybinds), HotkeyVerdict::Free);
        assert_eq!(verdict(Some(F1), 4, &bar, &keybinds), HotkeyVerdict::Free);
        assert_eq!(
            verdict(Some(F1), 0, &bar, &keybinds),
            HotkeyVerdict::Overwrites(4)
        );
        assert_eq!(
            verdict(Some(Hotkey::plain(KeyCode::KeyW)), 0, &bar, &keybinds),
            HotkeyVerdict::Reserved
        );
    }

    fn a_world_with_an_open_dialog(captured: Option<Hotkey>) -> (World, Entity) {
        let mut world = World::new();
        let mut bar = ActionBar::default();
        bar.set_action(
            0,
            SlotAction::Spell {
                id: SpellId(1),
                aim: None,
            },
        );
        bar.set_hotkey(4, F1);
        world.insert_resource(bar);
        world.init_resource::<Keybinds>();
        world.add_observer(on_hotkey_dialog_button);
        let dialog = world.spawn(HotkeyDialog { slot: 0, captured }).id();
        (world, dialog)
    }

    fn press(world: &mut World, dialog: Entity, button: DialogButtonId) {
        world.trigger(DialogButtonPressed { dialog, button });
        world.flush();
    }

    #[test]
    fn ok_moves_the_captured_hotkey_here() {
        let (mut world, dialog) = a_world_with_an_open_dialog(Some(F1));

        press(&mut world, dialog, DialogButtonId::Ok);

        let bar = world.resource::<ActionBar>();
        assert_eq!(bar.slot(0).hotkey, Some(F1));
        assert_eq!(bar.slot(4).hotkey, None);
        assert!(world.get_entity(dialog).is_err());
    }

    #[test]
    fn ok_on_a_reserved_combo_keeps_the_dialog_open() {
        let (mut world, dialog) = a_world_with_an_open_dialog(Some(Hotkey::plain(KeyCode::KeyW)));

        press(&mut world, dialog, DialogButtonId::Ok);

        assert!(world.get_entity(dialog).is_ok());
        assert_eq!(world.resource::<ActionBar>().slot(0).hotkey, None);
    }

    #[test]
    fn clear_removes_the_hotkey_and_keeps_the_action() {
        let (mut world, dialog) = a_world_with_an_open_dialog(Some(F1));
        world
            .resource_mut::<ActionBar>()
            .set_hotkey(0, Hotkey::plain(KeyCode::F2));

        press(&mut world, dialog, DialogButtonId::Custom(CLEAR));

        let bar = world.resource::<ActionBar>();
        assert_eq!(bar.slot(0).hotkey, None);
        assert!(bar.slot(0).action.is_some());
    }

    #[test]
    fn a_key_pressed_into_the_topmost_dialog_is_captured_with_its_modifiers() {
        let mut world = World::new();
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::AltLeft);
        keyboard.press(KeyCode::F5);
        world.insert_resource(keyboard);
        let dialog = world
            .spawn((
                ModalDialogRoot::for_test(),
                HotkeyDialog {
                    slot: 0,
                    captured: None,
                },
            ))
            .id();

        world.run_system_once(capture_hotkey).unwrap();

        assert_eq!(
            world.get::<HotkeyDialog>(dialog).unwrap().captured,
            Some(Hotkey::new(KeyCode::F5, false, false, true))
        );
    }
}
