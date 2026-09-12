use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

use crate::conf::ui::{dialog as conf, ui_colors};
use crate::core::ActiveCharacter;
use crate::game_ui::login::LoginPhase;
use crate::game_ui::{
    DialogButton, DialogButtonId, DialogButtonPressed, GameUiAssets, ModalDialog, ModalOrder,
};
use crate::network::login::{CharacterList, GenerateLoginToken};

/// Clamped arrow-key stepping over the character rows.
pub(super) fn step_selection(selected: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (selected as i32 + delta).clamp(0, len as i32 - 1) as usize
}

#[derive(Component)]
pub struct CharacterListDialog {
    pub selected: usize,
    /// (row index, Time::elapsed_secs at click) for double-click detection.
    last_click: Option<(usize, f32)>,
}

#[derive(Component)]
pub(super) struct CharacterRow(usize);

#[derive(Event, Debug)]
pub struct ConfirmCharacter;

pub(super) fn spawn_character_list(
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    characters: Res<CharacterList>,
) {
    commands.remove_resource::<super::PendingLoginPhase>();

    let handle = ModalDialog::new("Select Character")
        .with_buttons([DialogButton::ok(), DialogButton::cancel()])
        .spawn(&mut commands, &ui_assets, &mut order);
    let dialog = handle.root;
    commands.entity(dialog).insert(CharacterListDialog {
        selected: 0,
        last_click: None,
    });

    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
        ))
        .id();

    for (index, character) in characters.characters.iter().enumerate() {
        let row = commands
            .spawn((
                CharacterRow(index),
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_child((
                Text::new(format!("{} \u{2014} {}", character.name, character.level)),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            ))
            .with_child((
                Text::new(character.vocation.clone()),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            ))
            .observe(
                move |click: On<Pointer<Click>>,
                      mut dialogs: Query<&mut CharacterListDialog>,
                      time: Res<Time>,
                      mut commands: Commands| {
                    if click.button != PointerButton::Primary {
                        return;
                    }
                    let Ok(mut state) = dialogs.get_mut(dialog) else {
                        return;
                    };
                    let now = time.elapsed_secs();
                    let is_double = matches!(
                        state.last_click,
                        Some((i, t)) if i == index && now - t < conf::DOUBLE_CLICK_SECS
                    );
                    state.selected = index;
                    if is_double {
                        // Reset so a triple-click doesn't re-fire the confirm.
                        state.last_click = None;
                        commands.trigger(ConfirmCharacter);
                    } else {
                        state.last_click = Some((index, now));
                    }
                },
            )
            .id();
        commands.entity(list).add_child(row);
    }

    let footer = commands
        .spawn((
            Text::new("Account Status: Free Account"),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 10.0,
                ..default()
            },
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
        ))
        .id();

    commands
        .entity(handle.content)
        .add_children(&[list, footer]);
}

pub(super) fn despawn_character_list(
    mut commands: Commands,
    dialogs: Query<Entity, With<CharacterListDialog>>,
) {
    for entity in &dialogs {
        commands.entity(entity).despawn();
    }
}

pub(super) fn keyboard_navigation(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut dialogs: Query<&mut CharacterListDialog>,
    characters: Res<CharacterList>,
) {
    let Ok(mut state) = dialogs.single_mut() else {
        return;
    };
    let delta = if keyboard.just_pressed(KeyCode::ArrowUp) {
        -1
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        1
    } else {
        return;
    };
    let next = step_selection(state.selected, delta, characters.characters.len());
    if next != state.selected {
        state.selected = next;
    }
}

/// Repaints row highlights whenever the selection changes (including the
/// initial spawn, since `Changed` also matches newly added components).
pub(super) fn update_row_highlight(
    dialogs: Query<&CharacterListDialog, Changed<CharacterListDialog>>,
    mut rows: Query<(&CharacterRow, &mut BackgroundColor)>,
) {
    let Ok(state) = dialogs.single() else {
        return;
    };
    for (row, mut color) in &mut rows {
        *color = if row.0 == state.selected {
            BackgroundColor(conf::ROW_SELECTED_COLOR.into())
        } else {
            BackgroundColor(Color::NONE)
        };
    }
}

pub(super) fn on_charlist_dialog_button(
    event: On<DialogButtonPressed>,
    dialogs: Query<(), With<CharacterListDialog>>,
    mut commands: Commands,
) {
    if dialogs.get(event.dialog).is_err() {
        return;
    }
    match event.button {
        DialogButtonId::Ok => commands.trigger(ConfirmCharacter),
        DialogButtonId::Cancel => commands.set_state(LoginPhase::EnterGame),
        _ => {}
    }
}

pub(super) fn on_confirm_character(
    _: On<ConfirmCharacter>,
    mut commands: Commands,
    characters: Res<CharacterList>,
    char_list_q: Single<&CharacterListDialog>,
) {
    let char = characters.characters.get(char_list_q.selected);
    let Some(char) = char else {
        return;
    };
    commands.insert_resource(ActiveCharacter { id: char.id });
    commands.trigger(GenerateLoginToken {
        character_id: char.id,
    });
}

#[cfg(test)]
mod tests {
    use super::step_selection;

    #[test]
    fn steps_down() {
        assert_eq!(step_selection(0, 1, 3), 1);
    }

    #[test]
    fn clamps_at_top() {
        assert_eq!(step_selection(0, -1, 3), 0);
    }

    #[test]
    fn clamps_at_bottom() {
        assert_eq!(step_selection(2, 1, 3), 2);
    }

    #[test]
    fn empty_list_stays_zero() {
        assert_eq!(step_selection(0, 1, 0), 0);
    }

    #[test]
    fn clamps_out_of_range_selection() {
        assert_eq!(step_selection(5, 1, 3), 2);
    }
}
