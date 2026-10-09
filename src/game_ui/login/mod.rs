use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_text_outline::TextOutline;

use crate::conf::ui::login as conf;
use crate::core::{EndGameSession, GameState};
use crate::game_ui::GameUiAssets;

mod charlist;
mod form;
mod update_required;

/// Which login dialog is showing. Exists only while
/// `GameState::LoginScreen` is active; resets to `EnterGame` on re-entry.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(GameState = GameState::LoginScreen)]
pub enum LoginPhase {
    #[default]
    EnterGame,
    LoadingCharacters,
    CharacterList,
    UpdateRequired,
}

/// Error dialog (title, message) carried across the Connecting →
/// LoginScreen transition so the modal spawns AFTER the login form
/// (and therefore stacks on top).
#[derive(Resource, Default)]
pub struct PendingLoginError(pub Option<(String, String)>);

/// Where the login screen should land when it is next entered.
///
/// `LoginPhase` is a sub-state of `LoginScreen`, so it is recomputed to its
/// `EnterGame` default every time the parent state is entered — a `NextState`
/// written before the transition does not survive it. Cancel on the character list
/// also returns to the form while keeping `CharacterList`, so the presence of a
/// session cannot stand in for this either.
#[derive(Resource, Debug)]
pub struct PendingLoginPhase(pub LoginPhase);

#[derive(Component)]
struct LoginBackdrop;

pub struct LoginPlugin;

impl Plugin for LoginPlugin {
    fn build(&self, app: &mut App) {
        app.add_sub_state::<LoginPhase>()
            .init_resource::<PendingLoginError>()
            .add_systems(
                OnEnter(GameState::LoginScreen),
                (spawn_backdrop, apply_pending_login_phase),
            )
            .add_systems(
                OnExit(GameState::LoginScreen),
                (despawn_backdrop, clear_pending_login_phase),
            )
            .add_systems(
                OnEnter(LoginPhase::EnterGame),
                (form::spawn_login_form, form::show_pending_error)
                    .chain()
                    .run_if(not(resource_exists::<PendingLoginPhase>)),
            )
            .add_systems(OnExit(LoginPhase::EnterGame), form::despawn_login_form)
            .add_systems(
                Update,
                (
                    form::tab_switch_field,
                    form::on_login_field_submit,
                    form::update_password_display,
                )
                    .run_if(in_state(LoginPhase::EnterGame)),
            )
            .add_observer(form::on_login_dialog_button)
            .add_observer(form::on_attempt_login)
            .add_systems(
                OnEnter(LoginPhase::CharacterList),
                charlist::spawn_character_list,
            )
            .add_systems(
                OnExit(LoginPhase::CharacterList),
                charlist::despawn_character_list,
            )
            .add_systems(
                Update,
                (
                    charlist::keyboard_navigation,
                    charlist::update_row_highlight,
                )
                    .chain()
                    .run_if(in_state(LoginPhase::CharacterList)),
            )
            .add_observer(charlist::on_charlist_dialog_button)
            .add_observer(charlist::on_confirm_character)
            .add_systems(
                Update,
                (
                    update_required::route_to_update_required
                        .run_if(in_state(GameState::LoginScreen)),
                    form::resolve_deferred_login.run_if(resource_exists::<form::DeferredLogin>),
                ),
            )
            .add_systems(
                OnEnter(LoginPhase::UpdateRequired),
                update_required::spawn_update_required,
            )
            .add_systems(
                OnExit(LoginPhase::UpdateRequired),
                update_required::despawn_update_required,
            )
            .add_observer(update_required::on_update_required_button)
            .add_observer(on_login_error)
            .add_observer(on_connection_lost)
            .add_observer(on_end_game_session);
    }
}

fn on_login_error(
    _: On<crate::network::events::LoginError>,
    state: Res<State<GameState>>,
    mut pending: ResMut<PendingLoginError>,
    mut commands: Commands,
) {
    if *state.get() != GameState::Connecting {
        return;
    }
    pending.0 = Some((
        "Login Failed".to_string(),
        "Your character could not be logged in. Please try again.".to_string(),
    ));
    commands.set_state(GameState::LoginScreen);
}

