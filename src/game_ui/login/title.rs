use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy_text_outline::TextOutline;

use crate::conf::ui::login as conf;
use crate::core::GameState;
use crate::game_ui::GameUiAssets;
use crate::game_ui::login::LoginPhase;

/// Exists while the title backdrop is on screen.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) struct TitleScreen;

impl ComputedStates for TitleScreen {
    type SourceStates = GameState;
    const ALLOW_SAME_STATE_TRANSITIONS: bool = false;

    fn compute(game: GameState) -> Option<Self> {
        matches!(game, GameState::LoginScreen | GameState::Connecting).then_some(TitleScreen)
    }
}

/// Exists while the player is waiting on the site or the game server.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(super) struct LoginBusy;

impl ComputedStates for LoginBusy {
    type SourceStates = (GameState, Option<LoginPhase>);
    const ALLOW_SAME_STATE_TRANSITIONS: bool = false;

    fn compute((game, phase): (GameState, Option<LoginPhase>)) -> Option<Self> {
        let busy = game == GameState::Connecting
            || matches!(
                phase,
                Some(LoginPhase::LoadingCharacters | LoginPhase::EnteringGame)
            );
        busy.then_some(LoginBusy)
    }
}

pub(super) struct TitleScreenPlugin;

impl Plugin for TitleScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_computed_state::<TitleScreen>()
            .add_computed_state::<LoginBusy>()
            .add_systems(OnEnter(TitleScreen), spawn_backdrop)
            .add_systems(OnEnter(LoginBusy), spawn_spinner)
            .add_systems(Update, animate_spinner.run_if(in_state(LoginBusy)));
    }
}

#[derive(Component)]
struct LoginBackdrop;

