use std::collections::HashMap;
use std::time::Duration;

use bevy::prelude::*;

use crate::{
    agent::{FacingDirection, WalkingDirection},
    game_ui::{ActionSlotActivated, ContextMenuRoot, EnterChatMode, ModalDialogRoot},
    map::Map,
    player::Hotkey,
    player::interaction::InteractionIntent,
    player::movement::{ChangePlayerDirection, MovePlayer},
    player::target::{CombatTarget, TargetSquare, refresh_target_square},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerAction {
    Move(WalkingDirection),
    ChangeDirection(FacingDirection),
    EnterChatMode,
    ActivateActionSlot(u16),
}

#[derive(Resource)]
pub struct Keybinds {
    binds: HashMap<Hotkey, PlayerAction>,
}

impl Keybinds {
    pub fn action(&self, hotkey: &Hotkey) -> Option<PlayerAction> {
        self.binds.get(hotkey).copied()
    }

    pub fn slot_hotkey(&self, slot: u16) -> Option<Hotkey> {
        self.slot_binds()
            .find(|(bound, _)| *bound == slot)
            .map(|(_, hotkey)| hotkey)
    }

    pub fn slot_binds(&self) -> impl Iterator<Item = (u16, Hotkey)> + '_ {
        self.binds
            .iter()
            .filter_map(|(hotkey, action)| match action {
                PlayerAction::ActivateActionSlot(slot) => Some((*slot, *hotkey)),
                _ => None,
            })
    }

    /// Moves `hotkey` onto `slot`, replacing `slot`'s own hotkey and taking it from any other slot.
    /// `false`, with nothing changed, when a bind other than a slot holds `hotkey`.
    pub fn bind_slot(&mut self, slot: u16, hotkey: Hotkey) -> bool {
        match self.action(&hotkey) {
            None | Some(PlayerAction::ActivateActionSlot(_)) => {}
            Some(_) => return false,
        }
        self.unbind_slot(slot);
        self.binds
            .insert(hotkey, PlayerAction::ActivateActionSlot(slot));
        true
    }

    pub fn unbind_slot(&mut self, slot: u16) {
        self.binds
            .retain(|_, action| *action != PlayerAction::ActivateActionSlot(slot));
    }

    pub fn unbind_all_slots(&mut self) {
        self.binds
            .retain(|_, action| !matches!(action, PlayerAction::ActivateActionSlot(_)));
    }
}

impl Default for Keybinds {
    fn default() -> Self {
        use FacingDirection as Face;
        use KeyCode::*;
        use PlayerAction::{ChangeDirection, Move};
        use WalkingDirection as Walk;
        let shift = |key| Hotkey::new(key, false, true, false);
        Self {
            binds: HashMap::from([
                (shift(KeyW), ChangeDirection(Face::North)),
                (shift(KeyD), ChangeDirection(Face::East)),
                (shift(KeyS), ChangeDirection(Face::South)),
                (shift(KeyA), ChangeDirection(Face::West)),
                (Hotkey::plain(KeyW), Move(Walk::North)),
                (Hotkey::plain(ArrowUp), Move(Walk::North)),
                (Hotkey::plain(KeyD), Move(Walk::East)),
                (Hotkey::plain(ArrowRight), Move(Walk::East)),
                (Hotkey::plain(KeyS), Move(Walk::South)),
                (Hotkey::plain(ArrowDown), Move(Walk::South)),
                (Hotkey::plain(KeyA), Move(Walk::West)),
                (Hotkey::plain(ArrowLeft), Move(Walk::West)),
                (Hotkey::plain(KeyQ), Move(Walk::NorthWest)),
                (Hotkey::plain(KeyE), Move(Walk::NorthEast)),
                (Hotkey::plain(KeyZ), Move(Walk::SouthWest)),
                (Hotkey::plain(KeyC), Move(Walk::SouthEast)),
                (Hotkey::plain(Enter), PlayerAction::EnterChatMode),
            ]),
        }
    }
}

