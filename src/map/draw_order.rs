//! Tibia's draw order, packed into an integer.
//!
//! Bevy 2D has no ordering knob other than `Transform.translation.z`:
//! `queue_material2d_meshes` reads the composed world z into a `Transparent2d`
//! sort key, `extract_sprites` does the same for `Sprite`, and `radsort` orders
//! the phase by it. So z here is not a depth, it is a draw index — an
//! alpha-blended 2D mesh never writes the depth buffer, so nothing is ever
//! depth-tested against anything.
//!
//! Every integer below 2^24 is exactly representable in `f32`, so a key built by
//! integer arithmetic makes that sort exact and total, with no tie for the
//! stable sort to break in entity-iteration order.
//!
//! ```text
//! ((((MAX_FLOOR - z) * RANKS + rank) * ROWS + row) * COLS + col) * TILE_SPAN
//!     + layer * SLOT_COUNT + slot
//! ```
//!
//! Tiles run left to right then top to bottom, and a tile further down-and-right
//! draws over one up-and-left. That direction follows from the sprites: a 64 px
//! quad centres on its tile's top-left corner, so it covers its own tile plus
//! the three above and to the left, and nothing ever overflows down-right.
//!
//! `row`/`col` are **viewport-relative**, which is what keeps the key inside
//! 2^24 — absolute map coordinates are `u16` each and would need 36 bits before
//! any rank, layer or slot. The cost is that every key changes when the viewport
//! moves, which is what [`DrawOrigin`] and [`apply_draw_order`] exist for.

use bevy::prelude::*;

use crate::conf::draw_order::{LAYER_COUNT, SLOT_COUNT, VIEW_MARGIN_TILES};
use crate::conf::map::{MAX_FLOOR, TILE_SIZE, TILES_X, TILES_Y};
use crate::map::Position;
use crate::player::components::Player;

/// Which whole-floor pass a drawable belongs to. The discriminants **are** the
/// order, and this outranks the tile: everything in an earlier rank is drawn
/// before everything in a later one, wherever on the floor it sits.
///
/// It carries the rules that are not positional. "A creature draws over a dead
/// body whatever square it stands on" is one, and no arrangement of tiles can
/// state it: a 2x2 corpse is anchored at its bottom-right tile and spreads up
/// and left over tiles that draw before it, so by position it always wins.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum DrawRank {
    /// Grounds and borders. Flat, one per tile, never overlapping and never
    /// behind anything, so a whole floor's worth can go first — which is also
    /// what makes a creature impossible to slice with a ground.
    Ground = 0,
    /// Corpses and liquid pools. Above every ground, below everything that
    /// stands up. Membership is per item, not per layer; see
    /// `items::instancing::placement`.
    Lying = 1,
    /// Everything that stands up. Ordered by tile, which is where all the
    /// positional rules live.
    Standing = 2,
}

/// What a drawable is, within its tile. The discriminants **are** the order.
///
/// `Target` sits under `Creature` because OTClient draws the attack square
/// before the outfit, and `Missile` above `Top` because a missile flies over
/// everything on the tile it is currently crossing.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum DrawLayer {
    Ground = 0,
    Border = 1,
    /// A creature crossing a tile it never stands on: the corner tile of a
    /// north-east or south-west step. It passes BEHIND what stands there rather
    /// than in front of it, so it goes over that tile's ground and under its
    /// walls.
    PassingThrough = 2,
    Bottom = 3,
    Items = 4,
    Target = 5,
    Creature = 6,
    Effect = 7,
    Top = 8,
    Missile = 9,
}

const FLOOR_COUNT: i32 = (MAX_FLOOR + 1) as i32;
/// One more than the largest `DrawRank`, with a spare.
const RANKS: i32 = 4;
const MARGIN: i32 = VIEW_MARGIN_TILES as i32;
const COLS: i32 = TILES_X as i32 + 2 * MARGIN;
const ROWS: i32 = TILES_Y as i32 + 2 * MARGIN;
/// Keys per tile: one per (layer, slot) pair.
const TILE_SPAN: i32 = (LAYER_COUNT * SLOT_COUNT) as i32;

