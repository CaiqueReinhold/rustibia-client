use bevy::prelude::*;

use crate::{
    agent::AgentId,
    conf::ui::MIN_DRAG_THRESHOLD,
    game_ui::{MainUI, UiWindowRef},
    items::{
        ItemDragEnded, ItemDragStarted, ItemFlag, ItemPlacement, LootContainerUI, OpenSplitDialog,
    },
    map::{Map, Position},
    player::components::{Player, PlayerInventory},
    player::target::{CombatTarget, TargetSquare, refresh_target_square},
};

use super::hover::{MapPick, MouseHoverState, cursor_target, valid_drop_target};
use super::intent::InteractionIntent;
use super::mode::InteractionMode;

pub fn attach_observers(event: On<Add, MainUI>, mut commands: Commands) {
    commands
        .entity(event.entity)
        .observe(on_drag_start)
        .observe(on_drag)
        .observe(on_drag_end)
        .observe(on_click);
}

fn on_drag(event: On<Pointer<Drag>>, mut commands: Commands, mut mode: ResMut<InteractionMode>) {
    let InteractionMode::Dragging {
        item,
        origin,
        crossed_threshold,
    } = &mut *mode
    else {
        return;
    };

    if *crossed_threshold || event.distance.max_element().abs() < MIN_DRAG_THRESHOLD {
        return;
    }

    *crossed_threshold = true;
    commands.trigger(ItemDragStarted {
        item: item.clone(),
        origin: origin.clone(),
    });
}

fn on_drag_start(
    event: On<Pointer<DragStart>>,
    mut mode: ResMut<InteractionMode>,
    hover_state: Res<MouseHoverState>,
    map: Res<Map>,
    container_q: Query<(&LootContainerUI, &UiWindowRef)>,
    inventory: Res<PlayerInventory>,
) {
    if mode.is_targeting() {
        return; // targeting owns the pointer; Escape or a click ends it
    }
    *mode = InteractionMode::Idle;
    if event.button != PointerButton::Primary {
        return;
    }

    let Some(target) = cursor_target(&hover_state, &map, &container_q, &inventory, MapPick::Top)
    else {
        return;
    };

    if target.item.config.has_flag(ItemFlag::Unmove) {
        return;
    }

    *mode = InteractionMode::Dragging {
        item: target.item,
        origin: target.placement,
        crossed_threshold: false,
    };
}

fn on_drag_end(
    _: On<Pointer<DragEnd>>,
    mut commands: Commands,
    hover_state: Res<MouseHoverState>,
    mut mode: ResMut<InteractionMode>,
    map: Res<Map>,
    container_q: Query<(&LootContainerUI, &UiWindowRef)>,
    keyboard: Res<ButtonInput<KeyCode>>,
) {
    let InteractionMode::Dragging {
        item,
        origin,
        crossed_threshold,
    } = &*mode
    else {
        return;
    };

    commands.trigger(ItemDragEnded);
    if !crossed_threshold {
        *mode = InteractionMode::Idle;
        return;
    }

    let (item, origin) = (item.clone(), origin.clone());
    *mode = InteractionMode::Idle;

    let Some(to) = valid_drop_target(&item, &hover_state, &map, &container_q) else {
        return;
    };

    if to.to_wire_position() == origin.to_wire_position() {
        return;
    }

    // Ctrl on a divisible stack asks how many; everything else moves whole. The
    // fork sits here, after the destination is validated, so the dialog never
    // opens on a drop that was going nowhere.
    if keyboard.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
        && item.is_countable_stack()
    {
        commands.trigger(OpenSplitDialog { item, origin, to });
        return;
    }

    commands.trigger(InteractionIntent::MoveItem {
        origin,
        item_id: item.config.id,
        amount: item.amount as u8,
        to,
    });
}

/// The agent a right-click on `tile` should target: the topmost entry that is not
/// the local player. A tile's agent list is in arrival order, so the last entry
/// is the topmost — the same convention `Map::peek_item` uses for items. Scanning
/// from the back applies it while skipping self.
pub(super) fn targetable_agent_on(
    map: &Map,
    tile: &Position,
    self_id: Option<AgentId>,
) -> Option<AgentId> {
    map.agents_on(tile)
        .iter()
        .rev()
        .find(|id| Some(**id) != self_id)
        .copied()
}

fn agent_to_use_on(map: &Map, tile: &Position) -> Option<AgentId> {
    map.agents_on(tile).last().copied()
}

