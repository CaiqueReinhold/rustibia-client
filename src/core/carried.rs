use std::collections::HashMap;

use bevy::prelude::*;

use crate::{items::ItemId, network::events::CarriedItemsReceived};

/// `None` until the server's first table arrives.
#[derive(Resource, Debug, Default)]
pub struct CarriedItems(Option<HashMap<ItemId, u32>>);

impl CarriedItems {
    pub fn new(items: impl IntoIterator<Item = (ItemId, u32)>) -> Self {
        Self(Some(items.into_iter().collect()))
    }

    pub fn count(&self, item_id: ItemId) -> Option<u32> {
        Some(self.0.as_ref()?.get(&item_id).copied().unwrap_or(0))
    }
}

pub fn on_carried_items(event: On<CarriedItemsReceived>, mut carried: ResMut<CarriedItems>) {
    *carried = CarriedItems::new(event.items.iter().copied());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_counted_before_the_first_table() {
        assert_eq!(CarriedItems::default().count(ItemId(3031)), None);
    }

    #[test]
    fn an_id_missing_from_a_table_is_carried_zero_times() {
        let carried = CarriedItems::new([(ItemId(3031), 1240)]);

        assert_eq!(carried.count(ItemId(3031)), Some(1240));
        assert_eq!(carried.count(ItemId(268)), Some(0));
    }

    #[test]
    fn each_table_replaces_the_last() {
        let mut world = World::new();
        world.init_resource::<CarriedItems>();
        world.add_observer(on_carried_items);

        world.trigger(CarriedItemsReceived {
            items: vec![(ItemId(3031), 10), (ItemId(268), 3)],
        });
        world.trigger(CarriedItemsReceived {
            items: vec![(ItemId(268), 2)],
        });
        world.flush();

        let carried = world.resource::<CarriedItems>();
        assert_eq!(carried.count(ItemId(3031)), Some(0));
        assert_eq!(carried.count(ItemId(268)), Some(2));
    }
}
