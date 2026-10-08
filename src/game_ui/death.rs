use bevy::input_focus::InputFocus;
use bevy::prelude::*;

use crate::core::{EndGameSession, GameState, SessionEndReason, SessionEnding};
use crate::game_ui::GameUiAssets;
use crate::game_ui::chat::events::ExitChatMode;
use crate::game_ui::modal::{DialogButtonPressed, ModalDialog, ModalOrder};
use crate::network::events::PlayerDied;

const DEATH_TEXT: &str = "Alas! Brave adventurer, you have met a sad fate. \
But do not despair, for the gods will bring you back \
into this world in exchange for a small sacrifice. \
\n\
Simply click on Ok to resume your journeys!";

#[derive(Component)]
pub(super) struct DeathModal;

pub(super) fn on_player_died(
    _: On<PlayerDied>,
    mut commands: Commands,
    state: Res<State<GameState>>,
    existing: Query<(), With<DeathModal>>,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    mut input_focus: ResMut<InputFocus>,
) {
    if *state.get() != GameState::InGame || !existing.is_empty() {
        return;
    }
    let root = ModalDialog::message(
        "You're dead",
        DEATH_TEXT,
        &mut commands,
        &ui_assets,
        &mut order,
    );
    commands.entity(root).insert(DeathModal);
    commands.insert_resource(SessionEnding);
    commands.trigger(ExitChatMode);
    input_focus.clear();
}

pub(super) fn on_dismiss(
    event: On<DialogButtonPressed>,
    modals: Query<(), With<DeathModal>>,
    mut commands: Commands,
) {
    if !modals.contains(event.dialog) {
        return;
    }
    commands.trigger(EndGameSession {
        reason: SessionEndReason::Died,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_ui::assets::{UiButtons, UiInventory, UiWindow};
    use crate::game_ui::disconnect::{self, DisconnectedModal};
    use crate::game_ui::modal::DialogButtonId;
    use crate::network::events::ConnectionLost;

    #[derive(Resource, Default)]
    struct EndedWith(Option<SessionEndReason>);

    fn in_game_world() -> World {
        let mut world = World::new();
        world.insert_resource(State::new(GameState::InGame));
        world.insert_resource(GameUiAssets {
            font: Handle::default(),
            name_font: Handle::default(),
            window: UiWindow::default(),
            inventory: UiInventory::default(),
            background_dark: Handle::default(),
            background_light: Handle::default(),
            bar_overlay: Handle::default(),
            title_background: Handle::default(),
            buttons: UiButtons::default(),
        });
        world.init_resource::<ModalOrder>();
        world.init_resource::<InputFocus>();
        world.init_resource::<EndedWith>();
        world.add_observer(on_player_died);
        world.add_observer(on_dismiss);
        world.add_observer(disconnect::on_connection_lost);
        world.add_observer(|event: On<EndGameSession>, mut ended: ResMut<EndedWith>| {
            ended.0 = Some(event.reason);
        });
        world
    }

    fn death_modals(world: &mut World) -> Vec<Entity> {
        world
            .query_filtered::<Entity, With<DeathModal>>()
            .iter(world)
            .collect()
    }

    #[test]
    fn dying_opens_the_notice_and_gates_input() {
        let mut world = in_game_world();

        world.trigger(PlayerDied);
        world.flush();

        assert_eq!(death_modals(&mut world).len(), 1);
        assert!(world.contains_resource::<SessionEnding>());
    }

    #[test]
    fn a_second_death_does_not_stack_a_second_notice() {
        let mut world = in_game_world();

        world.trigger(PlayerDied);
        world.flush();
        world.trigger(PlayerDied);
        world.flush();

        assert_eq!(death_modals(&mut world).len(), 1);
    }

    #[test]
    fn ok_ends_the_session_as_a_death() {
        let mut world = in_game_world();
        world.trigger(PlayerDied);
        world.flush();
        let modal = death_modals(&mut world)[0];

        world.trigger(DialogButtonPressed {
            dialog: modal,
            button: DialogButtonId::Ok,
        });
        world.flush();

        assert_eq!(
            world.resource::<EndedWith>().0,
            Some(SessionEndReason::Died)
        );
    }

    /// The server closes the socket right after `PlayerDied`; that close must not
    /// put "Connection Lost" over the death notice.
    #[test]
    fn the_drop_after_a_death_shows_no_disconnect_notice() {
        let mut world = in_game_world();
        world.trigger(PlayerDied);
        world.flush();

        world.trigger(ConnectionLost);
        world.flush();

        let disconnect_modals = world
            .query_filtered::<(), With<DisconnectedModal>>()
            .iter(&world)
            .count();
        assert_eq!(disconnect_modals, 0);
        assert_eq!(world.resource::<EndedWith>().0, None);
    }
}
