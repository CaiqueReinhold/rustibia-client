use bevy::prelude::*;

use crate::{
    agent::AgentId,
    core::{ItemConfigs, TextMessageType},
    game_ui::WindowId,
    items::{ItemDragEnded, ItemFlag, ItemId, ItemPlacement},
    map::{Position, minimap::MinimapData},
    network::{ClientMessage, SendMessage, events::ShowTextMessage},
    player::{
        components::Player,
        movement::MovementQueue,
        pathfinding::{
            AutoWalkTarget, compute_path, compute_path_adjacent_to_both, compute_path_to_adjacent,
            is_adjacent,
        },
    },
};

use super::mode::ContainerNavTarget;

/// A semantic player action, produced by gesture handlers and consumed by
/// `on_interaction_intent`. Encoding to `ClientMessage` happens at send time,
/// so a deferred intent is still valid after the walk completes.
#[derive(Event, Debug, Clone)]
pub enum InteractionIntent {
    WalkTo(Position),
    Look(ItemPlacement),
    MoveItem {
        origin: ItemPlacement,
        item_id: ItemId,
        amount: u8,
        to: ItemPlacement,
    },
    UseItem {
        target: ItemPlacement,
        item_id: ItemId,
        /// When this use targets a container that lives inside an open container
        /// window, the window to reopen the result into (in-place navigation).
        window_id: Option<WindowId>,
    },
    UseItemWith {
        source: ItemPlacement,
        source_item_id: ItemId,
        target: ItemPlacement,
        target_item_id: ItemId,
        target_agent: Option<AgentId>,
    },
    SetTarget(Option<AgentId>, u32),
}

impl InteractionIntent {
    fn to_message(&self) -> Option<ClientMessage> {
        match self {
            InteractionIntent::WalkTo(_) => None,
            InteractionIntent::Look(placement) => Some(ClientMessage::Look {
                position: placement.to_wire_position(),
            }),
            InteractionIntent::MoveItem {
                origin,
                item_id,
                amount,
                to,
            } => Some(ClientMessage::MoveItem {
                from: origin.to_wire_position(),
                item_id: *item_id,
                amount: *amount,
                stack_index: origin.wire_stack_index(),
                to: to.to_wire_position(),
            }),
            InteractionIntent::UseItem {
                target, item_id, ..
            } => Some(ClientMessage::UseItem {
                position: target.to_wire_position(),
                item_id: *item_id,
                stack_index: target.wire_stack_index(),
            }),
            InteractionIntent::UseItemWith {
                source,
                source_item_id,
                target,
                target_item_id,
                target_agent,
            } => Some(ClientMessage::UseItemWith {
                source: source.to_wire_position(),
                source_item_id: *source_item_id,
                source_index: source.wire_stack_index(),
                target: target.to_wire_position(),
                target_item_id: *target_item_id,
                target_index: target.wire_stack_index(),
                target_agent: *target_agent,
            }),
            InteractionIntent::SetTarget(agent_id, seq) => Some(ClientMessage::SetTarget {
                agent_id: *agent_id,
                seq: *seq,
            }),
        }
    }

    /// Map-floor tiles the player must stand on or next to before this
    /// intent may be sent. At most two entries (use-with source + target).
    /// `thrown` drops the target: a rune is used from where the player stands,
    /// and the server limits it by line of sight rather than by adjacency.
    fn required_map_positions(&self, thrown: bool) -> Vec<Position> {
        let mut positions = Vec::new();
        let mut push_if_map = |placement: &ItemPlacement| {
            if let ItemPlacement::Map { position, .. } = placement {
                positions.push(position.clone());
            }
        };
        match self {
            InteractionIntent::WalkTo(_)
            | InteractionIntent::Look(_)
            | InteractionIntent::SetTarget(..) => {}
            InteractionIntent::MoveItem { origin, .. } => push_if_map(origin),
            InteractionIntent::UseItem { target, .. } => push_if_map(target),
            InteractionIntent::UseItemWith { source, target, .. } => {
                push_if_map(source);
                if !thrown {
                    push_if_map(target);
                }
            }
        }
        positions
    }
}

#[derive(Resource, Debug)]
pub struct PendingWalkAction {
    pub item_pos: Position,
    pub intent: InteractionIntent,
}

