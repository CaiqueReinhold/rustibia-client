use std::sync::Arc;

use crate::{
    conf::map::{CONTAINER_COORD_FLAG, INVENTORY_COORD_FLAG},
    core::SpriteConfig,
    items::{ContainerId, fluid_cell},
    map::Position,
};

pub type ItemId = u16;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Copy, PartialOrd, Ord)]
pub enum InventorySlot {
    Head,
    Amulet,
    Chest,
    Backpack,
    LeftHand,
    RightHand,
    BothHands,
    Ring,
    Legs,
    Feet,
    Trinket,
}

impl InventorySlot {
    pub fn as_id(&self) -> u16 {
        match self {
            InventorySlot::BothHands => 0,
            InventorySlot::Head => 1,
            InventorySlot::Amulet => 2,
            InventorySlot::Backpack => 3,
            InventorySlot::Chest => 4,
            InventorySlot::RightHand => 5,
            InventorySlot::LeftHand => 6,
            InventorySlot::Legs => 7,
            InventorySlot::Feet => 8,
            InventorySlot::Ring => 9,
            InventorySlot::Trinket => 10,
        }
    }

    pub fn from_id(id: u8) -> Option<Self> {
        match id {
            0 => Some(InventorySlot::BothHands),
            1 => Some(InventorySlot::Head),
            2 => Some(InventorySlot::Amulet),
            3 => Some(InventorySlot::Backpack),
            4 => Some(InventorySlot::Chest),
            5 => Some(InventorySlot::RightHand),
            6 => Some(InventorySlot::LeftHand),
            7 => Some(InventorySlot::Legs),
            8 => Some(InventorySlot::Feet),
            9 => Some(InventorySlot::Ring),
            10 => Some(InventorySlot::Trinket),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum ItemPlacement {
    Map {
        position: Position,
        index: usize,
    },
    Container {
        container_id: ContainerId,
        slot: usize,
    },
    Inventory {
        slot: InventorySlot,
    },
}

impl ItemPlacement {
    pub fn to_wire_position(&self) -> Position {
        match self {
            ItemPlacement::Map { position, .. } => position.clone(),
            ItemPlacement::Container { container_id, slot } => Position {
                x: CONTAINER_COORD_FLAG,
                y: *container_id,
                z: *slot as u8,
            },
            ItemPlacement::Inventory { slot } => Position {
                x: INVENTORY_COORD_FLAG,
                y: slot.as_id(),
                z: 0,
            },
        }
    }

    pub fn wire_stack_index(&self) -> u8 {
        match self {
            ItemPlacement::Map { index, .. } => *index as u8,
            _ => 0,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ItemFlag {
    Ground,
    Border,
    Container,
    Cumulative,
    Top,
    Unpass,
    Unmove,
    Take,
    FullBank,
    Bottom,
    Usable,
    MultiUse,
    ForceUse,
    Avoid,
    BlockSight,
    LiquidPool,
    LiquidContainer,
    /// Lies flat on the floor, a corpse being the case that matters. Ranked
    /// below everything that stands up; see `map::DrawRank`.
    LyingObject,
}

#[derive(Debug, Eq)]
pub struct ItemConfig {
    pub id: ItemId,
    pub flags: Vec<ItemFlag>,
    pub friction: Option<u16>,
    pub slot: Option<InventorySlot>,
    pub minimap_color: Option<u8>,
    pub elevation: Option<u8>,
}

impl ItemConfig {
    pub fn has_flag(&self, flag: ItemFlag) -> bool {
        self.flags.contains(&flag)
    }
}

impl PartialEq for ItemConfig {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub config: Arc<ItemConfig>,
    pub amount: u32,
}

impl Item {
    pub fn new(config: Arc<ItemConfig>, amount: u32) -> Self {
        Item { config, amount }
    }

    /// Whether `amount` is a countable number of items greater than one.
    ///
    /// The `Cumulative` gate is the load-bearing half: on a `LiquidPool` or
    /// `LiquidContainer` the same byte is the FLUID TYPE, so reading it as a
    /// count would label a vial of blood "5" and offer to split it into five.
    /// This is the same fork `intrinsic_patterns` makes below, and the two must
    /// stay in step -- whatever picks the count *tier* is what has a count at
    /// all.
    ///
    /// A stack of one is excluded: it has no number to show and nothing to
    /// divide.
    pub fn is_countable_stack(&self) -> bool {
        self.config.has_flag(ItemFlag::Cumulative) && self.amount > 1
    }

    /// The pattern chosen by the ITEM rather than by where it lies: a stack's
    /// count tier, or a fluid's colour. `None` when neither applies.
    ///
    /// Split out of `get_patterns` because a UI item -- in an inventory slot or
    /// a backpack -- has an amount but no position, so it cannot call that at
    /// all. Both branches here were already position-independent; only the
    /// signature made them unreachable, which is why a stack of 50 gold drew
    /// the "1" sprite everywhere except on the ground.
    pub fn intrinsic_patterns(&self, sprite: &SpriteConfig) -> Option<(u32, u32, u32)> {
        if self.config.has_flag(ItemFlag::Cumulative)
            && sprite.pattern_x == 4
            && sprite.pattern_y == 2
        {
            return Some(if self.amount < 5 {
                (self.amount.saturating_sub(1), 0, 0)
            } else if self.amount < 10 {
                (0, 1, 0)
            } else if self.amount < 25 {
                (1, 1, 0)
            } else if self.amount < 50 {
                (2, 1, 0)
            } else {
                (3, 1, 0)
            });
        }

        // A splash or fluid container indexes a COLOUR grid, so its pattern
        // comes from what it holds rather than from where it lies. The subtype
        // byte is the fluid; `amount` is that same byte, named for its other
        // meaning.
        if self.config.has_flag(ItemFlag::LiquidPool)
            || self.config.has_flag(ItemFlag::LiquidContainer)
        {
            let (x, y) = fluid_cell(self.amount as u8, sprite.pattern_x, sprite.pattern_y);
            return Some((x, y, 0));
        }

        None
    }

    /// The pattern for an item on the map, where a position is available.
    ///
    /// Intrinsic patterns win: they must precede the position rule below, which
    /// would otherwise answer and pick a count tier or a colour from the tile's
    /// coordinates.
    pub fn get_patterns(&self, pos: &Position, sprite: &SpriteConfig) -> (u32, u32, u32) {
        if let Some(patterns) = self.intrinsic_patterns(sprite) {
            return patterns;
        }

        let x = pos.x as u32 % sprite.pattern_x;
        let y = pos.y as u32 % sprite.pattern_y;
        let z = pos.z as u32 % sprite.pattern_z;
        (x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conf::map::{CONTAINER_COORD_FLAG, INVENTORY_COORD_FLAG};
    use crate::core::SpriteAnimation;
    use crate::items::fluid::FluidType;
    use bevy::prelude::Vec2;

    fn config_with(flags: Vec<ItemFlag>) -> Arc<ItemConfig> {
        Arc::new(ItemConfig {
            id: 2886,
            flags,
            friction: None,
            slot: None,
            minimap_color: None,
            elevation: None,
        })
    }

    /// A pool's appearance: a 4x3 colour grid, one layer, no animation.
    fn pool_sprite() -> SpriteConfig {
        SpriteConfig {
            id: 2886,
            group: "item-32-32-3".to_string(),
            pattern_x: 4,
            pattern_y: 3,
            pattern_z: 1,
            layers: 1,
            sprite_ids: vec![0; 12],
            animation: SpriteAnimation::Static,
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        }
    }

    /// A stack in a backpack has an amount but no position, so the UI cannot
    /// call `get_patterns`. It must still get the count tier -- this is the bug
    /// where 50 gold on the ground showed the "50" sprite and the same 50 gold
    /// in the inventory showed the "1" sprite.
    #[test]
    fn a_stacks_pattern_comes_from_its_amount_alone() {
        let sprite = SpriteConfig {
            pattern_x: 4,
            pattern_y: 2,
            ..pool_sprite()
        };
        let stack = |amount: u32| {
            Item::new(config_with(vec![ItemFlag::Cumulative]), amount).intrinsic_patterns(&sprite)
        };

        // Below 5 each count has its own sprite; above it they are tiers.
        assert_eq!(stack(1), Some((0, 0, 0)));
        assert_eq!(stack(4), Some((3, 0, 0)));
        assert_eq!(stack(5), Some((0, 1, 0)));
        assert_eq!(stack(50), Some((3, 1, 0)));
    }

    /// The two gates on a count. The vial of blood is the case that matters:
    /// its `amount` is a fluid type, so reading it as a count would label it
    /// "5" and offer to divide it into five.
    #[test]
    fn only_a_cumulative_item_of_more_than_one_has_a_count() {
        let stack = |flags, amount| Item::new(config_with(flags), amount).is_countable_stack();

        assert!(stack(vec![ItemFlag::Cumulative], 50));
        assert!(!stack(vec![ItemFlag::Cumulative], 1));
        assert!(!stack(vec![ItemFlag::LiquidContainer], 5));
        assert!(!stack(vec![ItemFlag::LiquidPool], 5));
        assert!(!stack(vec![ItemFlag::Take], 1));
    }

    /// An item whose sprite depends on neither its amount nor its contents has
    /// nothing to say here, and the caller falls back to position.
    #[test]
    fn an_ordinary_item_has_no_intrinsic_pattern() {
        let plain = Item::new(config_with(vec![ItemFlag::Take]), 1);

        assert_eq!(plain.intrinsic_patterns(&pool_sprite()), None);
    }

    /// A pool's 4x3 grid is a colour grid. Blood is fluid 5, red is colour 2,
    /// and the cell is (2, 0).
    #[test]
    fn a_pool_draws_the_cell_for_its_fluid() {
        let pool = Item::new(
            config_with(vec![ItemFlag::LiquidPool]),
            FluidType::Blood as u32,
        );

        assert_eq!(
            pool.get_patterns(&Position::new(100, 200, 7), &pool_sprite()),
            (2, 0, 0)
        );
    }

    /// The branch has to actually be entered. Without it a pool falls through to
    /// the position rule and its colour is chosen by its tile -- blood here,
    /// something else one step east. Two positions, one fluid, one answer.
    #[test]
    fn a_pools_colour_does_not_depend_on_where_it_lies() {
        let pool = Item::new(
            config_with(vec![ItemFlag::LiquidPool]),
            FluidType::Blood as u32,
        );
        let sprite = pool_sprite();

        let here = pool.get_patterns(&Position::new(100, 200, 7), &sprite);
        let one_step_east = pool.get_patterns(&Position::new(101, 200, 7), &sprite);

        assert_eq!(here, one_step_east);
    }

    #[test]
    fn map_placement_encodes_as_its_position() {
        let p = ItemPlacement::Map {
            position: Position {
                x: 100,
                y: 200,
                z: 7,
            },
            index: 3,
        };
        assert_eq!(
            p.to_wire_position(),
            Position {
                x: 100,
                y: 200,
                z: 7
            }
        );
        assert_eq!(p.wire_stack_index(), 3);
    }

    #[test]
    fn container_placement_encodes_flag_id_slot() {
        let p = ItemPlacement::Container {
            container_id: 5,
            slot: 9,
        };
        assert_eq!(
            p.to_wire_position(),
            Position {
                x: CONTAINER_COORD_FLAG,
                y: 5,
                z: 9
            }
        );
        assert_eq!(p.wire_stack_index(), 0);
    }

    #[test]
    fn inventory_placement_encodes_flag_slot() {
        let p = ItemPlacement::Inventory {
            slot: InventorySlot::Head,
        };
        assert_eq!(
            p.to_wire_position(),
            Position {
                x: INVENTORY_COORD_FLAG,
                y: InventorySlot::Head.as_id(),
                z: 0
            }
        );
        assert_eq!(p.wire_stack_index(), 0);
    }
}