fn on_connection_lost(
    _: On<crate::network::events::ConnectionLost>,
    state: Res<State<GameState>>,
    mut pending: ResMut<PendingLoginError>,
    mut commands: Commands,
) {
    if *state.get() != GameState::Connecting {
        return;
    }
    pending.0 = Some((
        "Connection Error".to_string(),
        "Cannot connect to the game server.".to_string(),
    ));
    commands.set_state(GameState::LoginScreen);
}

/// The one path from a live session back to the login screen.
fn on_end_game_session(event: On<EndGameSession>, mut commands: Commands) {
    // The reason never changes what happens here — it is logged because a client
    // that drops out of the world should say whether the player asked for it.
    info!("session ended: {:?}", event.reason);
    commands.insert_resource(PendingLoginPhase(LoginPhase::CharacterList));
    commands.set_state(GameState::LoginScreen);
}

fn apply_pending_login_phase(pending: Option<Res<PendingLoginPhase>>, mut commands: Commands) {
    let Some(pending) = pending else {
        return;
    };
    commands.set_state(pending.0);
}

fn clear_pending_login_phase(mut commands: Commands) {
    commands.remove_resource::<PendingLoginPhase>();
}

fn spawn_backdrop(mut commands: Commands, ui_assets: Res<GameUiAssets>) {
    commands
        .spawn((
            LoginBackdrop,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            // Stretch-fills the window; non-16:9 ratios distort the art.
            // Bevy 0.18 has no cover/contain image mode — don't "fix" this
            // by switching to Stretch (identical) or chasing a mode that
            // doesn't exist.
            ImageNode {
                image: ui_assets.title_background.clone(),
                ..default()
            },
            RenderLayers::layer(1),
        ))
        .with_child((
            Text::new("RUSTIBIA"),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: conf::LOGO_FONT_SIZE,
                ..default()
            },
            TextColor(conf::LOGO_COLOR.into()),
            TextOutline {
                width: 3.0,
                color: Color::BLACK,
            },
            Node {
                margin: UiRect::top(Val::Px(conf::LOGO_TOP_MARGIN)),
                ..default()
            },
        ));
}

fn despawn_backdrop(mut commands: Commands, backdrops: Query<Entity, With<LoginBackdrop>>) {
    for entity in &backdrops {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SessionEndReason;
    use bevy::ecs::system::RunSystemOnce;

    /// The account is still logged in, so the player goes back to picking a
    /// character rather than retyping their password.
    #[test]
    fn ending_a_session_returns_to_the_character_list() {
        let mut world = World::new();
        world.insert_resource(State::new(GameState::InGame));
        world.init_resource::<NextState<GameState>>();
        world.add_observer(on_end_game_session);

        world.trigger(EndGameSession {
            reason: SessionEndReason::Disconnected,
        });
        world.flush();

        assert!(matches!(
            world.resource::<NextState<GameState>>(),
            NextState::Pending(GameState::LoginScreen)
        ));
        assert_eq!(
            world.resource::<PendingLoginPhase>().0,
            LoginPhase::CharacterList
        );
    }

    /// `LoginPhase` is a sub-state: re-entering `LoginScreen` recomputes it to its
    /// `EnterGame` default, overwriting anything written before the transition. The
    /// intent therefore has to be carried in a resource and applied on entry.
    #[test]
    fn a_pending_phase_redirects_to_the_character_list() {
        let mut world = World::new();
        world.init_resource::<NextState<LoginPhase>>();
        world.insert_resource(PendingLoginPhase(LoginPhase::CharacterList));

        world.run_system_once(apply_pending_login_phase).unwrap();

        assert!(matches!(
            world.resource::<NextState<LoginPhase>>(),
            NextState::Pending(LoginPhase::CharacterList)
        ));
    }

    /// A normal startup has no pending phase and must be left on the login form.
    #[test]
    fn no_pending_phase_leaves_the_default_alone() {
        let mut world = World::new();
        world.init_resource::<NextState<LoginPhase>>();

        world.run_system_once(apply_pending_login_phase).unwrap();

        assert!(matches!(
            world.resource::<NextState<LoginPhase>>(),
            NextState::Unchanged
        ));
    }
}
