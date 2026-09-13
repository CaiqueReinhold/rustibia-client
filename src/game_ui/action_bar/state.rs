use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    core::SpellId,
    items::ItemId,
    player::{Hotkey, Keybinds},
};

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

/// Each slot's action, sparse. A slot's hotkey is its bind in `Keybinds`.
#[derive(Resource, Debug, Default)]
pub struct ActionBar {
    actions: BTreeMap<u16, SlotAction>,
    spells_checked: bool,
}

impl ActionBar {
    pub fn from_actions(actions: BTreeMap<u16, SlotAction>) -> Self {
        Self {
            actions,
            ..default()
        }
    }

    pub fn action(&self, index: u16) -> Option<SlotAction> {
        self.actions.get(&index).copied()
    }

    pub fn slot(&self, keybinds: &Keybinds, index: u16) -> ActionSlot {
        ActionSlot {
            action: self.action(index),
            hotkey: keybinds.slot_hotkey(index),
        }
    }

    /// Every slot with an action or a hotkey.
    pub fn slots(&self, keybinds: &Keybinds) -> BTreeMap<u16, ActionSlot> {
        let mut slots: BTreeMap<u16, ActionSlot> = self
            .actions
            .iter()
            .map(|(index, action)| {
                let slot = ActionSlot {
                    action: Some(*action),
                    hotkey: None,
                };
                (*index, slot)
            })
            .collect();
        for (index, hotkey) in keybinds.slot_binds() {
            slots.entry(index).or_default().hotkey = Some(hotkey);
        }
        slots
    }

    pub fn set_action(&mut self, index: u16, action: SlotAction) {
        self.actions.insert(index, action);
    }

    pub fn clear_action(&mut self, index: u16) {
        self.actions.remove(&index);
    }

    /// Drops every action `keep` rejects and returns the slots it was in.
    pub fn retain_actions(&mut self, keep: impl Fn(&SlotAction) -> bool) -> Vec<u16> {
        let cleared: Vec<u16> = self
            .actions
            .iter()
            .filter(|(_, action)| !keep(action))
            .map(|(index, _)| *index)
            .collect();
        for index in &cleared {
            self.actions.remove(index);
        }
        cleared
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
    fn a_slot_is_its_action_and_its_bind() {
        let mut bar = ActionBar::default();
        let mut keybinds = Keybinds::default();

        bar.set_action(3, HEAL);
        keybinds.bind_slot(3, Hotkey::plain(KeyCode::F1));
        keybinds.bind_slot(5, Hotkey::plain(KeyCode::F2));

        assert_eq!(
            bar.slots(&keybinds),
            BTreeMap::from([
                (
                    3,
                    ActionSlot {
                        action: Some(HEAL),
                        hotkey: Some(Hotkey::plain(KeyCode::F1)),
                    }
                ),
                (
                    5,
                    ActionSlot {
                        action: None,
                        hotkey: Some(Hotkey::plain(KeyCode::F2)),
                    }
                ),
            ])
        );
        assert!(bar.slot(&keybinds, 4).is_empty());
    }

    #[test]
    fn a_rejected_action_is_dropped_and_its_slot_reported() {
        let mut bar = ActionBar::default();
        bar.set_action(0, HEAL);
        bar.set_action(
            1,
            SlotAction::Item {
                item_id: ItemId(266),
                aim: None,
            },
        );

        let cleared = bar.retain_actions(|action| matches!(action, SlotAction::Item { .. }));

        assert_eq!(cleared, vec![0]);
        assert_eq!(bar.action(0), None);
        assert!(bar.action(1).is_some());
    }
}