/// One past the largest key this scheme can produce. The game camera's near and
/// far planes are set from it — a key outside them is clipped, silently.
pub const DRAW_KEY_MAX: f32 = (FLOOR_COUNT * RANKS * ROWS * COLS * TILE_SPAN) as f32;

/// The tile at the top-left of the drawn window, and the floor it was measured
/// on. Mirrors `map::events::iter_viewport`, including its per-floor parallax:
/// the window on floor `f` is shifted by `z - f` tiles, because `to_world`
/// shifts that floor's tiles by the same amount.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrawOrigin {
    pub x: i32,
    pub y: i32,
    pub z: u8,
}

impl DrawOrigin {
    pub fn around(pos: &Position) -> Self {
        DrawOrigin {
            x: pos.x as i32 - (TILES_X / 2) as i32,
            y: pos.y as i32 - (TILES_Y / 2) as i32,
            z: pos.z,
        }
    }
}

/// Where a drawable sits in the draw order. The **only** thing that puts a z on
/// a game-world entity is [`apply_draw_order`] reading this.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct DrawOrder {
    pub pos: Position,
    rank: DrawRank,
    offset: f32,
}

impl DrawOrder {
    pub fn new(pos: Position, rank: DrawRank, layer: DrawLayer, slot: u32) -> Self {
        DrawOrder {
            pos,
            rank,
            offset: layer_offset(layer, slot),
        }
    }

    pub fn key(&self, origin: &DrawOrigin) -> f32 {
        tile_base(origin, &self.pos, self.rank) + self.offset
    }

    /// Leaves the component clean when neither argument changed. `move_agent`
    /// calls this every frame of every step, and a dirty `DrawOrder` costs a
    /// transform write and a hierarchy re-propagation.
    pub fn move_to(this: &mut Mut<DrawOrder>, pos: &Position, layer: DrawLayer) {
        let offset = layer_offset(layer, 0);
        if this.pos != *pos || this.offset != offset {
            this.pos = pos.clone();
            this.offset = offset;
        }
    }
}

/// A tile's base key: its slot in the viewport window, times the keys per tile.
///
/// Clamped rather than wrapped. A drawable outside the window keys as if it
/// were on the edge tile, which can order it wrongly against its neighbour —
/// but it is off-screen, and the alternative is a key that collides with a tile
/// on the far side of the map.
fn tile_base(origin: &DrawOrigin, pos: &Position, rank: DrawRank) -> f32 {
    let parallax = origin.z as i32 - pos.z as i32;
    let col = (pos.x as i32 - (origin.x + parallax) + MARGIN).clamp(0, COLS - 1);
    let row = (pos.y as i32 - (origin.y + parallax) + MARGIN).clamp(0, ROWS - 1);
    let floor = (MAX_FLOOR as i32 - pos.z as i32).clamp(0, FLOOR_COUNT - 1);

    (((((floor * RANKS + rank as i32) * ROWS) + row) * COLS + col) * TILE_SPAN) as f32
}

/// A drawable's position within its tile. `slot` separates items in one stack;
/// it saturates rather than wrapping, because `Map::get_items` caps nothing and
/// a wrapped slot would put the 17th item under the ground.
fn layer_offset(layer: DrawLayer, slot: u32) -> f32 {
    (layer as u32 * SLOT_COUNT + slot.min(SLOT_COUNT - 1)) as f32
}

