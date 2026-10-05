use bevy::ecs::message::MessageReader;
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy_ui_text_input::{
    SubmitText, TextInputBuffer, TextInputContents, TextInputMode, TextInputNode, TextInputPrompt,
    TextInputStyle,
};
use cosmic_text::Edit;

use crate::conf::ui::{chat as chat_conf, dialog as conf, ui_colors};
use crate::game_ui::login::{LoginPhase, PendingLoginError};
use crate::game_ui::{
    DialogButton, DialogButtonId, DialogButtonPressed, GameUiAssets, ModalDialog, ModalOrder,
};
use crate::network::login::FetchCharacters;

/// Both fields must be non-blank. Values are otherwise unchecked —
/// the login is fake until the server has real accounts.
pub(super) fn is_valid(email: &str, password: &str) -> bool {
    !email.trim().is_empty() && !password.trim().is_empty()
}

#[derive(Component)]
pub struct LoginForm;

#[derive(Component)]
pub struct EmailField;

#[derive(Component)]
pub struct PasswordField;

#[derive(Component)]
pub(super) struct PasswordDisplay;

/// Bullet run before the cursor position.
#[derive(Component)]
pub(super) struct PasswordBulletsPre;

/// Bullet run after the cursor position.
#[derive(Component)]
pub(super) struct PasswordBulletsPost;

/// 1px caret node sitting between the two bullet runs.
#[derive(Component)]
pub(super) struct PasswordCaret;

/// Marker for error popups spawned by the login flow (validation failures
/// and connection errors). At most one exists at a time.
#[derive(Component)]
pub struct LoginErrorModal;

#[derive(Event, Debug)]
pub struct AttemptLogin;

