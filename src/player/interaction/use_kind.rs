use bevy::prelude::*;

use crate::{
    game_ui::WindowId,
    items::{ItemConfig, ItemFlag, ItemId, ItemPlacement},
};

use super::intent::InteractionIntent;
use super::mode::{InteractionMode, TargetingSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseKind {
    UseWith,
    Open,
    Use,
}

pub fn use_kind(config: &ItemConfig) -> Option<UseKind> {
    if config.has_flag(ItemFlag::MultiUse) {
        Some(UseKind::UseWith)
    } else if !config.has_flag(ItemFlag::Usable) {
        None
    } else if config.has_flag(ItemFlag::Container) {
        Some(UseKind::Open)
    } else {
        Some(UseKind::Use)
    }
}

/// `window_id` is the container window hosting the item, if any.
pub fn start_use(
    kind: UseKind,
    placement: ItemPlacement,
    item_id: ItemId,
    window_id: Option<WindowId>,
    commands: &mut Commands,
    mode: &mut InteractionMode,
) {
    if kind == UseKind::UseWith {
        *mode = InteractionMode::Targeting(TargetingSource::Item { placement, item_id });
        return;
    }
    let window_id = window_id
        .filter(|_| kind == UseKind::Open && matches!(placement, ItemPlacement::Container { .. }));
    commands.trigger(InteractionIntent::UseItem {
        target: placement,
        item_id,
        window_id,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::ContainerId;
    use crate::map::Position;
    use bevy::ecs::system::RunSystemOnce;

    fn config(flags: Vec<ItemFlag>) -> ItemConfig {
        ItemConfig {
            id: ItemId(100),
            name: None,
            flags,
            friction: None,
            slot: None,
            minimap_color: None,
            elevation: None,
        }
    }

    #[test]
    fn a_multi_use_item_is_used_with_something() {
        assert_eq!(
            use_kind(&config(vec![ItemFlag::Usable, ItemFlag::MultiUse])),
            Some(UseKind::UseWith)
        );
        assert_eq!(
            use_kind(&config(vec![ItemFlag::MultiUse])),
            Some(UseKind::UseWith)
        );
    }

    #[test]
    fn a_usable_container_is_opened() {
        assert_eq!(
            use_kind(&config(vec![ItemFlag::Usable, ItemFlag::Container])),
            Some(UseKind::Open)
        );
    }

    #[test]
    fn any_other_usable_item_is_used() {
        assert_eq!(
            use_kind(&config(vec![ItemFlag::Usable])),
            Some(UseKind::Use)
        );
    }

    #[test]
    fn an_item_that_is_not_usable_offers_nothing() {
        assert_eq!(use_kind(&config(vec![ItemFlag::Take])), None);
        assert_eq!(use_kind(&config(vec![ItemFlag::Container])), None);
    }

    #[derive(Resource, Default)]
    struct Intents(Vec<InteractionIntent>);

    fn started(kind: UseKind, placement: ItemPlacement, window_id: Option<WindowId>) -> World {
        let mut world = World::new();
        world.init_resource::<InteractionMode>();
        world.init_resource::<Intents>();
        world.add_observer(
            |event: On<InteractionIntent>, mut intents: ResMut<Intents>| {
                intents.0.push((*event).clone())
            },
        );
        world
            .run_system_once(
                move |mut commands: Commands, mut mode: ResMut<InteractionMode>| {
                    start_use(
                        kind,
                        placement.clone(),
                        ItemId(100),
                        window_id,
                        &mut commands,
                        &mut mode,
                    )
                },
            )
            .unwrap();
        world.flush();
        world
    }

    fn in_a_container() -> ItemPlacement {
        ItemPlacement::Container {
            container_id: ContainerId(1),
            slot: 2,
        }
    }

    #[test]
    fn use_with_enters_the_crosshair_and_sends_nothing() {
        let world = started(UseKind::UseWith, in_a_container(), Some(WindowId(9)));

        assert!(matches!(
            *world.resource::<InteractionMode>(),
            InteractionMode::Targeting(TargetingSource::Item {
                item_id: ItemId(100),
                ..
            })
        ));
        assert!(world.resource::<Intents>().0.is_empty());
    }

    #[test]
    fn opening_a_nested_container_reopens_in_its_window() {
        let world = started(UseKind::Open, in_a_container(), Some(WindowId(9)));

        assert!(matches!(
            world.resource::<Intents>().0.as_slice(),
            [InteractionIntent::UseItem {
                window_id: Some(WindowId(9)),
                ..
            }]
        ));
    }

    #[test]
    fn using_a_plain_item_in_a_container_names_no_window() {
        let world = started(UseKind::Use, in_a_container(), Some(WindowId(9)));

        assert!(matches!(
            world.resource::<Intents>().0.as_slice(),
            [InteractionIntent::UseItem {
                window_id: None,
                ..
            }]
        ));
    }

    #[test]
    fn opening_a_container_on_the_map_names_no_window() {
        let on_the_map = ItemPlacement::Map {
            position: Position::new(100, 100, 7),
            index: 0,
        };
        let world = started(UseKind::Open, on_the_map, None);

        assert!(matches!(
            world.resource::<Intents>().0.as_slice(),
            [InteractionIntent::UseItem {
                window_id: None,
                ..
            }]
        ));
    }
}