/// The tile a creature draws with: the one holding the bottom-right corner of
/// the tile-sized rect its sprite occupies, which is the greatest row and column
/// the sprite covers. No tile it overlaps can draw after it, so none can cut it.
///
/// This is OTClient's `Creature::updateWalkingTile`, which re-homes a walking
/// creature onto that tile every frame of a step and draws it during that tile's
/// pass, offset back to its true position.
///
/// `anchor` is the creature's world position WITHOUT elevation, which shifts a
/// sprite up and left and could only pick an earlier tile.
pub fn drawn_tile(anchor: Vec2, floor: u8) -> Position {
    // A pixel back inside the rect. `to_world` returns a tile's top-left corner
    // and the quad spans one tile down and right, so the boundary itself floors
    // into the next tile and a standing creature claims the one below-right.
    Position::from_world(
        anchor + Vec2::new(TILE_SIZE - 1.0, -(TILE_SIZE - 1.0)),
        floor,
    )
}

/// The local z of a child that shares its parent's tile but not its layer.
///
/// The attack square is a child of the agent, whose transform already carries
/// the whole key. Hierarchies compose additively, so this is the DIFFERENCE
/// between the two layers; the target layer's own offset would put the square in
/// front of the creature instead of under it.
pub const TARGET_SQUARE_LOCAL_Z: f32 =
    (DrawLayer::Target as u32 as f32 - DrawLayer::Creature as u32 as f32) * SLOT_COUNT as f32;

/// Recomputes the viewport anchor when the player moves. Every key is relative
/// to it, so this changing is what makes [`apply_draw_order`] rewrite the world.
pub fn update_draw_origin(
    player: Query<&Position, (With<Player>, Changed<Position>)>,
    mut origin: ResMut<DrawOrigin>,
) {
    let Ok(pos) = player.single() else {
        return;
    };
    let next = DrawOrigin::around(pos);
    // `Position` is reinserted at its current value on arrival, and an
    // unconditional write here would rewrite every z in the world each time.
    if *origin != next {
        *origin = next;
    }
}