/// Encode and send an intent, plus its post-send bookkeeping. Called both by
/// the dispatcher (when already in reach) and by `fire_pending_action` (after
/// a deferred walk completes).
pub fn send_intent(commands: &mut Commands, intent: &InteractionIntent) {
    let Some(msg) = intent.to_message() else {
        return;
    };
    commands.trigger(SendMessage(msg));
    match intent {
        // Fire-and-forget: the server is authoritative and will send the
        // resulting state (a walk, a look text message, etc.) if the action
        // succeeds. Nothing to track client-side.
        InteractionIntent::WalkTo(_)
        | InteractionIntent::Look(_)
        | InteractionIntent::UseItemWith { .. }
        | InteractionIntent::SetTarget(..) => {}
        InteractionIntent::MoveItem { .. } => {
            commands.trigger(ItemDragEnded);
        }
        InteractionIntent::UseItem { window_id, .. } => {
            // Only when using a container nested inside an open container window:
            // record which window the resulting OpenContainer should replace.
            if let Some(window_id) = window_id {
                commands.insert_resource(ContainerNavTarget(*window_id));
            }
        }
    }
}

fn show_no_way(commands: &mut Commands) {
    commands.trigger(ShowTextMessage {
        text: "There is no way.".to_string(),
        message_type: TextMessageType::ActionDenied,
    });
}

pub fn on_interaction_intent(
    event: On<InteractionIntent>,
    mut commands: Commands,
    minimap: Res<MinimapData>,
    mut move_queue: ResMut<MovementQueue>,
    configs: Res<ItemConfigs>,
    player_q: Single<&Position, With<Player>>,
) {
    let player_pos = player_q.into_inner();
    let intent = &*event;

    if let InteractionIntent::WalkTo(target) = intent {
        match compute_path(player_pos, target, &minimap) {
            Some(steps) => {
                move_queue.set_auto_walk_path(steps);
                commands.insert_resource(AutoWalkTarget(target.clone()));
            }
            None => show_no_way(&mut commands),
        }
        return;
    }

    let thrown = match intent {
        InteractionIntent::UseItemWith { source_item_id, .. } => configs
            .items
            .get(source_item_id)
            .is_some_and(|config| config.has_flag(ItemFlag::Rune)),
        _ => false,
    };

    let reaches = |p: &Position| player_pos == p || is_adjacent(player_pos, p);
    let required = intent.required_map_positions(thrown);

    if required.iter().all(reaches) {
        send_intent(&mut commands, intent);
        return;
    }

    let path = match required.as_slice() {
        [single] => compute_path_to_adjacent(player_pos, single, &minimap),
        [a, b] => compute_path_adjacent_to_both(player_pos, a, b, &minimap),
        _ => None,
    };
    // Anchor on the last required position: the use-with target / the item
    // being moved or used. Matches the pre-refactor behavior.
    let anchor = required
        .last()
        .expect("unreached position implies at least one required position")
        .clone();

    match path {
        Some(steps) => {
            move_queue.set_auto_walk_path(steps);
            commands.insert_resource(AutoWalkTarget(anchor.clone()));
            commands.insert_resource(PendingWalkAction {
                item_pos: anchor,
                intent: intent.clone(),
            });
        }
        None => show_no_way(&mut commands),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentId;

    #[test]
    fn a_carried_use_sends_the_search_coordinate() {
        let intent = InteractionIntent::UseItem {
            target: ItemPlacement::Carried,
            item_id: ItemId(266),
            window_id: None,
        };

        assert!(matches!(
            intent.to_message(),
            Some(ClientMessage::UseItem {
                position: Position {
                    x: 0xFFFE,
                    y: 0xFFFF,
                    z: 0
                },
                item_id: ItemId(266),
                stack_index: 0,
            })
        ));
        assert!(intent.required_map_positions(false).is_empty());
    }

    #[test]
    fn a_carried_use_with_walks_only_to_its_target() {
        let target = Position::new(101, 100, 7);
        let intent = InteractionIntent::UseItemWith {
            source: ItemPlacement::Carried,
            source_item_id: ItemId(266),
            target: ItemPlacement::Map {
                position: target.clone(),
                index: 0,
            },
            target_item_id: ItemId(100),
            target_agent: Some(AgentId(3)),
        };

        assert!(matches!(
            intent.to_message(),
            Some(ClientMessage::UseItemWith {
                source: Position {
                    x: 0xFFFE,
                    y: 0xFFFF,
                    z: 0
                },
                source_index: 0,
                target_agent: Some(AgentId(3)),
                ..
            })
        ));
        assert_eq!(intent.required_map_positions(false), vec![target]);
    }

    /// The rune case: thrown from where the player stands, so the aimed tile is
    /// not a place the walk has to reach first.
    #[test]
    fn a_thrown_use_with_requires_nothing_of_its_target() {
        let intent = InteractionIntent::UseItemWith {
            source: ItemPlacement::Carried,
            source_item_id: ItemId(3161),
            target: ItemPlacement::Map {
                position: Position::new(105, 100, 7),
                index: 0,
            },
            target_item_id: ItemId(100),
            target_agent: None,
        };

        assert!(intent.required_map_positions(true).is_empty());
        assert_eq!(intent.required_map_positions(false).len(), 1);
    }
}