pub(super) fn spawn_login_form(
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    mut input_focus: ResMut<InputFocus>,
) {
    let handle = ModalDialog::new("Enter Game")
        .with_buttons([DialogButton::custom("login", "Login")])
        .spawn(&mut commands, &ui_assets, &mut order);
    commands.entity(handle.root).insert(LoginForm);

    let email_label = spawn_field_label(&mut commands, &ui_assets, "Email");
    let (email_wrapper, email_input) = spawn_text_field(&mut commands, &ui_assets);
    commands.entity(email_input).insert((
        EmailField,
        TextInputPrompt {
            text: "you@example.com".to_string(),
            color: Some(chat_conf::INPUT_PLACEHOLDER_COLOR.into()),
            ..default()
        },
    ));

    let password_label = spawn_field_label(&mut commands, &ui_assets, "Password");
    let (password_wrapper, password_input) = spawn_text_field(&mut commands, &ui_assets);
    // No native masking in bevy_ui_text_input: the input's own glyphs are
    // drawn fully transparent and a bullet overlay renders on top.
    // The input's own caret/selection are drawn at the real (invisible)
    // glyph positions, which don't line up with the uniform-width bullets,
    // so they're hidden and the overlay renders its own caret instead: a
    // 1px node between the pre-/post-cursor bullet runs, kept aligned by
    // flex layout and blinked by `update_password_display`.
    commands.entity(password_input).insert((
        PasswordField,
        TextColor(Color::NONE),
        TextInputStyle {
            cursor_color: Color::NONE,
            selection_color: Color::NONE,
            ..default()
        },
    ));
    let bullet_font = TextFont {
        font: ui_assets.font.clone(),
        font_size: 12.0,
        ..default()
    };
    let overlay = commands
        .spawn((
            PasswordDisplay,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(3.0),
                top: Val::Px(-2.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|overlay| {
            overlay.spawn((
                PasswordBulletsPre,
                Text::new(""),
                bullet_font.clone(),
                TextColor(chat_conf::TAB_TITLE_COLOR.into()),
                Pickable::IGNORE,
            ));
            overlay.spawn((
                PasswordCaret,
                Node {
                    width: Val::Px(2.0),
                    height: Val::Px(14.0),
                    ..default()
                },
                BackgroundColor(chat_conf::TAB_TITLE_COLOR.into()),
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
            overlay.spawn((
                PasswordBulletsPost,
                Text::new(""),
                bullet_font,
                TextColor(chat_conf::TAB_TITLE_COLOR.into()),
                Pickable::IGNORE,
            ));
        })
        .id();
    commands.entity(password_wrapper).add_child(overlay);

    commands.entity(handle.content).add_children(&[
        email_label,
        email_wrapper,
        password_label,
        password_wrapper,
    ]);

    input_focus.set(email_input);
}

fn spawn_field_label(commands: &mut Commands, ui_assets: &GameUiAssets, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            Node {
                margin: UiRect::bottom(Val::Px(3.0)),
                ..default()
            },
        ))
        .id()
}

fn spawn_text_field(commands: &mut Commands, ui_assets: &GameUiAssets) -> (Entity, Entity) {
    let wrapper = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(conf::FIELD_HEIGHT),
                padding: UiRect::all(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::bottom(Val::Px(8.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(ui_colors::BG_COLOR.into()),
        ))
        .id();
    let input = commands
        .spawn((
            TextInputNode {
                mode: TextInputMode::SingleLine,
                clear_on_submit: false,
                unfocus_on_submit: false,
                is_enabled: true,
                ..default()
            },
            TextInputContents::default(),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 12.0,
                ..default()
            },
            TextColor(chat_conf::TAB_TITLE_COLOR.into()),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
        ))
        .id();
    commands.entity(wrapper).add_child(input);
    (wrapper, input)
}

/// Spawns the queued connection-error popup, if any. Runs chained after
/// `spawn_login_form` so the popup stacks above the form.
pub(super) fn show_pending_error(
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    mut pending: ResMut<PendingLoginError>,
    mut input_focus: ResMut<InputFocus>,
) {
    if let Some((title, message)) = pending.0.take() {
        let root = ModalDialog::message(title, message, &mut commands, &ui_assets, &mut order);
        commands.entity(root).insert(LoginErrorModal);
        input_focus.clear();
    }
}

pub(super) fn despawn_login_form(
    mut commands: Commands,
    roots: Query<Entity, Or<(With<LoginForm>, With<LoginErrorModal>)>>,
    mut input_focus: ResMut<InputFocus>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    input_focus.clear();
}

/// Global observer: handles buttons on the login form AND on error popups.
pub(super) fn on_login_dialog_button(
    event: On<DialogButtonPressed>,
    forms: Query<(), With<LoginForm>>,
    error_modals: Query<(), With<LoginErrorModal>>,
    email: Query<Entity, With<EmailField>>,
    mut input_focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    if error_modals.get(event.dialog).is_ok() {
        commands.entity(event.dialog).despawn();
        if let Ok(email_entity) = email.single() {
            input_focus.set(email_entity);
        }
        return;
    }
    if forms.get(event.dialog).is_ok() && event.button == DialogButtonId::Custom("login") {
        commands.trigger(AttemptLogin);
    }
}

/// Enter inside either text field fires `SubmitText`; treat it as Login.
/// Gated to `LoginPhase::EnterGame` so it can't clash with chat (which is
/// additionally gated to `GameState::InGame`).
pub(super) fn on_login_field_submit(mut events: MessageReader<SubmitText>, mut commands: Commands) {
    for _ in events.read() {
        commands.trigger(AttemptLogin);
    }
}

pub(super) fn on_attempt_login(
    _: On<AttemptLogin>,
    error_modals: Query<(), With<LoginErrorModal>>,
    email: Query<&TextInputContents, With<EmailField>>,
    password: Query<&TextInputContents, With<PasswordField>>,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    mut input_focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    // An error popup is already up — ignore repeat submits so Enter
    // can't stack a second popup.
    if !error_modals.is_empty() {
        return;
    }
    let (Ok(email), Ok(password)) = (email.single(), password.single()) else {
        return;
    };
    if is_valid(email.get(), password.get()) {
        commands.trigger(FetchCharacters {
            email: email.get().to_owned(),
            password: password.get().to_owned(),
        });
        commands.set_state(LoginPhase::LoadingCharacters);
    } else {
        let root = ModalDialog::message(
            "Login Error",
            "Please enter your email address and password.",
            &mut commands,
            &ui_assets,
            &mut order,
        );
        commands.entity(root).insert(LoginErrorModal);
        input_focus.clear();
    }
}

pub(super) fn tab_switch_field(
    keyboard: Res<ButtonInput<KeyCode>>,
    error_modals: Query<(), With<LoginErrorModal>>,
    email: Query<Entity, With<EmailField>>,
    password: Query<Entity, With<PasswordField>>,
    mut input_focus: ResMut<InputFocus>,
) {
    if !keyboard.just_pressed(KeyCode::Tab) {
        return;
    }
    // Don't steal focus to a field behind an open error popup — otherwise
    // Enter stops dismissing the popup.
    if !error_modals.is_empty() {
        return;
    }
    let (Ok(email), Ok(password)) = (email.single(), password.single()) else {
        return;
    };
    let next = if input_focus.0 == Some(email) {
        password
    } else {
        email
    };
    input_focus.set(next);
}

/// Mirrors the password buffer into the bullet overlay and drives the
/// overlay's own caret: bullet runs split at the real cursor position
/// (read from the editor), caret blinking on the field's `blink_interval`
/// while the password field is focused. Runs every frame (blink), but only
/// writes `Text` when the bullet counts actually change.
pub(super) fn update_password_display(
    field: Query<
        (
            Entity,
            &TextInputContents,
            &TextInputBuffer,
            &TextInputStyle,
        ),
        With<PasswordField>,
    >,
    mut pre: Query<&mut Text, (With<PasswordBulletsPre>, Without<PasswordBulletsPost>)>,
    mut post: Query<&mut Text, (With<PasswordBulletsPost>, Without<PasswordBulletsPre>)>,
    mut caret: Query<&mut Visibility, With<PasswordCaret>>,
    input_focus: Res<InputFocus>,
    time: Res<Time>,
) {
    let Ok((entity, contents, buffer, style)) = field.single() else {
        return;
    };
    let (Ok(mut pre), Ok(mut post), Ok(mut caret)) =
        (pre.single_mut(), post.single_mut(), caret.single_mut())
    else {
        return;
    };

    let text = contents.get();
    // Byte offset of the cursor within the (single) line; always lands on a
    // char boundary, but stay defensive against a stale index.
    let cursor_bytes = buffer.editor.cursor().index.min(text.len());
    let chars_before = text
        .get(..cursor_bytes)
        .map_or_else(|| text.chars().count(), |s| s.chars().count());
    let chars_after = text.chars().count() - chars_before;

    set_bullets(&mut pre, chars_before);
    set_bullets(&mut post, chars_after);

    let focused = input_focus.0 == Some(entity);
    let blink_on =
        time.elapsed_secs().rem_euclid(style.blink_interval * 2.0) < style.blink_interval;
    let target = if focused && blink_on {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *caret != target {
        *caret = target;
    }
}

fn set_bullets(text: &mut Text, count: usize) {
    if text.0.chars().count() != count {
        text.0 = "\u{2022}".repeat(count);
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid;

    #[test]
    fn rejects_empty_email() {
        assert!(!is_valid("", "secret"));
    }

    #[test]
    fn rejects_blank_password() {
        assert!(!is_valid("a@b.c", "   "));
    }

    #[test]
    fn rejects_blank_email() {
        assert!(!is_valid("   ", "x"));
    }

    #[test]
    fn accepts_filled_fields() {
        assert!(is_valid("a@b.c", "secret"));
    }
}
