use bevy::prelude::*;

use crate::core::TextMessageType;
use crate::items::{ItemConfig, ItemFlag};
use crate::network::events::ShowTextMessage;
use crate::player::ObjectPicked;

use super::state::{ActionBar, SlotAction};
use super::target_dialog::{OpenTargetDialog, PendingAssignment};

pub const NOT_USABLE: &str = "This object cannot be used.";

#[derive(Debug, PartialEq, Eq)]
pub enum ObjectAssignment {
    ChooseAim,
    PlainUse,
    NotUsable,
}

/// `MultiUse` wins over `Usable`, as it does for a right-click (`gestures::on_click`).
pub fn assignment_for(config: &ItemConfig) -> ObjectAssignment {
    if config.has_flag(ItemFlag::MultiUse) {
        ObjectAssignment::ChooseAim
    } else if config.has_flag(ItemFlag::Usable) {
        ObjectAssignment::PlainUse
    } else {
        ObjectAssignment::NotUsable
    }
}

pub(super) fn on_object_picked(
    event: On<ObjectPicked>,
    mut commands: Commands,
    mut bar: ResMut<ActionBar>,
) {
    let item_id = event.item.config.id;
    match assignment_for(&event.item.config) {
        ObjectAssignment::ChooseAim => commands.trigger(OpenTargetDialog {
            slot: event.slot,
            pending: PendingAssignment::Item(item_id),
        }),
        ObjectAssignment::PlainUse => {
            bar.set_action(event.slot, SlotAction::Item { item_id, aim: None })
        }
        ObjectAssignment::NotUsable => commands.trigger(ShowTextMessage {
            text: NOT_USABLE.to_string(),
            message_type: TextMessageType::ActionDenied,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{Item, ItemId};
    use std::sync::Arc;

    fn an_item(flags: Vec<ItemFlag>) -> Arc<Item> {
        Arc::new(Item::new(
            Arc::new(ItemConfig {
                id: ItemId(266),
                flags,
                friction: None,
                slot: None,
                minimap_color: None,
                elevation: None,
            }),
            1,
        ))
    }

    #[derive(Resource, Default)]
    struct Seen {
        aim_dialogs: usize,
        denials: Vec<String>,
    }

    fn pick(flags: Vec<ItemFlag>) -> World {
        let mut world = World::new();
        world.init_resource::<ActionBar>();
        world.init_resource::<Seen>();
        world.add_observer(on_object_picked);
        world.add_observer(|_: On<OpenTargetDialog>, mut seen: ResMut<Seen>| seen.aim_dialogs += 1);
        world.add_observer(|event: On<ShowTextMessage>, mut seen: ResMut<Seen>| {
            seen.denials.push(event.text.clone())
        });

        world.trigger(ObjectPicked {
            slot: 1,
            item: an_item(flags),
        });
        world.flush();
        world
    }

    #[test]
    fn a_multiuse_item_asks_for_an_aim_even_when_usable() {
        let world = pick(vec![ItemFlag::Usable, ItemFlag::MultiUse]);

        assert_eq!(world.resource::<Seen>().aim_dialogs, 1);
        assert!(world.resource::<ActionBar>().action(1).is_none());
    }

    #[test]
    fn a_usable_item_is_assigned_as_a_plain_use() {
        let world = pick(vec![ItemFlag::Usable]);

        assert_eq!(
            world.resource::<ActionBar>().action(1),
            Some(SlotAction::Item {
                item_id: ItemId(266),
                aim: None
            })
        );
    }

    #[test]
    fn an_unusable_item_is_refused_and_nothing_is_assigned() {
        let world = pick(vec![ItemFlag::Take]);

        assert_eq!(
            world.resource::<Seen>().denials,
            vec![NOT_USABLE.to_string()]
        );
        assert!(world.resource::<ActionBar>().action(1).is_none());
    }
}
