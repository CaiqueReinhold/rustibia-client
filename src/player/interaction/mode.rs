use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::CursorIcon;

use crate::{
    core::SpellId,
    game_ui::WindowId,
    items::{Item, ItemId, ItemPlacement},
    network::events::{ContainerClosed, IventorySlotUpdated, TileChanged, UpdateContainer},
};

#[derive(Debug, Clone)]
pub enum TargetingSource {
    Item {
        placement: ItemPlacement,
        item_id: ItemId,
    },
    Spell(SpellId),
    AssignObject {
        slot: u16,
    },
}

/// A crosshair click that picked an object for action-bar slot `slot`.
#[derive(Event, Debug)]
pub struct ObjectPicked {
    pub slot: u16,
    pub item: Arc<Item>,
}

#[derive(Resource, Debug, Default)]
pub enum InteractionMode {
    #[default]
    Idle,
    Dragging {
        item: Arc<Item>,
        origin: ItemPlacement,
        crossed_threshold: bool,
    },
    Targeting(TargetingSource),
}

impl InteractionMode {
    pub fn drag_crossed_threshold(&self) -> bool {
        matches!(
            self,
            InteractionMode::Dragging {
                crossed_threshold: true,
                ..
            }
        )
    }

    pub fn is_targeting(&self) -> bool {
        matches!(self, InteractionMode::Targeting(_))
    }

    fn clear_targeting_if_gone(&mut self, gone: impl FnOnce(&ItemPlacement, ItemId) -> bool) {
        if let InteractionMode::Targeting(TargetingSource::Item { placement, item_id }) = &*self
            && gone(placement, *item_id)
        {
            *self = InteractionMode::Idle;
        }
    }
}

#[derive(Resource, Debug)]
pub struct ContainerNavTarget(pub WindowId);

pub fn sync_targeting_cursor(
    mut commands: Commands,
    mode: Res<InteractionMode>,
    window_q: Single<Entity, With<Window>>,
    mut last_active: Local<bool>,
) {
    let active = mode.is_targeting();
    if active == *last_active {
        return;
    }

    info!("Is targeting: {}. Last active: {}", active, *last_active);
    *last_active = active;
    let icon = if active {
        bevy::window::SystemCursorIcon::Crosshair
    } else {
        bevy::window::SystemCursorIcon::Default
    };
    commands.entity(*window_q).insert(CursorIcon::System(icon));
}

pub fn on_targeting_tile_changed(event: On<TileChanged>, mut mode: ResMut<InteractionMode>) {
    mode.clear_targeting_if_gone(|source, item_id| {
        let ItemPlacement::Map { position, index } = source else {
            return false;
        };
        if position != &event.position {
            return false;
        }
        !event
            .items
            .iter()
            .filter_map(|opt| opt.as_ref())
            .nth(*index)
            .map(|(id, _)| *id == item_id)
            .unwrap_or(false)
    });
}

pub fn on_targeting_container_updated(
    event: On<UpdateContainer>,
    mut mode: ResMut<InteractionMode>,
) {
    mode.clear_targeting_if_gone(|source, item_id| {
        let ItemPlacement::Container { container_id, slot } = source else {
            return false;
        };
        if *container_id != event.container_id {
            return false;
        }
        !event
            .items
            .get(*slot)
            .and_then(|opt| opt.as_ref())
            .map(|(id, _)| *id == item_id)
            .unwrap_or(false)
    });
}

pub fn on_targeting_container_closed(
    event: On<ContainerClosed>,
    mut mode: ResMut<InteractionMode>,
) {
    mode.clear_targeting_if_gone(|source, _| {
        matches!(source, ItemPlacement::Container { container_id, .. }
            if *container_id == event.container_id)
    });
}

pub fn on_targeting_inventory_updated(
    event: On<IventorySlotUpdated>,
    mut mode: ResMut<InteractionMode>,
) {
    mode.clear_targeting_if_gone(|source, item_id| {
        let ItemPlacement::Inventory { slot } = source else {
            return false;
        };
        *slot == event.slot && !event.item_id.map(|id| id == item_id).unwrap_or(false)
    });
}