#[derive(Resource, Debug)]
pub struct KeyRepeatState {
    pressed_key: Option<KeyCode>,
    timer: Timer,
}

pub fn init_repeat_state(mut commands: Commands) {
    commands.insert_resource(KeyRepeatState {
        pressed_key: None,
        timer: Timer::new(Duration::from_millis(200), TimerMode::Repeating),
    });
}

pub fn read_player_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybinds: Res<Keybinds>,
    mut key_repeat: ResMut<KeyRepeatState>,
    mut commands: Commands,
    time: Res<Time>,
    chat_mode: Res<crate::game_ui::ChatMode>,
    input_focus: Res<bevy::input_focus::InputFocus>,
    modals: Query<(), With<ModalDialogRoot>>,
) {
    // `chat_mode` covers the chat bar; `input_focus` covers every other text field,
    // such as the channels dialog's player-name entry. Without the second check,
    // typing a name there would also walk the character and fire keybinds.
    if chat_mode.active || input_focus.0.is_some() || !modals.is_empty() {
        // Reset key-repeat so movement doesn't auto-resume when typing ends.
        key_repeat.pressed_key = None;
        key_repeat.timer.reset();
        return;
    }
    key_repeat.timer.tick(time.delta());
    let is_modifier = |k: &&KeyCode| {
        matches!(
            k,
            KeyCode::AltLeft
                | KeyCode::AltRight
                | KeyCode::ShiftLeft
                | KeyCode::ShiftRight
                | KeyCode::ControlLeft
                | KeyCode::ControlRight
        )
    };
    let just_pressed_key = keyboard.get_just_pressed().find(|k| !is_modifier(k));

    let mut pressed = None;
    if let Some(key) = just_pressed_key {
        pressed = Some(*key);
    } else if let Some(key) = key_repeat.pressed_key
        && keyboard.pressed(key)
    {
        pressed = Some(key);
    }

    if key_repeat.pressed_key.is_some()
        && key_repeat.pressed_key == pressed
        && !key_repeat.timer.just_finished()
    {
        return;
    }

    if key_repeat.pressed_key != pressed {
        key_repeat.pressed_key = pressed;
        key_repeat.timer.reset();
    }

    if let Some(key) = pressed
        && let Some(hotkey) = Hotkey::from_input(key, &keyboard)
        && let Some(action) = keybinds.action(&hotkey)
    {
        route_action(action, &mut commands);
    }
}

fn route_action(action: PlayerAction, commands: &mut Commands) {
    match action {
        PlayerAction::Move(direction) => commands.trigger(MovePlayer { direction }),
        PlayerAction::ChangeDirection(direction) => {
            commands.trigger(ChangePlayerDirection { direction })
        }
        PlayerAction::EnterChatMode => commands.trigger(EnterChatMode),
        PlayerAction::ActivateActionSlot(slot) => commands.trigger(ActionSlotActivated { slot }),
    }
}