/// Writes the draw order into `Transform.translation.z`. The one place in the
/// client that decides what draws over what.
pub fn apply_draw_order(
    origin: Res<DrawOrigin>,
    mut drawables: Query<(Ref<DrawOrder>, &mut Transform)>,
) {
    let origin_changed = origin.is_changed();
    for (order, mut transform) in &mut drawables {
        if !origin_changed && !order.is_changed() {
            continue;
        }
        let key = order.key(&origin);
        // `Transform` is change-detected; a blind write would re-propagate the
        // whole hierarchy on every viewport move.
        if transform.translation.z != key {
            transform.translation.z = key;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> DrawOrigin {
        DrawOrigin::around(&Position::new(1000, 1000, 7))
    }

    fn key(pos: &Position, layer: DrawLayer, slot: u32) -> f32 {
        ranked(pos, DrawRank::Standing, layer, slot)
    }

    fn ranked(pos: &Position, rank: DrawRank, layer: DrawLayer, slot: u32) -> f32 {
        DrawOrder::new(pos.clone(), rank, layer, slot).key(&origin())
    }

    /// The whole scheme rests on this: every key an `f32` holds exactly, so the
    /// radix sort over them is an exact integer sort with no ties to break.
    #[test]
    fn every_key_is_exactly_representable_in_f32() {
        assert!(
            DRAW_KEY_MAX <= (1u32 << 24) as f32,
            "keys must stay under 2^24, got {DRAW_KEY_MAX}"
        );
        // And every layer fits the space reserved for it, or `layer_offset`
        // would spill one layer into the next tile.
        assert!((DrawLayer::Missile as u32) < LAYER_COUNT);
    }

    /// Tile-outer: everything on a tile draws before anything on the next tile,
    /// down to that tile's ground covering the previous tile's top item.
    #[test]
    fn a_whole_tile_is_drawn_before_the_next_one_starts() {
        let here = Position::new(1000, 1000, 7);
        let east = Position::new(1001, 1000, 7);

        assert!(key(&here, DrawLayer::Top, 15) < key(&east, DrawLayer::Ground, 0));
    }

    /// Row-major, not diagonal. A key that summed the two axes would give
    /// `(x+1, y-1)` and `(x, y)` the same value, leaving the stable sort to
    /// break the tie in entity-iteration order.
    #[test]
    fn tiles_run_left_to_right_then_top_to_bottom() {
        let here = Position::new(1000, 1000, 7);
        let east = Position::new(1001, 1000, 7);
        let north_east = Position::new(1001, 999, 7);
        let south_west = Position::new(999, 1001, 7);

        assert!(key(&here, DrawLayer::Ground, 0) < key(&east, DrawLayer::Ground, 0));
        // The row wins over the column: a tile one row down draws later than one
        // a row up, whatever its column.
        assert!(key(&north_east, DrawLayer::Ground, 0) < key(&here, DrawLayer::Ground, 0));
        assert!(key(&here, DrawLayer::Ground, 0) < key(&south_west, DrawLayer::Ground, 0));
    }

    /// Within one tile, the order the game is specified in.
    #[test]
    fn a_tiles_layers_run_ground_to_missile() {
        let pos = Position::new(1000, 1000, 7);
        let layers = [
            DrawLayer::Ground,
            DrawLayer::Border,
            DrawLayer::PassingThrough,
            DrawLayer::Bottom,
            DrawLayer::Items,
            DrawLayer::Target,
            DrawLayer::Creature,
            DrawLayer::Effect,
            DrawLayer::Top,
            DrawLayer::Missile,
        ];

        for pair in layers.windows(2) {
            assert!(
                key(&pos, pair[0], 15) < key(&pos, pair[1], 0),
                "{:?} must draw before {:?}, even under a full stack",
                pair[0],
                pair[1]
            );
        }
    }

    /// A deeper floor is drawn first, so the floor above covers it. Floor 0 is
    /// the sky and draws last.
    #[test]
    fn deeper_floors_are_drawn_first() {
        let deep = Position::new(1000, 1000, 9);
        let shallow = Position::new(1000, 1000, 5);

        assert!(key(&deep, DrawLayer::Missile, 15) < key(&shallow, DrawLayer::Ground, 0));
    }

    /// A whole floor's keys must fit between its neighbours': a tile at the far
    /// corner of one floor may not reach the next floor's first tile. A key
    /// built from absolute map coordinates loses this as they grow, which is
    /// what the viewport-relative window prevents.
    #[test]
    fn a_floors_widest_tile_stays_inside_its_floor() {
        let corner = Position::new(1000 + 40, 1000 + 40, 7);
        let next_floor_first = Position::new(0, 0, 6);

        assert!(
            key(&corner, DrawLayer::Missile, 15) < key(&next_floor_first, DrawLayer::Ground, 0)
        );
    }

    /// The parallax is the reason a floor's window is not the player's window:
    /// `to_world` shifts floor `f` by `z - f` tiles, and the key has to shift
    /// with it, or two tiles drawn on the same screen row would key to
    /// different rows.
    #[test]
    fn a_higher_floor_keys_by_its_own_shifted_window() {
        let origin = origin(); // player on floor 7 at (1000, 1000)
        // One floor up the window slides one tile down-right, so (1001, 1001, 6)
        // occupies the screen cell that (1000, 1000, 7) does.
        let here = DrawOrder::new(
            Position::new(1000, 1000, 7),
            DrawRank::Ground,
            DrawLayer::Ground,
            0,
        );
        let above = DrawOrder::new(
            Position::new(1001, 1001, 6),
            DrawRank::Ground,
            DrawLayer::Ground,
            0,
        );

        // Same cell within their own floors: the gap is exactly one floor, and
        // a floor is now RANKS windows wide.
        let gap = above.key(&origin) - here.key(&origin);
        assert_eq!(gap, (RANKS * ROWS * COLS * TILE_SPAN) as f32);

        // Without the parallax, the same-screen-cell tile would key a row and a
        // column further on, which is what the assertion above pins.
        let unshifted = DrawOrder::new(
            Position::new(1000, 1000, 6),
            DrawRank::Ground,
            DrawLayer::Ground,
            0,
        );
        assert_ne!(unshifted.key(&origin) - here.key(&origin), gap);
    }

    /// The square is spawned as a child of the agent, whose transform already
    /// carries the whole key, so its local z is the layer difference. This drove
    /// a real bug once: using the target layer's own offset put the square in
    /// front of the creature.
    #[test]
    fn the_target_square_composes_to_just_under_its_agent() {
        let pos = Position::new(1000, 1000, 7);
        let agent = key(&pos, DrawLayer::Creature, 0);

        assert_eq!(
            agent + TARGET_SQUARE_LOCAL_Z,
            key(&pos, DrawLayer::Target, 0)
        );
        assert!(agent + TARGET_SQUARE_LOCAL_Z < agent);
    }

    /// The sample is pulled a pixel inside the rect. On the boundary it floors
    /// into the next tile, and a creature standing still would claim the tile
    /// below and right of the one it is actually on.
    #[test]
    fn a_standing_creature_draws_with_its_own_tile() {
        for pos in [
            Position::new(1000, 1000, 7),
            Position::new(0, 0, 7),
            Position::new(1000, 1000, 3),
        ] {
            assert_eq!(drawn_tile(pos.to_world().truncate(), pos.z), pos);
        }
    }

    /// Walking south the sprite crosses into the destination almost at once, so
    /// the creature claims it at once; walking north it keeps its own tile until
    /// the sprite has all but left it. The asymmetry is the point — it is what
    /// stops the ground of either tile from cutting the sprite.
    #[test]
    fn a_cardinal_step_claims_a_tile_when_the_sprite_reaches_it() {
        let here = Position::new(1000, 1000, 7);

        let at = |to: &Position, f: f32| {
            drawn_tile(here.to_world().lerp(to.to_world(), f).truncate(), here.z)
        };

        let south = Position::new(1000, 1001, 7);
        assert_eq!(at(&south, 0.0), here);
        assert_eq!(
            at(&south, 0.1),
            south,
            "claimed as soon as the sprite enters"
        );

        let north = Position::new(1000, 999, 7);
        assert_eq!(at(&north, 0.0), here);
        assert_eq!(
            at(&north, 0.5),
            here,
            "still overlapping the tile it leaves"
        );
        assert_eq!(at(&north, 1.0), north);

        let east = Position::new(1001, 1000, 7);
        assert_eq!(at(&east, 0.1), east);

        let west = Position::new(999, 1000, 7);
        assert_eq!(at(&west, 0.5), here);
        assert_eq!(at(&west, 1.0), west);
    }

    /// The case a per-step rule cannot express: walking north-east the sprite
    /// passes THROUGH the tile east of the start, which is on neither end of the
    /// step, and lets it go again before it arrives.
    #[test]
    fn a_diagonal_passes_through_a_tile_it_never_stands_on() {
        let here = Position::new(1000, 1000, 7);
        let north_east = Position::new(1001, 999, 7);

        let at = |f: f32| {
            drawn_tile(
                here.to_world().lerp(north_east.to_world(), f).truncate(),
                here.z,
            )
        };

        assert_eq!(at(0.0), here);
        assert_eq!(
            at(0.5),
            Position::new(1001, 1000, 7),
            "mid-step, a tile on neither end of the step"
        );
        assert_eq!(at(1.0), north_east, "and released before it arrives");
    }

    /// The invariant behind all of it, asserted over whole steps rather than at
    /// a chosen fraction: at no point may a tile the sprite is CURRENTLY over
    /// draw above the creature, ground or items alike.
    ///
    /// The covered set is derived from the sprite's pixel rect — the tiles
    /// holding its four corners — and not from `drawn_tile`, which would make
    /// the test assert `f(x) == f(x)`. Three of the four corners are sampled by
    /// an expression `drawn_tile` does not use, so picking the wrong corner (the
    /// top-left, or the centre) fails here.
    #[test]
    fn nothing_the_sprite_covers_ever_draws_over_the_creature() {
        let origin = origin();
        let here = Position::new(1000, 1000, 7);
        let inside = TILE_SIZE - 1.0;

        for to in [
            Position::new(1000, 1001, 7),
            Position::new(1000, 999, 7),
            Position::new(1001, 1000, 7),
            Position::new(999, 1000, 7),
            Position::new(1001, 999, 7),
            Position::new(999, 1001, 7),
            Position::new(1001, 1001, 7),
            Position::new(999, 999, 7),
        ] {
            for step in 0..=40 {
                let f = step as f32 / 40.0;
                let anchor = here.to_world().lerp(to.to_world(), f).truncate();

                // The sprite's rect is one tile, anchored at its top-left
                // corner; world y grows upward while tile y grows down.
                let covered = [
                    Position::from_world(anchor, here.z),
                    Position::from_world(anchor + Vec2::new(inside, 0.0), here.z),
                    Position::from_world(anchor + Vec2::new(0.0, -inside), here.z),
                    Position::from_world(anchor + Vec2::new(inside, -inside), here.z),
                ];

                let creature = DrawOrder::new(
                    drawn_tile(anchor, here.z),
                    DrawRank::Standing,
                    DrawLayer::Creature,
                    0,
                )
                .key(&origin);

                for tile in &covered {
                    for layer in [DrawLayer::Ground, DrawLayer::Border, DrawLayer::Items] {
                        assert!(
                            key(tile, layer, 15) < creature,
                            "stepping {here} -> {to} at f={f}: {tile} {layer:?} draws over it"
                        );
                    }
                }
            }
        }
    }

    /// The other half of the contract: the creature must not claim a tile it is
    /// not over. Holding one is what made a diagonal walker pass in front of the
    /// corner tile's items for the whole step.
    #[test]
    fn a_creature_claims_no_tile_its_sprite_has_not_reached() {
        let here = Position::new(1000, 1000, 7);
        let north_east = Position::new(1001, 999, 7);
        let inside = TILE_SIZE - 1.0;

        for step in 0..=40 {
            let f = step as f32 / 40.0;
            let anchor = here.to_world().lerp(north_east.to_world(), f).truncate();
            let claimed = drawn_tile(anchor, here.z);

            let covered = [
                Position::from_world(anchor, here.z),
                Position::from_world(anchor + Vec2::new(inside, 0.0), here.z),
                Position::from_world(anchor + Vec2::new(0.0, -inside), here.z),
                Position::from_world(anchor + Vec2::new(inside, -inside), here.z),
            ];

            assert!(
                covered.contains(&claimed),
                "at f={f} the creature claims {claimed}, which its sprite does not cover"
            );
        }
    }

    /// The rule that is not positional, and the reason `DrawRank` exists: in
    /// Tibia a creature is over a dead body whatever square it stands on. The
    /// corpse is checked from every tile around it, including the ones it draws
    /// AFTER — which is every arrangement a tile-ordered key gets wrong.
    #[test]
    fn a_creature_draws_over_a_corpse_from_any_square() {
        let corpse_tile = Position::new(1000, 1000, 7);
        let corpse = ranked(&corpse_tile, DrawRank::Lying, DrawLayer::Items, 0);

        for dy in -2i32..=2 {
            for dx in -2i32..=2 {
                let stood = Position::new((1000 + dx) as u16, (1000 + dy) as u16, 7);
                assert!(
                    ranked(&stood, DrawRank::Standing, DrawLayer::Creature, 0) > corpse,
                    "a creature on {stood} must draw over the corpse on {corpse_tile}"
                );
            }
        }
    }

    /// And the constraint that stops the rank being pushed any lower: a corpse
    /// draws over its own blood. The pool carries `bottom` in the appearance
    /// data, so it shares the `Lying` rank and the two are separated by their
    /// layers, not by the rank.
    #[test]
    fn a_corpse_draws_over_its_own_blood_pool() {
        let tile = Position::new(1000, 1000, 7);

        let pool = ranked(&tile, DrawRank::Lying, DrawLayer::Bottom, 0);
        let corpse = ranked(&tile, DrawRank::Lying, DrawLayer::Items, 0);
        let ground = ranked(&tile, DrawRank::Ground, DrawLayer::Ground, 0);

        assert!(ground < pool, "the pool lies on the ground");
        assert!(pool < corpse, "and the body lies on the pool");
    }

    /// A corpse spreads up and left over tiles that draw before its own, and
    /// must still clear their grounds — the whole reason it cannot simply be
    /// keyed to an earlier tile.
    #[test]
    fn a_corpse_clears_the_ground_of_every_tile_it_spreads_over() {
        let corpse_tile = Position::new(1000, 1000, 7);
        let corpse = ranked(&corpse_tile, DrawRank::Lying, DrawLayer::Items, 0);

        for tile in [
            corpse_tile.clone(),
            Position::new(999, 1000, 7),
            Position::new(1000, 999, 7),
            Position::new(999, 999, 7),
            // And the tiles it does NOT reach, which it must also clear: every
            // ground on the floor is in an earlier rank.
            Position::new(1005, 1005, 7),
        ] {
            for layer in [DrawLayer::Ground, DrawLayer::Border] {
                assert!(
                    ranked(&tile, DrawRank::Ground, layer, 15) < corpse,
                    "{tile} {layer:?} must draw under a corpse anywhere on the floor"
                );
            }
        }
    }

    /// The rank must not swallow the positional rules. A wall on a later tile
    /// still occludes a creature on an earlier one — both stand up, so both are
    /// in the same rank and the tile decides, exactly as before.
    #[test]
    fn a_wall_still_occludes_a_creature_behind_it() {
        let behind = Position::new(1000, 1000, 7);
        let wall_tile = Position::new(1000, 1001, 7);

        assert!(
            ranked(&behind, DrawRank::Standing, DrawLayer::Creature, 0)
                < ranked(&wall_tile, DrawRank::Standing, DrawLayer::Bottom, 0)
        );
    }

    /// And the rank must not reach across floors: a deeper floor is drawn first
    /// in its entirety, so a creature down there cannot climb over a corpse on
    /// the floor above by rank alone.
    #[test]
    fn a_rank_never_outranks_a_floor() {
        let deep = Position::new(1000, 1000, 9);
        let shallow = Position::new(1000, 1000, 8);

        assert!(
            ranked(&deep, DrawRank::Standing, DrawLayer::Missile, 15)
                < ranked(&shallow, DrawRank::Ground, DrawLayer::Ground, 0),
            "everything on the deeper floor draws before anything on the one above"
        );
    }

    /// `Map::get_items` caps nothing, so a deep stack must saturate. Wrapping
    /// would put the 17th item under the tile's ground.
    #[test]
    fn a_stack_slot_saturates_instead_of_wrapping() {
        let pos = Position::new(1000, 1000, 7);

        assert_eq!(
            key(&pos, DrawLayer::Items, 99),
            key(&pos, DrawLayer::Items, 15)
        );
        assert!(key(&pos, DrawLayer::Items, 99) < key(&pos, DrawLayer::Target, 0));
    }

    /// Off-window tiles clamp to the edge rather than wrapping into a key that
    /// belongs to a tile on the other side of the map.
    #[test]
    fn a_tile_far_outside_the_window_clamps_to_the_edge() {
        let far = Position::new(64000, 64000, 7);
        let edge = Position::new(
            (1000 - (TILES_X / 2) as u16) + (COLS - 1 - MARGIN) as u16,
            (1000 - (TILES_Y / 2) as u16) + (ROWS - 1 - MARGIN) as u16,
            7,
        );

        assert_eq!(
            key(&far, DrawLayer::Ground, 0),
            key(&edge, DrawLayer::Ground, 0)
        );
    }
}
