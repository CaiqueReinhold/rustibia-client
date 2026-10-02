use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use smallvec::SmallVec;

use crate::agent::AgentId;
use crate::items::{Item, ItemFlag};
use crate::map::position::Position;

#[derive(Debug, Default)]
pub struct MapTile {
    pub items: Vec<Arc<Item>>,
    /// Agents standing here, in arrival order. Topmost is `.last()`, mirroring
    /// `peek_item`. This is the client's own ordering: the server knows its own
    /// stack order but does not send it (see the spec's follow-up task), so this
    /// list is authoritative for the client and nothing else.
    pub agents: SmallVec<[AgentId; 1]>,
}

#[derive(Resource, Default)]
pub struct Map {
    tiles: HashMap<Position, MapTile>,
    agents: HashMap<AgentId, Entity>,
    /// Where each indexed agent currently sits, per `index_agent`/`unindex_agent`.
    /// The single source of truth for the tile index: callers name the agent
    /// rather than a position, which can already be stale by the time they call.
    agent_tiles: HashMap<AgentId, Position>,
}

impl Map {
    pub fn add_agent(&mut self, id: AgentId, agent: Entity) {
        self.agents.insert(id, agent);
    }

    pub fn remove_agent(&mut self, id: AgentId) {
        self.agents.remove(&id);
    }

    pub fn get_agent(&self, id: AgentId) -> Option<Entity> {
        self.agents.get(&id).cloned()
    }

    pub fn replace_tile(&mut self, items: Vec<Arc<Item>>, pos: &Position) {
        let tile = self.tiles.entry(pos.clone()).or_default();
        tile.items = items;
    }

    /// Strips every tile `keep` rejects, keeping the entry of one an agent stands on. Returns the
    /// positions that lost items.
    pub fn evict_unless(&mut self, keep: impl Fn(&Position) -> bool) -> Vec<Position> {
        let mut evicted = Vec::new();
        self.tiles.retain(|pos, tile| {
            if keep(pos) {
                return true;
            }
            if !tile.items.is_empty() {
                tile.items.clear();
                evicted.push(pos.clone());
            }
            !tile.agents.is_empty()
        });
        evicted
    }

    pub fn agents_on(&self, pos: &Position) -> &[AgentId] {
        self.tiles.get(pos).map_or(&[], |t| t.agents.as_slice())
    }

    /// Moves `id`'s tile-index entry to `pos`, first removing it from wherever
    /// `agent_tiles` says it was indexed before. A no-op if `id` is already
    /// indexed at `pos` — the caller (`sync_tile_agents`) fires on every
    /// `Changed<Position>`, including a component reinserted at its current
    /// value (`teleport_agents` re-inserts unconditionally), and skipping the
    /// remove-then-push in that case is what keeps arrival order from churning.
    pub(crate) fn index_agent(&mut self, id: AgentId, pos: &Position) {
        if self.agent_tiles.get(&id) == Some(pos) {
            return;
        }
        if let Some(old) = self.agent_tiles.remove(&id)
            && let Some(tile) = self.tiles.get_mut(&old)
        {
            tile.agents.retain(|a| *a != id);
        }
        self.tiles.entry(pos.clone()).or_default().agents.push(id);
        self.agent_tiles.insert(id, pos.clone());
    }

    /// Removes `id` from wherever `Map` last recorded it standing — never from
    /// a position the caller supplies. A caller's live `Position` component can
    /// already be stale by the time it calls this (a `Position`-writing system
    /// can run in the same `Update` as the removal, ahead of `sync_tile_agents`'s
    /// next `PreUpdate` pass), which would otherwise leave a stale id on the old
    /// tile forever since the despawned entity never triggers `Changed<Position>`
    /// again. `agent_tiles` is authoritative, so there is no stale position to
    /// pass.
    pub(crate) fn unindex_agent(&mut self, id: AgentId) {
        if let Some(pos) = self.agent_tiles.remove(&id)
            && let Some(tile) = self.tiles.get_mut(&pos)
        {
            tile.agents.retain(|a| *a != id);
        }
    }

    pub fn can_walk(&self, pos: &Position) -> bool {
        let tile = match self.tiles.get(pos) {
            Some(t) => t,
            None => return false,
        };

        let has_ground = tile
            .items
            .iter()
            .any(|i| i.config.has_flag(ItemFlag::Ground));
        if !has_ground {
            return false;
        }

        let blocked = tile
            .items
            .iter()
            .any(|i| i.config.has_flag(ItemFlag::Unpass));
        !blocked
    }