pub fn cancel_targeting_on_escape(
    mut commands: Commands,
    mut mode: ResMut<crate::player::InteractionMode>,
    mut combat_target: ResMut<CombatTarget>,
    map: Res<Map>,
    square_q: Query<Entity, With<TargetSquare>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    modals: Query<(), With<ModalDialogRoot>>,
    menus: Query<(), With<ContextMenuRoot>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) || !modals.is_empty() || !menus.is_empty() {
        return;
    }

    // The use-with crosshair is the more modal state and wins.
    if mode.is_targeting() {
        *mode = crate::player::InteractionMode::Idle;
        return;
    }

    if combat_target.target.is_some() {
        let seq = combat_target.clear_locally();
        refresh_target_square(&mut commands, &combat_target, &map, &square_q);
        commands.trigger(InteractionIntent::SetTarget(None, seq));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentId;
    use crate::items::ItemId;
    use crate::items::{InventorySlot, ItemPlacement};
    use crate::player::InteractionMode;
    use crate::player::TargetingSource;
    use bevy::ecs::system::RunSystemOnce;

    fn seeded_world() -> World {
        let mut world = World::new();
        world.init_resource::<InteractionMode>();
        world.init_resource::<CombatTarget>();
        world.insert_resource(Map::default());
        world
    }

    fn press_escape(world: &mut World) {
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Escape);
        world.insert_resource(keyboard);
    }

    /// Escape must not clear a combat target while the use-with crosshair is up —
    /// the player is cancelling the crosshair, not their target.
    #[test]
    fn escape_cancels_the_crosshair_before_the_target() {
        let mut world = seeded_world();
        let mut target = CombatTarget::default();
        target.apply_click(AgentId(7));
        world.insert_resource(target);
        *world.resource_mut::<InteractionMode>() =
            InteractionMode::Targeting(TargetingSource::Item {
                placement: ItemPlacement::Inventory {
                    slot: InventorySlot::Head,
                },
                item_id: ItemId(1),
            });
        press_escape(&mut world);

        world.run_system_once(cancel_targeting_on_escape).unwrap();

        assert!(matches!(
            *world.resource::<InteractionMode>(),
            InteractionMode::Idle
        ));
        assert_eq!(world.resource::<CombatTarget>().target, Some(AgentId(7)));
    }

    #[test]
    fn escape_clears_the_target_when_no_crosshair_is_up() {
        let mut world = seeded_world();
        let mut target = CombatTarget::default();
        target.apply_click(AgentId(7));
        world.insert_resource(target);
        press_escape(&mut world);

        world.run_system_once(cancel_targeting_on_escape).unwrap();

        assert_eq!(world.resource::<CombatTarget>().target, None);
    }

    #[test]
    fn a_slot_takes_a_free_combo_but_never_a_built_in_one() {
        let mut binds = Keybinds::default();
        let ctrl_w = Hotkey::new(KeyCode::KeyW, true, false, false);

        assert!(!binds.bind_slot(0, Hotkey::plain(KeyCode::KeyW)));
        assert!(binds.bind_slot(0, ctrl_w));
        assert!(binds.bind_slot(0, Hotkey::plain(KeyCode::F1)));
        assert!(binds.bind_slot(3, Hotkey::plain(KeyCode::F1)));

        assert_eq!(binds.slot_hotkey(0), None);
        assert_eq!(binds.slot_hotkey(3), Some(Hotkey::plain(KeyCode::F1)));
        assert_eq!(binds.action(&ctrl_w), None);
        assert_eq!(
            binds.action(&Hotkey::plain(KeyCode::KeyW)),
            Some(PlayerAction::Move(WalkingDirection::North))
        );
    }

    #[test]
    fn escape_cancels_every_kind_of_crosshair() {
        use crate::core::SpellId;

        for source in [
            TargetingSource::Spell(SpellId(4)),
            TargetingSource::AssignObject { slot: 2 },
        ] {
            let mut world = seeded_world();
            *world.resource_mut::<InteractionMode>() = InteractionMode::Targeting(source);
            press_escape(&mut world);

            world.run_system_once(cancel_targeting_on_escape).unwrap();

            assert!(matches!(
                *world.resource::<InteractionMode>(),
                InteractionMode::Idle
            ));
        }
    }

    #[test]
    fn escape_leaves_the_target_alone_while_a_modal_or_menu_is_open() {
        use crate::game_ui::{ContextMenuRoot, ModalDialogRoot};

        let overlays: [fn(&mut World); 2] = [
            |world| {
                world.spawn(ModalDialogRoot::for_test(0));
            },
            |world| {
                world.spawn(ContextMenuRoot);
            },
        ];
        for spawn_overlay in overlays {
            let mut world = seeded_world();
            let mut target = CombatTarget::default();
            target.apply_click(AgentId(7));
            world.insert_resource(target);
            spawn_overlay(&mut world);
            press_escape(&mut world);

            world.run_system_once(cancel_targeting_on_escape).unwrap();

            assert_eq!(world.resource::<CombatTarget>().target, Some(AgentId(7)));
        }
    }

    #[derive(Resource, Default)]
    struct Fired {
        walks: usize,
        slots: Vec<u16>,
    }

    fn input_world(pressed: &[KeyCode]) -> World {
        let mut world = World::new();
        let mut keyboard = ButtonInput::<KeyCode>::default();
        for key in pressed {
            keyboard.press(*key);
        }
        world.insert_resource(keyboard);
        world.init_resource::<Keybinds>();
        world.init_resource::<Time>();
        world.init_resource::<crate::game_ui::ChatMode>();
        world.init_resource::<bevy::input_focus::InputFocus>();
        world.init_resource::<Fired>();
        world.run_system_once(init_repeat_state).unwrap();
        world.add_observer(|_: On<MovePlayer>, mut fired: ResMut<Fired>| fired.walks += 1);
        world.add_observer(|event: On<ActionSlotActivated>, mut fired: ResMut<Fired>| {
            fired.slots.push(event.slot)
        });
        world
    }

    #[test]
    fn a_hotkey_fires_its_slot() {
        let mut world = input_world(&[KeyCode::F1]);
        world
            .resource_mut::<Keybinds>()
            .bind_slot(3, Hotkey::plain(KeyCode::F1));

        world.run_system_once(read_player_input).unwrap();

        assert_eq!(world.resource::<Fired>().slots, vec![3]);
    }

    #[test]
    fn a_bind_needs_exactly_its_modifiers() {
        let mut world = input_world(&[KeyCode::ControlLeft, KeyCode::F1]);
        world
            .resource_mut::<Keybinds>()
            .bind_slot(3, Hotkey::plain(KeyCode::F1));
        world.run_system_once(read_player_input).unwrap();
        assert!(world.resource::<Fired>().slots.is_empty());

        let mut world = input_world(&[KeyCode::ControlLeft, KeyCode::KeyW]);
        world.run_system_once(read_player_input).unwrap();
        assert_eq!(world.resource::<Fired>().walks, 0);
    }

    #[test]
    fn a_modified_combo_fires_its_slot_and_not_the_bare_built_in() {
        let mut world = input_world(&[KeyCode::ControlLeft, KeyCode::KeyW]);
        world
            .resource_mut::<Keybinds>()
            .bind_slot(2, Hotkey::new(KeyCode::KeyW, true, false, false));

        world.run_system_once(read_player_input).unwrap();

        let fired = world.resource::<Fired>();
        assert_eq!(fired.slots, vec![2]);
        assert_eq!(fired.walks, 0);
    }

    #[test]
    fn either_shift_turns_rather_than_walks() {
        for shift in [KeyCode::ShiftLeft, KeyCode::ShiftRight] {
            let mut world = input_world(&[shift, KeyCode::KeyW]);
            world.init_resource::<Turned>();
            world.add_observer(|_: On<ChangePlayerDirection>, mut turned: ResMut<Turned>| {
                turned.0 += 1
            });

            world.run_system_once(read_player_input).unwrap();

            assert_eq!(world.resource::<Fired>().walks, 0, "{shift:?}");
            assert_eq!(world.resource::<Turned>().0, 1, "{shift:?}");
        }
    }

    #[derive(Resource, Default)]
    struct Turned(usize);

    #[test]
    fn no_key_does_anything_while_a_modal_is_open() {
        use crate::game_ui::ModalDialogRoot;

        let mut open = input_world(&[KeyCode::KeyW]);
        open.spawn(ModalDialogRoot::for_test(0));
        open.run_system_once(read_player_input).unwrap();
        assert_eq!(open.resource::<Fired>().walks, 0);

        let mut closed = input_world(&[KeyCode::KeyW]);
        closed.run_system_once(read_player_input).unwrap();
        assert_eq!(closed.resource::<Fired>().walks, 1);
    }
}
