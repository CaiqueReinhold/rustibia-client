use bevy::prelude::*;

use crate::game_ui::{ContextMenu, ContextMenuEntry, ContextMenuPicked, GameUiAssets};
use crate::player::{InteractionMode, TargetingSource};

use super::hotkey_dialog::OpenHotkeyDialog;
use super::spell_dialog::OpenAssignSpellDialog;
use super::state::{ActionBar, ActionSlot};

const ASSIGN_SPELL: usize = 0;
const ASSIGN_ITEM: usize = 1;
const ASSIGN_HOTKEY: usize = 2;
const CLEAR: usize = 3;

#[derive(Event, Debug, Clone, Copy)]
pub struct OpenSlotMenu {
    pub slot: u16,
    pub at: Vec2,
}

#[derive(Component, Debug)]
pub(super) struct SlotMenu {
    slot: u16,
}

pub fn slot_menu_entries(slot: &ActionSlot) -> [ContextMenuEntry; 4] {
    [
        ContextMenuEntry::new("Assign Spell", true),
        ContextMenuEntry::new("Assign Item", true),
        ContextMenuEntry::new("Assign Hotkey", true),
        ContextMenuEntry::new("Clear", !slot.is_empty()),
    ]
}

pub(super) fn on_open_slot_menu(
    event: On<OpenSlotMenu>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    bar: Res<ActionBar>,
) {
    let menu = ContextMenu::new(slot_menu_entries(&bar.slot(event.slot))).spawn(
        &mut commands,
        &ui_assets,
        event.at,
    );
    commands.entity(menu).insert(SlotMenu { slot: event.slot });
}

pub(super) fn on_slot_menu_picked(
    event: On<ContextMenuPicked>,
    mut commands: Commands,
    menus: Query<&SlotMenu>,
    mut bar: ResMut<ActionBar>,
    mut mode: ResMut<InteractionMode>,
) {
    let Ok(menu) = menus.get(event.menu) else {
        return;
    };
    let slot = menu.slot;
    match event.index {
        ASSIGN_SPELL => commands.trigger(OpenAssignSpellDialog { slot }),
        ASSIGN_ITEM => *mode = InteractionMode::Targeting(TargetingSource::AssignObject { slot }),
        ASSIGN_HOTKEY => commands.trigger(OpenHotkeyDialog { slot }),
        CLEAR => bar.clear_slot(slot),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SpellId;
    use crate::game_ui::action_bar::SlotAction;
    use crate::player::Hotkey;

    fn enabled(slot: &ActionSlot) -> Vec<bool> {
        slot_menu_entries(slot)
            .iter()
            .map(|entry| entry.enabled)
            .collect()
    }

    #[test]
    fn an_empty_slot_can_be_given_anything_but_has_nothing_to_clear() {
        assert_eq!(
            enabled(&ActionSlot::default()),
            vec![true, true, true, false]
        );
    }

    #[test]
    fn a_hotkey_alone_is_something_to_clear() {
        let slot = ActionSlot {
            action: None,
            hotkey: Some(Hotkey::plain(KeyCode::F1)),
        };

        assert_eq!(enabled(&slot), vec![true, true, true, true]);
    }

    fn a_world_with_a_slot_menu() -> (World, Entity) {
        let mut world = World::new();
        let mut bar = ActionBar::default();
        bar.set_action(
            5,
            SlotAction::Spell {
                id: SpellId(1),
                aim: None,
            },
        );
        world.insert_resource(bar);
        world.init_resource::<InteractionMode>();
        world.add_observer(on_slot_menu_picked);
        let menu = world.spawn(SlotMenu { slot: 5 }).id();
        (world, menu)
    }

    #[test]
    fn assign_item_enters_the_crosshair_for_this_slot() {
        let (mut world, menu) = a_world_with_a_slot_menu();

        world.trigger(ContextMenuPicked {
            menu,
            index: ASSIGN_ITEM,
        });
        world.flush();

        assert!(matches!(
            *world.resource::<InteractionMode>(),
            InteractionMode::Targeting(TargetingSource::AssignObject { slot: 5 })
        ));
    }

    #[test]
    fn clear_empties_the_slot() {
        let (mut world, menu) = a_world_with_a_slot_menu();

        world.trigger(ContextMenuPicked { menu, index: CLEAR });
        world.flush();

        assert!(world.resource::<ActionBar>().slot(5).is_empty());
    }
}