    pub fn can_drop_item(&self, pos: &Position) -> bool {
        let tile = match self.tiles.get(pos) {
            Some(t) => t,
            None => return false,
        };

        tile.items
            .iter()
            .any(|i| i.config.has_flag(ItemFlag::FullBank))
            && !tile
                .items
                .iter()
                .any(|i| i.config.has_flag(ItemFlag::Bottom))
    }

    pub fn peek_item(&self, position: &Position) -> Option<(&Arc<Item>, usize)> {
        let tile = self.tiles.get(position)?;
        let item = tile.items.last()?;
        let index = tile.items.len() - 1;
        Some((item, index))
    }

    pub fn item_at(&self, position: &Position, index: usize) -> Option<&Arc<Item>> {
        self.tiles.get(position)?.items.get(index)
    }

    pub fn get_tile_friction(&self, pos: &Position) -> Option<u16> {
        let tile = self.tiles.get(pos)?;

        if !self.can_walk(pos) {
            return None;
        }

        // The server takes the first item carrying a `tile_friction` attribute
        // (`GameMap::tile_friction`). Matching that rule rather than filtering on
        // the Ground flag separately keeps the two sides from drifting apart; the
        // parse in `core::items` is what makes the two selections equivalent.
        tile.items.iter().find_map(|i| i.config.friction)
    }

    pub fn get_items(&self, pos: &Position) -> Option<impl Iterator<Item = &Item>> {
        let tile = self.tiles.get(pos)?;
        Some(tile.items.iter().map(|i| i.as_ref()))
    }

    pub fn get_minimap_color(&self, pos: &Position) -> Option<u8> {
        let tile = self.tiles.get(pos)?;
        tile.items
            .iter()
            .rev()
            .find_map(|it| it.config.minimap_color)
    }

    pub fn avoid(&self, pos: &Position) -> bool {
        let Some(tile) = self.tiles.get(pos) else {
            return true;
        };

        tile.items
            .iter()
            .any(|it| it.config.has_flag(ItemFlag::Avoid))
    }

    pub fn get_elevation(&self, pos: &Position) -> u8 {
        let Some(tile) = self.tiles.get(pos) else {
            return 0;
        };

        tile.items
            .iter()
            .filter_map(|it| it.config.elevation)
            .take(3)
            .sum()
    }

    pub fn is_ground(&self, pos: &Position) -> bool {
        let Some(tile) = self.tiles.get(pos) else {
            return false;
        };
        tile.items
            .iter()
            .any(|it| it.config.has_flag(ItemFlag::Ground) || it.config.has_flag(ItemFlag::Border))
    }

    pub fn is_bottom(&self, pos: &Position) -> bool {
        let Some(tile) = self.tiles.get(pos) else {
            return false;
        };
        tile.items
            .iter()
            .any(|it| it.config.has_flag(ItemFlag::Bottom))
    }

    pub fn block_sight(&self, pos: &Position) -> bool {
        let Some(tile) = self.tiles.get(pos) else {
            return false;
        };
        tile.items
            .iter()
            .any(|it| it.config.has_flag(ItemFlag::BlockSight))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemConfig, ItemFlag, ItemId};
    use crate::map::viewport::in_viewport;

    fn item(id: ItemId, flags: Vec<ItemFlag>, friction: Option<u16>) -> Arc<Item> {
        Arc::new(Item::new(
            Arc::new(ItemConfig {
                id,
                flags,
                friction,
                slot: None,
                minimap_color: None,
                elevation: None,
            }),
            1,
        ))
    }

    fn at(x: u16, y: u16) -> Position {
        Position { x, y, z: 7 }
    }

    /// The rule is the server's: first item carrying a value, not first item
    /// carrying the Ground flag. Items stacked above the ground have no friction,
    /// so the two agree — this pins that they keep agreeing.
    #[test]
    fn friction_comes_from_the_first_item_that_has_it() {
        let mut map = Map::default();
        map.replace_tile(
            vec![
                item(ItemId(100), vec![ItemFlag::Ground], Some(150)),
                item(ItemId(200), Vec::new(), None),
            ],
            &at(10, 10),
        );

        assert_eq!(map.get_tile_friction(&at(10, 10)), Some(150));
    }

    /// The regression this task exists for: 260 used to truncate through a `u8`
    /// into 4, predicting a 50ms step where the server charges 900ms.
    #[test]
    fn friction_above_255_survives() {
        let mut map = Map::default();
        map.replace_tile(
            vec![item(ItemId(21718), vec![ItemFlag::Ground], Some(260))],
            &at(10, 10),
        );

        assert_eq!(map.get_tile_friction(&at(10, 10)), Some(260));
    }