fn spawn_backdrop(mut commands: Commands, ui_assets: Res<GameUiAssets>) {
    commands
        .spawn((
            LoginBackdrop,
            DespawnOnExit(TitleScreen),
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

#[derive(Component)]
struct LoginSpinner;

#[derive(Component)]
struct SpinnerDot(usize);

fn spawn_spinner(mut commands: Commands, backdrop: Single<Entity, With<LoginBackdrop>>) {
    let centre = conf::SPINNER_SIZE / 2.0;
    let spinner = commands
        .spawn((
            LoginSpinner,
            DespawnOnExit(LoginBusy),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                width: Val::Px(conf::SPINNER_SIZE),
                height: Val::Px(conf::SPINNER_SIZE),
                margin: UiRect {
                    left: Val::Px(-centre),
                    top: Val::Px(-centre),
                    ..default()
                },
                border_radius: BorderRadius::MAX,
                ..default()
            },
            BackgroundColor(conf::SPINNER_DISC_COLOR.into()),
        ))
        .with_children(|ring| {
            for index in 0..conf::SPINNER_DOTS {
                let angle = index as f32 / conf::SPINNER_DOTS as f32 * std::f32::consts::TAU;
                let offset = centre - conf::SPINNER_DOT_SIZE / 2.0;
                ring.spawn((
                    SpinnerDot(index),
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(offset + conf::SPINNER_RING_RADIUS * angle.cos()),
                        top: Val::Px(offset + conf::SPINNER_RING_RADIUS * angle.sin()),
                        width: Val::Px(conf::SPINNER_DOT_SIZE),
                        height: Val::Px(conf::SPINNER_DOT_SIZE),
                        border_radius: BorderRadius::MAX,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    UiTransform::default(),
                ));
            }
        })
        .id();
    commands.entity(*backdrop).add_child(spinner);
}

fn animate_spinner(
    time: Res<Time>,
    mut dots: Query<(&SpinnerDot, &mut BackgroundColor, &mut UiTransform)>,
) {
    let elapsed = time.elapsed_secs();
    for (dot, mut color, mut transform) in &mut dots {
        let intensity = dot_intensity(dot.0, elapsed);
        color.0 = Srgba {
            alpha: conf::SPINNER_MIN_ALPHA.lerp(1.0, intensity),
            ..conf::SPINNER_COLOR
        }
        .into();
        transform.scale = Vec2::splat(conf::SPINNER_MIN_SCALE.lerp(1.0, intensity));
    }
}

/// 1.0 as the head passes a dot, fading to 0.0 along the tail behind it, then rising back to 1.0
/// over the last step before the head returns. The head goes round once per `SPINNER_PERIOD_SECS`.
fn dot_intensity(index: usize, elapsed_secs: f32) -> f32 {
    let dots = conf::SPINNER_DOTS as f32;
    let head = (elapsed_secs / conf::SPINNER_PERIOD_SECS).fract() * dots;
    let behind = (head - index as f32).rem_euclid(dots);
    let tail = dots - 1.0;
    if behind <= tail {
        1.0 - behind / tail
    } else {
        behind - tail
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_ui::assets::{UiButtons, UiInventory, UiWindow};
    use bevy::state::app::StatesPlugin;
    use bevy::state::state::FreelyMutableState;

    fn title_app() -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin)
            .init_resource::<Time>()
            .insert_resource(GameUiAssets {
                font: Handle::default(),
                name_font: Handle::default(),
                window: UiWindow::default(),
                inventory: UiInventory::default(),
                background_dark: Handle::default(),
                background_light: Handle::default(),
                bar_overlay: Handle::default(),
                title_background: Handle::default(),
                buttons: UiButtons::default(),
            })
            .insert_state(GameState::LoginScreen)
            .add_sub_state::<LoginPhase>()
            .add_plugins(TitleScreenPlugin);
        app.update();
        app
    }

    fn go<S: FreelyMutableState>(app: &mut App, state: S) {
        app.world_mut().resource_mut::<NextState<S>>().set(state);
        app.update();
    }

    fn count<C: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        world.query_filtered::<(), With<C>>().iter(world).count()
    }

    #[test]
    fn busy_while_waiting_on_the_site_or_the_server() {
        for sources in [
            (GameState::LoginScreen, Some(LoginPhase::LoadingCharacters)),
            (GameState::LoginScreen, Some(LoginPhase::EnteringGame)),
            (GameState::Connecting, None),
        ] {
            assert_eq!(LoginBusy::compute(sources), Some(LoginBusy), "{sources:?}");
        }
    }

    #[test]
    fn idle_while_a_dialog_is_up_or_outside_the_login() {
        for sources in [
            (GameState::LoginScreen, Some(LoginPhase::EnterGame)),
            (GameState::LoginScreen, Some(LoginPhase::CharacterList)),
            (GameState::LoginScreen, Some(LoginPhase::UpdateRequired)),
            (GameState::LoadingAssets, None),
            (GameState::InGame, None),
        ] {
            assert_eq!(LoginBusy::compute(sources), None, "{sources:?}");
        }
    }

    #[test]
    fn the_title_screen_spans_the_login_and_the_connect() {
        assert_eq!(
            TitleScreen::compute(GameState::LoginScreen),
            Some(TitleScreen)
        );
        assert_eq!(
            TitleScreen::compute(GameState::Connecting),
            Some(TitleScreen)
        );
        assert_eq!(TitleScreen::compute(GameState::LoadingAssets), None);
        assert_eq!(TitleScreen::compute(GameState::InGame), None);
    }

    #[test]
    fn the_backdrop_stays_through_the_connect_and_goes_in_game() {
        let mut app = title_app();
        assert_eq!(count::<LoginBackdrop>(&mut app), 1);

        go(&mut app, GameState::Connecting);
        assert_eq!(count::<LoginBackdrop>(&mut app), 1);

        go(&mut app, GameState::InGame);
        assert_eq!(count::<LoginBackdrop>(&mut app), 0);
    }

    #[test]
    fn a_failed_connect_keeps_a_single_backdrop() {
        let mut app = title_app();
        go(&mut app, GameState::Connecting);
        go(&mut app, GameState::LoginScreen);

        assert_eq!(count::<LoginBackdrop>(&mut app), 1);
    }

    #[test]
    fn the_head_dot_is_at_full_intensity() {
        assert_eq!(dot_intensity(0, 0.0), 1.0);
        let step = conf::SPINNER_PERIOD_SECS / conf::SPINNER_DOTS as f32;
        assert!((dot_intensity(3, step * 3.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn the_tail_fades_away_from_the_head() {
        let dots = conf::SPINNER_DOTS;
        for index in 2..dots {
            assert!(dot_intensity(index, 0.0) > dot_intensity(index - 1, 0.0));
        }
        assert_eq!(dot_intensity(1, 0.0), 0.0);
    }

    #[test]
    fn every_dot_changes_smoothly_and_stays_in_range() {
        let dt = 0.001;
        for frame in 0..3000 {
            let elapsed = frame as f32 * dt;
            for dot in 0..conf::SPINNER_DOTS {
                let now = dot_intensity(dot, elapsed);
                let next = dot_intensity(dot, elapsed + dt);
                assert!((0.0..=1.0).contains(&now), "dot {dot} at {elapsed}s: {now}");
                assert!(
                    (next - now).abs() < 0.02,
                    "dot {dot} jumped from {now} to {next} at {elapsed}s"
                );
            }
        }
    }

    #[test]
    fn the_spinner_exists_only_while_the_player_waits() {
        let mut app = title_app();
        assert_eq!(count::<LoginSpinner>(&mut app), 0);

        go(&mut app, LoginPhase::LoadingCharacters);
        assert_eq!(count::<LoginSpinner>(&mut app), 1);

        go(&mut app, LoginPhase::CharacterList);
        assert_eq!(count::<LoginSpinner>(&mut app), 0);

        go(&mut app, LoginPhase::EnteringGame);
        assert_eq!(count::<LoginSpinner>(&mut app), 1);

        go(&mut app, GameState::Connecting);
        assert_eq!(count::<LoginSpinner>(&mut app), 1);

        go(&mut app, GameState::InGame);
        assert_eq!(count::<LoginSpinner>(&mut app), 0);
    }

    #[test]
    fn a_failed_connect_takes_the_spinner_down() {
        let mut app = title_app();
        go(&mut app, LoginPhase::EnteringGame);
        go(&mut app, GameState::Connecting);
        go(&mut app, GameState::LoginScreen);

        assert_eq!(count::<LoginSpinner>(&mut app), 0);
        assert_eq!(count::<LoginBackdrop>(&mut app), 1);
    }
}
