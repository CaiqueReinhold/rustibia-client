use std::sync::Arc;

use bevy::prelude::*;

use crate::items::{ContainerId, Item, ItemPlacement};

#[derive(Event)]
pub struct ItemDragStarted {
    pub item: Arc<Item>,
}

#[derive(Event)]
pub struct ItemDragEnded;

#[derive(Event)]
pub struct OpenParentContainer {
    pub container_id: ContainerId,
}

/// A Ctrl-drop on a countable stack: ask how much of it to move.
///
/// Raised by `gestures::on_drag_end` only after the destination has been
/// validated, so the dialog never has to reject one.
#[derive(Event)]
pub struct OpenSplitDialog {
    pub item: Arc<Item>,
    pub origin: ItemPlacement,
    pub to: ItemPlacement,
}
