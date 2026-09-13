use bevy::prelude::*;

use crate::game_ui::{ContextMenu, ContextMenuEntry, ContextMenuPicked, GameUiAssets};
use crate::player::{InteractionMode, Keybinds, TargetingSource};

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
    keybinds: Res<Keybinds>,
) {
    let menu = ContextMenu::new(slot_menu_entries(&bar.slot(&keybinds, event.slot))).spawn(
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
    mut keybinds: ResMut<Keybinds>,
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
        CLEAR => {
            bar.clear_action(slot);
            keybinds.unbind_slot(slot);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::super::state::SlotAction;
    use super::*;
    use crate::core::SpellId;
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
        let mut keybinds = Keybinds::default();
        keybinds.bind_slot(5, Hotkey::plain(KeyCode::F5));
        world.insert_resource(keybinds);
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

        let keybinds = world.resource::<Keybinds>();
        assert!(world.resource::<ActionBar>().slot(keybinds, 5).is_empty());
    }

    /// The consts index into what `slot_menu_entries` returns, and nothing but this ties the two
    /// orderings together — reorder one and a click routes to the wrong action, silently.
    #[test]
    fn every_entry_sits_at_the_index_its_const_names() {
        let entries = slot_menu_entries(&ActionSlot::default());

        assert_eq!(entries[ASSIGN_SPELL].label, "Assign Spell");
        assert_eq!(entries[ASSIGN_ITEM].label, "Assign Item");
        assert_eq!(entries[ASSIGN_HOTKEY].label, "Assign Hotkey");
        assert_eq!(entries[CLEAR].label, "Clear");
    }

    #[derive(Resource, Default)]
    struct Opened {
        spell: Vec<u16>,
        hotkey: Vec<u16>,
    }

    fn pick(index: usize) -> World {
        let (mut world, menu) = a_world_with_a_slot_menu();
        world.init_resource::<Opened>();
        world.add_observer(
            |event: On<OpenAssignSpellDialog>, mut opened: ResMut<Opened>| {
                opened.spell.push(event.slot)
            },
        );
        world.add_observer(|event: On<OpenHotkeyDialog>, mut opened: ResMut<Opened>| {
            opened.hotkey.push(event.slot)
        });

        world.trigger(ContextMenuPicked { menu, index });
        world.flush();
        world
    }

    #[test]
    fn assign_spell_opens_the_spell_modal_for_this_slot() {
        let world = pick(ASSIGN_SPELL);

        assert_eq!(world.resource::<Opened>().spell, vec![5]);
        assert!(world.resource::<Opened>().hotkey.is_empty());
    }

    #[test]
    fn assign_hotkey_opens_the_hotkey_modal_for_this_slot() {
        let world = pick(ASSIGN_HOTKEY);

        assert_eq!(world.resource::<Opened>().hotkey, vec![5]);
        assert!(world.resource::<Opened>().spell.is_empty());
    }
}