    /// An unwalkable tile has no friction to report, which is what keeps the
    /// minimap's A* from routing through it.
    #[test]
    fn an_unwalkable_tile_reports_no_friction() {
        let mut map = Map::default();
        map.replace_tile(
            vec![
                item(ItemId(100), vec![ItemFlag::Ground], Some(150)),
                item(ItemId(200), vec![ItemFlag::Unpass], None),
            ],
            &at(10, 10),
        );

        assert_eq!(map.get_tile_friction(&at(10, 10)), None);
    }

    #[test]
    fn an_unknown_tile_reports_no_friction() {
        let map = Map::default();
        assert_eq!(map.get_tile_friction(&at(10, 10)), None);
    }

    /// `replace_tile` only replaces items; a tile update (e.g. a door opening)
    /// must not evict the agents standing there. A wholesale
    /// `MapTile { items, ..Default::default() }` would pass every other test
    /// in this module while silently emptying the agent index on every tile
    /// change.
    #[test]
    fn replace_tile_preserves_agents_standing_there() {
        let mut map = Map::default();
        map.index_agent(AgentId(5), &at(10, 10));

        map.replace_tile(
            vec![item(ItemId(100), vec![ItemFlag::Ground], None)],
            &at(10, 10),
        );

        assert_eq!(map.agents_on(&at(10, 10)), &[AgentId(5)]);
    }

    /// `sync_tile_agents` calls `index_agent` on every `Changed<Position>`,
    /// including a no-op re-insert of the agent's current tile (e.g.
    /// `teleport_agents` re-inserts unconditionally). Without the same-position
    /// short-circuit this would unindex-then-repush on every such touch,
    /// silently reordering co-located agents even though nobody moved.
    #[test]
    fn indexing_an_agent_at_its_current_tile_is_a_no_op() {
        let mut map = Map::default();
        map.index_agent(AgentId(1), &at(10, 10));
        map.index_agent(AgentId(2), &at(10, 10));

        map.index_agent(AgentId(1), &at(10, 10));

        assert_eq!(map.agents_on(&at(10, 10)), &[AgentId(1), AgentId(2)]);
    }

    fn ground() -> Arc<Item> {
        item(ItemId(100), vec![ItemFlag::Ground], None)
    }

    #[test]
    fn eviction_drops_a_tile_outside_the_window_and_keeps_one_inside() {
        let mut map = Map::default();
        map.replace_tile(vec![ground()], &at(100, 100));
        map.replace_tile(vec![ground()], &at(110, 100));

        let evicted = map.evict_unless(|pos| in_viewport(&at(100, 100), pos));

        assert_eq!(evicted, vec![at(110, 100)]);
        assert!(map.get_items(&at(110, 100)).is_none());
        assert_eq!(map.get_items(&at(100, 100)).unwrap().count(), 1);
    }

    #[test]
    fn eviction_drops_a_floor_the_server_no_longer_describes() {
        let mut map = Map::default();
        let surface = Position::new(100, 100, 7);
        map.replace_tile(vec![ground()], &surface);

        let evicted = map.evict_unless(|pos| in_viewport(&Position::new(100, 100, 8), pos));

        assert_eq!(evicted, vec![surface.clone()]);
        assert!(map.get_items(&surface).is_none());
    }

    #[test]
    fn eviction_clears_the_items_under_an_agent_but_keeps_the_agent_indexed() {
        let mut map = Map::default();
        map.replace_tile(vec![ground()], &at(110, 100));
        map.index_agent(AgentId(5), &at(110, 100));

        let evicted = map.evict_unless(|pos| in_viewport(&at(100, 100), pos));

        assert_eq!(evicted, vec![at(110, 100)]);
        assert_eq!(map.get_items(&at(110, 100)).unwrap().count(), 0);
        assert_eq!(map.agents_on(&at(110, 100)), &[AgentId(5)]);
    }

    #[test]
    fn an_agent_only_tile_outside_is_not_reported_as_evicted() {
        let mut map = Map::default();
        map.index_agent(AgentId(5), &at(110, 100));

        assert!(
            map.evict_unless(|pos| in_viewport(&at(100, 100), pos))
                .is_empty()
        );
        assert_eq!(map.agents_on(&at(110, 100)), &[AgentId(5)]);
    }
}