fn on_click(
    event: On<Pointer<Click>>,
    mut commands: Commands,
    hover_state: Res<MouseHoverState>,
    map: Res<Map>,
    mut mode: ResMut<InteractionMode>,
    container_q: Query<(&LootContainerUI, &UiWindowRef)>,
    inventory: Res<PlayerInventory>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut combat_target: ResMut<CombatTarget>,
    square_q: Query<Entity, With<TargetSquare>>,
    player_q: Query<&Player>,
) {
    let player_agent_id = player_q.single().ok().map(|p| p.agent_id);

    // Targeting owns the click: resolve UseItemWith on primary, cancel on
    // secondary.
    if let InteractionMode::Targeting {
        source,
        source_item_id,
    } = &*mode
    {
        let (source, source_item_id) = (source.clone(), *source_item_id);
        *mode = InteractionMode::Idle;

        if event.button != PointerButton::Primary {
            return;
        }

        let Some(target) = cursor_target(
            &hover_state,
            &map,
            &container_q,
            &inventory,
            MapPick::PreferForceUse,
        ) else {
            return;
        };

        commands.trigger(InteractionIntent::UseItemWith {
            source,
            source_item_id,
            target: target.placement,
            target_item_id: target.item.config.id,
            target_agent: hover_state
                .tile_position
                .as_ref()
                .and_then(|tile| agent_to_use_on(&map, tile)),
        });
        return;
    }

    if mode.drag_crossed_threshold() {
        return;
    }

    if event.button == PointerButton::Primary {
        // Shift+primary → look.
        if keyboard.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
            if let Some(target) =
                cursor_target(&hover_state, &map, &container_q, &inventory, MapPick::Top)
            {
                commands.trigger(InteractionIntent::Look(target.placement));
            }
            return;
        }

        // Unmodified primary → walk.
        if let Some(target) = &hover_state.tile_position
            && !keyboard.any_pressed([
                KeyCode::ControlLeft,
                KeyCode::ControlRight,
                KeyCode::AltLeft,
                KeyCode::AltRight,
            ])
        {
            commands.trigger(InteractionIntent::WalkTo(target.clone()));
        }
        return;
    }

    if event.button == PointerButton::Secondary {
        // An agent on the tile takes the click; otherwise it falls through to the
        // normal use / multi-use path below, unchanged.
        if let Some(tile) = &hover_state.tile_position
            && let Some(agent_id) = targetable_agent_on(&map, tile, player_agent_id)
        {
            // Applies optimistically and yields what to send, in one step.
            let (next, seq) = combat_target.apply_click(agent_id);
            refresh_target_square(&mut commands, &combat_target, &map, &square_q);
            commands.trigger(InteractionIntent::SetTarget(next, seq));
            return;
        }

        let Some(target) =
            cursor_target(&hover_state, &map, &container_q, &inventory, MapPick::Top)
        else {
            return;
        };

        if target.item.config.has_flag(ItemFlag::MultiUse) {
            *mode = InteractionMode::Targeting {
                source: target.placement.clone(),
                source_item_id: target.item.config.id,
            };
            return;
        }

        if !target.item.config.has_flag(ItemFlag::Usable) {
            return;
        }

        let window_id = if matches!(target.placement, ItemPlacement::Container { .. })
            && target.item.config.has_flag(ItemFlag::Container)
        {
            target.window_id
        } else {
            None
        };
        commands.trigger(InteractionIntent::UseItem {
            target: target.placement,
            item_id: target.item.config.id,
            window_id,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Targetable = any agent on the tile except the local player. Skipping self is
    /// what keeps right-clicking your own tile reaching the item beneath you.
    #[test]
    fn resolves_the_topmost_non_self_agent() {
        let mut map = Map::default();
        let tile = Position { x: 10, y: 10, z: 7 };
        map.index_agent(AgentId(1), &tile);
        map.index_agent(AgentId(2), &tile);

        assert_eq!(
            targetable_agent_on(&map, &tile, Some(AgentId(2))),
            Some(AgentId(1))
        );
        assert_eq!(
            targetable_agent_on(&map, &tile, Some(AgentId(99))),
            Some(AgentId(2))
        );
    }

    #[test]
    fn a_tile_holding_only_the_player_has_no_target() {
        let mut map = Map::default();
        let tile = Position { x: 10, y: 10, z: 7 };
        map.index_agent(AgentId(7), &tile);

        assert_eq!(targetable_agent_on(&map, &tile, Some(AgentId(7))), None);
    }

    #[test]
    fn an_empty_tile_has_no_target() {
        let map = Map::default();
        let tile = Position { x: 10, y: 10, z: 7 };

        assert_eq!(targetable_agent_on(&map, &tile, Some(AgentId(7))), None);
    }

    /// The counterpart to `resolves_the_topmost_non_self_agent`: this one must not
    /// skip self, or you could never drink your own potion.
    #[test]
    fn the_use_target_includes_the_local_player() {
        let mut map = Map::default();
        let tile = Position { x: 10, y: 10, z: 7 };
        map.index_agent(AgentId(7), &tile);

        assert_eq!(agent_to_use_on(&map, &tile), Some(AgentId(7)));
    }

    #[test]
    fn the_use_target_is_the_topmost_agent() {
        let mut map = Map::default();
        let tile = Position { x: 10, y: 10, z: 7 };
        map.index_agent(AgentId(1), &tile);
        map.index_agent(AgentId(2), &tile);

        assert_eq!(agent_to_use_on(&map, &tile), Some(AgentId(2)));
    }

    #[test]
    fn an_empty_tile_has_nothing_to_use_on() {
        let map = Map::default();
        let tile = Position { x: 10, y: 10, z: 7 };

        assert_eq!(agent_to_use_on(&map, &tile), None);
    }
}
