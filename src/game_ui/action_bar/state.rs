use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{core::SpellId, items::ItemId, player::Hotkey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Aim {
    Yourself,
    Target,
    Crosshair,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotAction {
    /// `aim` is `Some` only for an aimable spell.
    Spell { id: SpellId, aim: Option<Aim> },
    /// `aim` is `Some` for a use-with, `None` for a plain use.
    Item { item_id: ItemId, aim: Option<Aim> },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionSlot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<SlotAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<Hotkey>,
}

impl ActionSlot {
    pub fn is_empty(&self) -> bool {
        self.action.is_none() && self.hotkey.is_none()
    }
}

/// Slots by index, sparse. `dirty` means a change not yet written to disk.
#[derive(Resource, Debug, Default)]
pub struct ActionBar {
    slots: BTreeMap<u16, ActionSlot>,
    dirty: bool,
    spells_checked: bool,
}

impl ActionBar {
    pub fn from_slots(slots: BTreeMap<u16, ActionSlot>) -> Self {
        Self { slots, ..default() }
    }

    pub fn slots(&self) -> &BTreeMap<u16, ActionSlot> {
        &self.slots
    }

    pub fn slot(&self, index: u16) -> ActionSlot {
        self.slots.get(&index).copied().unwrap_or_default()
    }

    pub fn slot_with_hotkey(&self, hotkey: &Hotkey) -> Option<u16> {
        self.slots
            .iter()
            .find(|(_, slot)| slot.hotkey.as_ref() == Some(hotkey))
            .map(|(index, _)| *index)
    }

    pub fn set_action(&mut self, index: u16, action: SlotAction) {
        self.slots.entry(index).or_default().action = Some(action);
        self.dirty = true;
    }

    pub fn set_hotkey(&mut self, index: u16, hotkey: Hotkey) {
        for slot in self.slots.values_mut() {
            if slot.hotkey == Some(hotkey) {
                slot.hotkey = None;
            }
        }
        self.slots.entry(index).or_default().hotkey = Some(hotkey);
        self.slots.retain(|_, slot| !slot.is_empty());
        self.dirty = true;
    }

    pub fn clear_hotkey(&mut self, index: u16) {
        if let Some(slot) = self.slots.get_mut(&index) {
            slot.hotkey = None;
        }
        self.slots.retain(|_, slot| !slot.is_empty());
        self.dirty = true;
    }

    pub fn clear_slot(&mut self, index: u16) {
        self.slots.remove(&index);
        self.dirty = true;
    }

    /// Clears every slot, hotkey included, whose action `keep` rejects, and returns their indices.
    pub fn retain_actions(&mut self, keep: impl Fn(&SlotAction) -> bool) -> Vec<u16> {
        let cleared: Vec<u16> = self
            .slots
            .iter()
            .filter(|(_, slot)| slot.action.as_ref().is_some_and(|action| !keep(action)))
            .map(|(index, _)| *index)
            .collect();
        for index in &cleared {
            self.slots.remove(index);
        }
        if !cleared.is_empty() {
            self.dirty = true;
        }
        cleared
    }

    pub(super) fn take_dirty(&mut self) -> bool {
        std::mem::take(&mut self.dirty)
    }

    pub(super) fn spells_checked(&self) -> bool {
        self.spells_checked
    }

    pub(super) fn mark_spells_checked(&mut self) {
        self.spells_checked = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAL: SlotAction = SlotAction::Spell {
        id: SpellId(1),
        aim: None,
    };

    #[test]
    fn an_action_and_a_hotkey_are_set_independently() {
        let mut bar = ActionBar::default();

        bar.set_action(3, HEAL);
        bar.set_hotkey(3, Hotkey::plain(KeyCode::F1));
        bar.set_action(
            3,
            SlotAction::Item {
                item_id: ItemId(266),
                aim: None,
            },
        );

        assert_eq!(bar.slot(3).hotkey, Some(Hotkey::plain(KeyCode::F1)));
        assert!(matches!(bar.slot(3).action, Some(SlotAction::Item { .. })));
        assert!(bar.take_dirty());
        assert!(!bar.take_dirty());
    }

    #[test]
    fn a_hotkey_moves_rather_than_being_shared() {
        let mut bar = ActionBar::default();
        bar.set_action(0, HEAL);
        bar.set_hotkey(0, Hotkey::plain(KeyCode::F1));

        bar.set_hotkey(5, Hotkey::plain(KeyCode::F1));

        assert_eq!(bar.slot(0).hotkey, None);
        assert_eq!(bar.slot_with_hotkey(&Hotkey::plain(KeyCode::F1)), Some(5));
    }

    #[test]
    fn a_slot_left_with_nothing_has_no_entry() {
        let mut bar = ActionBar::default();
        bar.set_hotkey(0, Hotkey::plain(KeyCode::F1));
        bar.set_hotkey(1, Hotkey::plain(KeyCode::F1));
        assert!(!bar.slots().contains_key(&0));

        bar.clear_hotkey(1);
        assert!(bar.slots().is_empty());
    }

    #[test]
    fn a_hotkey_without_an_action_is_kept() {
        let mut bar = ActionBar::default();

        bar.set_hotkey(2, Hotkey::plain(KeyCode::F2));

        assert_eq!(bar.slot(2).action, None);
        assert!(bar.slots().contains_key(&2));
    }

    #[test]
    fn clearing_a_slot_removes_its_action_and_hotkey() {
        let mut bar = ActionBar::default();
        bar.set_action(4, HEAL);
        bar.set_hotkey(4, Hotkey::plain(KeyCode::F4));

        bar.clear_slot(4);

        assert!(bar.slot(4).is_empty());
    }

    #[test]
    fn a_rejected_action_clears_its_whole_slot() {
        let mut bar = ActionBar::default();
        bar.set_action(0, HEAL);
        bar.set_hotkey(0, Hotkey::plain(KeyCode::F1));
        bar.set_action(
            1,
            SlotAction::Item {
                item_id: ItemId(266),
                aim: None,
            },
        );
        bar.set_hotkey(2, Hotkey::plain(KeyCode::F2));
        bar.take_dirty();

        let cleared = bar.retain_actions(|action| matches!(action, SlotAction::Item { .. }));

        assert_eq!(cleared, vec![0]);
        assert!(bar.slot(0).is_empty());
        assert!(!bar.slot(1).is_empty());
        assert!(!bar.slot(2).is_empty());
        assert!(bar.take_dirty());
    }
}
