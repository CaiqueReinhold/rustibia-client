use std::time::Duration;

use bevy::prelude::*;

use crate::{
    agent::{FacingDirection, WalkingDirection},
    core::SpellId,
    game_ui::EnterChatMode,
    map::Map,
    player::interaction::InteractionIntent,
    player::movement::{ChangePlayerDirection, MovePlayer},
    player::spells::CastSpellRequested,
    player::target::{CombatTarget, TargetSquare, refresh_target_square},
};

#[derive(Clone, Debug)]
pub enum PlayerAction {
    Move(WalkingDirection),
    ChangeDirection(FacingDirection),
    EnterChatMode,
    CastSpell(SpellId),
}

#[derive(Clone, Debug)]
pub struct KeyCombo {
    /// Any of these being just_pressed activates the combo
    pub keys: Vec<KeyCode>,
    /// All of these must be held (empty = no modifier required)
    pub modifiers: Vec<KeyCode>,
}

impl KeyCombo {
    pub fn single(key: KeyCode) -> Self {
        Self {
            keys: vec![key],
            modifiers: vec![],
        }
    }
    pub fn any(keys: Vec<KeyCode>) -> Self {
        Self {
            keys,
            modifiers: vec![],
        }
    }
    pub fn modified(modifier: KeyCode, key: KeyCode) -> Self {
        Self {
            keys: vec![key],
            modifiers: vec![modifier],
        }
    }
    pub fn matches(&self, key: &KeyCode, modifiers: &Vec<&KeyCode>) -> bool {
        self.modifiers.iter().all(|m| modifiers.contains(&m)) && self.keys.contains(key)
    }
}

#[derive(Resource)]
pub struct Keybinds {
    pub binds: Vec<(KeyCombo, PlayerAction)>,
}

impl Default for Keybinds {
    fn default() -> Self {
        use KeyCode::*;
        Self {
            binds: vec![
                // Shift combos must come before bare keys
                (
                    KeyCombo::modified(ShiftLeft, KeyW),
                    PlayerAction::ChangeDirection(FacingDirection::North),
                ),
                (
                    KeyCombo::modified(ShiftLeft, KeyD),
                    PlayerAction::ChangeDirection(FacingDirection::East),
                ),
                (
                    KeyCombo::modified(ShiftLeft, KeyS),
                    PlayerAction::ChangeDirection(FacingDirection::South),
                ),
                (
                    KeyCombo::modified(ShiftLeft, KeyA),
                    PlayerAction::ChangeDirection(FacingDirection::West),
                ),
                (
                    KeyCombo::any(vec![KeyW, ArrowUp]),
                    PlayerAction::Move(WalkingDirection::North),
                ),
                (
                    KeyCombo::any(vec![KeyD, ArrowRight]),
                    PlayerAction::Move(WalkingDirection::East),
                ),
                (
                    KeyCombo::any(vec![KeyS, ArrowDown]),
                    PlayerAction::Move(WalkingDirection::South),
                ),
                (
                    KeyCombo::any(vec![KeyA, ArrowLeft]),
                    PlayerAction::Move(WalkingDirection::West),
                ),
                (
                    KeyCombo::single(KeyQ),
                    PlayerAction::Move(WalkingDirection::NorthWest),
                ),
                (
                    KeyCombo::single(KeyE),
                    PlayerAction::Move(WalkingDirection::NorthEast),
                ),
                (
                    KeyCombo::single(KeyZ),
                    PlayerAction::Move(WalkingDirection::SouthWest),
                ),
                (
                    KeyCombo::single(KeyC),
                    PlayerAction::Move(WalkingDirection::SouthEast),
                ),
                (KeyCombo::single(Enter), PlayerAction::EnterChatMode),
                // Debug binding until the hotkey surface lands: Energy Strike,
                // which needs a combat target set.
                (KeyCombo::single(F1), PlayerAction::CastSpell(SpellId(3))),
                (KeyCombo::single(F2), PlayerAction::CastSpell(SpellId(1))),
                (KeyCombo::single(F3), PlayerAction::CastSpell(SpellId(2))),
            ],
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
) {
    // `chat_mode` covers the chat bar; `input_focus` covers every other text field,
    // such as the channels dialog's player-name entry. Without the second check,
    // typing a name there would also walk the character and fire keybinds.
    if chat_mode.active || input_focus.0.is_some() {
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

    if let Some(key) = pressed {
        let modifiers: Vec<&KeyCode> = keyboard.get_pressed().filter(is_modifier).collect();
        for (combo, action) in &keybinds.binds {
            if combo.matches(&key, &modifiers) {
                route_action(action, &mut commands);
                break;
            }
        }
    }
}

fn route_action(action: &PlayerAction, commands: &mut Commands) {
    match action {
        PlayerAction::Move(dir) => commands.trigger(MovePlayer { direction: *dir }),
        PlayerAction::ChangeDirection(dir) => {
            commands.trigger(ChangePlayerDirection { direction: *dir })
        }
        PlayerAction::EnterChatMode => {
            commands.trigger(EnterChatMode);
        }
        PlayerAction::CastSpell(spell_id) => {
            commands.trigger(CastSpellRequested {
                spell_id: *spell_id,
            });
        }
    }
}

pub fn cancel_targeting_on_escape(
    mut commands: Commands,
    mut mode: ResMut<crate::player::InteractionMode>,
    mut combat_target: ResMut<CombatTarget>,
    map: Res<Map>,
    square_q: Query<Entity, With<TargetSquare>>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
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
        *world.resource_mut::<InteractionMode>() = InteractionMode::Targeting {
            source: ItemPlacement::Inventory {
                slot: InventorySlot::Head,
            },
            source_item_id: ItemId(1),
        };
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
}
