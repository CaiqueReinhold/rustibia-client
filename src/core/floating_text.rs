//! World-anchored transient text: damage numbers and speech over a tile.
//! Distinct from `core::text`, which anchors to the *viewport*.
//!
//! Each text is a `Text2d` child of a root that sits at its anchor, in world units,
//! for life. The root is `HudScaled`, so the child's local translation, the rise and
//! the collision push, is in logical pixels.

use std::collections::VecDeque;
use std::time::Duration;

use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSmoothing, TextBounds, TextLayoutInfo};
use bevy_text_outline::TextOutline;

use crate::conf::floating_text as ft;
use crate::conf::map::TILE_SIZE;
use crate::conf::ui::chat::CREATURE_SAY_COLOR;
use crate::conf::ui::chat::LOCAL_CHANNEL_COLOR;
use crate::game_ui::{GameUiAssets, GameViewport};
use crate::map::Position;
use crate::network::events::ShowFloatingText;
use crate::overlay::{HudScaled, OVERLAY_TEXT_Z, on_overlay_layer, world_hud_scale};
use crate::player::components::Player;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatingTextType {
    HitPoints,
    PlayerMessage,
    CreatureSay,
}

/// State shared by both kinds.
#[derive(Component, Debug)]
pub struct FloatingText {
    pub kind: FloatingTextType,
    pub speaker: Option<String>,
    /// The tile this text is pinned to, for life.
    pub anchor: Position,
    /// `Time::elapsed` at spawn. The collision sort key.
    pub spawned_at: Duration,
    /// Collision-resolved push, logical px, upward. Vertical only.
    pub offset_y: f32,
}

#[derive(Component, Debug)]
pub struct FloatingTextRoot;

#[derive(Component, Debug)]
pub struct HitPointsText {
    /// Parsed once at spawn. `None` means the text is not a number and can never
    /// merge, so the merge path never re-parses.
    pub value: Option<i64>,
    pub color: Color,
    pub timer: Timer,
}

#[derive(Component, Debug)]
pub struct SpeechBlock {
    pub lines: VecDeque<(String, Timer)>,
}

impl SpeechBlock {
    pub fn compose(&self) -> String {
        self.lines
            .iter()
            .map(|(line, _)| line.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn resolve_color(kind: FloatingTextType, color: Option<(u8, u8, u8)>) -> Color {
    match color {
        Some((r, g, b)) => Color::srgb_u8(r, g, b),
        None => match kind {
            FloatingTextType::HitPoints => Color::WHITE,
            FloatingTextType::PlayerMessage => Color::from(LOCAL_CHANNEL_COLOR),
            FloatingTextType::CreatureSay => Color::from(CREATURE_SAY_COLOR),
        },
    }
}

pub fn line_duration(chars: usize) -> Duration {
    let ms = (ft::SPEECH_MS_PER_CHAR * chars as u64).clamp(ft::SPEECH_MIN_MS, ft::SPEECH_MAX_MS);
    Duration::from_millis(ms)
}

pub fn risen(fraction: f32) -> f32 {
    ft::HP_RISE_PX * fraction
}

pub fn alpha(fraction: f32) -> f32 {
    if fraction < ft::HP_FADE_START {
        return 1.0;
    }
    ((1.0 - fraction) / (1.0 - ft::HP_FADE_START)).clamp(0.0, 1.0)
}

/// Where to place an arriving damage number, given the current heights above the
/// tile of every live number already there.
pub fn stagger_offset(heights: &[f32]) -> f32 {
    if heights.iter().all(|h| *h >= ft::HP_CLEARANCE_PX) {
        return 0.0;
    }
    let highest = heights.iter().copied().fold(0.0f32, f32::max);
    let candidate = highest + ft::HP_CLEARANCE_PX;
    if candidate > ft::HP_MAX_STAGGER_PX {
        return 0.0;
    }
    candidate
}

pub struct LiveHitPoints {
    pub entity: Entity,
    pub offset_y: f32,
    pub value: Option<i64>,
    pub color: Color,
    pub fraction: f32,
    pub spawned_at: Duration,
}

pub enum HpArrival {
    /// Fold the arriving number into an existing one.
    Merge { target: Entity, sum: i64 },
    /// Spawn a new number at this vertical offset.
    Spawn { offset_y: f32 },
}

/// Decide what an arriving damage number does. `live` must already be filtered to
/// the numbers sharing the arriving text's tile.
///
/// Merge candidates are considered newest first, so a burst collapses into the
/// freshest number, but an older text is still used when the newest cannot absorb
/// the hit.
pub fn plan_hit_points(live: &[LiveHitPoints], value: Option<i64>, color: Color) -> HpArrival {
    if let Some(v) = value {
        let mut newest_first: Vec<&LiveHitPoints> = live.iter().collect();
        newest_first.sort_by_key(|o| std::cmp::Reverse(o.spawned_at));
        for other in newest_first {
            if other.color == color
                && other.fraction < ft::HP_MERGE_WINDOW
                && let Some(existing) = other.value
            {
                return HpArrival::Merge {
                    target: other.entity,
                    sum: existing + v,
                };
            }
        }
    }

    let heights: Vec<f32> = live
        .iter()
        .map(|o| o.offset_y + risen(o.fraction))
        .collect();
    HpArrival::Spawn {
        offset_y: stagger_offset(&heights),
    }
}

pub struct BlockLayout {
    pub anchor_px: Vec2,
    pub size: Vec2,
    pub spawned_at: Duration,
}

fn block_rect(b: &BlockLayout, offset_y: f32) -> Rect {
    // y-down: pushing a block up subtracts from its bottom edge.
    let bottom = b.anchor_px.y - offset_y;
    Rect {
        min: Vec2::new(b.anchor_px.x - b.size.x / 2.0, bottom - b.size.y),
        max: Vec2::new(b.anchor_px.x + b.size.x / 2.0, bottom),
    }
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.min.x < b.max.x && b.min.x < a.max.x && a.min.y < b.max.y && b.min.y < a.max.y
}

pub fn resolve_offsets(blocks: &[BlockLayout]) -> Vec<f32> {
    let mut order: Vec<usize> = (0..blocks.len()).collect();
    order.sort_by_key(|&i| blocks[i].spawned_at);

    let mut offsets = vec![0.0f32; blocks.len()];
    let mut placed: Vec<Rect> = Vec::with_capacity(blocks.len());

    for &i in &order {
        let b = &blocks[i];
        let mut offset_y = 0.0f32;
        for _ in 0..=placed.len() {
            let rect = block_rect(b, offset_y);
            let Some(blocker) = placed.iter().find(|p| overlaps(&rect, p)) else {
                break;
            };
            offset_y += (rect.max.y - blocker.min.y) + ft::SPEECH_GAP_PX;
        }
        offsets[i] = offset_y;
        placed.push(block_rect(b, offset_y));
    }

    offsets
}

pub fn on_floating_text(
    event: On<ShowFloatingText>,
    mut commands: Commands,
    time: Res<Time>,
    ui_assets: Res<GameUiAssets>,
    mut hp_q: Query<(Entity, &FloatingText, &mut HitPointsText, &mut Text2d), Without<SpeechBlock>>,
    mut speech_q: Query<
        (Entity, &FloatingText, &mut SpeechBlock, &mut Text2d),
        Without<HitPointsText>,
    >,
) {
    // The anchor is the tile the server named, never the speaker's current
    // position: text outlives its speaker, and the killing blow's damage number
    // arrives for an agent the next message removes.
    let anchor = event.position.clone();
    let now = time.elapsed();
    let color = resolve_color(event.text_type, event.color);

    match event.text_type {
        FloatingTextType::HitPoints => {
            let value = event.text.trim().parse::<i64>().ok();

            // Snapshot before any mutation, so planning stays a pure function.
            let live: Vec<LiveHitPoints> = hp_q
                .iter()
                .filter(|(_, ft, _, _)| ft.anchor == anchor)
                .map(|(entity, ft, hp, _)| LiveHitPoints {
                    entity,
                    offset_y: ft.offset_y,
                    value: hp.value,
                    color: hp.color,
                    fraction: hp.timer.fraction(),
                    spawned_at: ft.spawned_at,
                })
                .collect();

            match plan_hit_points(&live, value, color) {
                HpArrival::Merge { target, sum } => {
                    if let Ok((_, _, mut hp, mut text)) = hp_q.get_mut(target) {
                        hp.value = Some(sum);
                        text.0 = sum.to_string();
                    }
                }
                HpArrival::Spawn { offset_y } => {
                    spawn_floating_text(
                        &mut commands,
                        FloatingText {
                            kind: FloatingTextType::HitPoints,
                            speaker: event.speaker.clone(),
                            anchor: anchor.clone(),
                            spawned_at: now,
                            offset_y,
                        },
                        (
                            HitPointsText {
                                value,
                                color,
                                timer: Timer::new(
                                    Duration::from_millis(ft::HP_DURATION_MS),
                                    TimerMode::Once,
                                ),
                            },
                            Text2d::new(event.text.clone()),
                            text_font(&ui_assets),
                            TextColor(color),
                        ),
                    );
                }
            }
        }
        FloatingTextType::PlayerMessage | FloatingTextType::CreatureSay => {
            let existing = speech_q
                .iter()
                .find(|(_, ft, _, _)| {
                    ft.anchor == anchor && ft.kind == event.text_type && ft.speaker == event.speaker
                })
                .map(|(entity, _, _, _)| entity);

            let line = (
                event.text.clone(),
                Timer::new(line_duration(event.text.chars().count()), TimerMode::Once),
            );

            if let Some(entity) = existing
                && let Ok((_, _, mut block, mut text)) = speech_q.get_mut(entity)
            {
                block.lines.push_back(line);
                while block.lines.len() > ft::SPEECH_MAX_LINES {
                    block.lines.pop_front();
                }
                text.0 = block.compose();
                return;
            }

            let mut lines = VecDeque::new();
            lines.push_back(line);
            spawn_floating_text(
                &mut commands,
                FloatingText {
                    kind: event.text_type,
                    speaker: event.speaker.clone(),
                    anchor: anchor.clone(),
                    spawned_at: now,
                    offset_y: 0.0,
                },
                (
                    SpeechBlock { lines },
                    Text2d::new(event.text.clone()),
                    TextLayout::new_with_justify(Justify::Center),
                    TextBounds::new_horizontal(ft::SPEECH_MAX_WIDTH_PX),
                    text_font(&ui_assets),
                    TextColor(color),
                ),
            );
        }
    }
}

fn spawn_floating_text(commands: &mut Commands, text: FloatingText, kind_parts: impl Bundle) {
    let root = commands
        .spawn((
            FloatingTextRoot,
            HudScaled,
            Transform::from_translation(
                anchor_world(&text.anchor, text.kind).extend(OVERLAY_TEXT_Z),
            ),
            Visibility::default(),
            on_overlay_layer(),
        ))
        .id();
    commands.spawn((
        Transform::from_xyz(0.0, text.offset_y, 0.0),
        text,
        kind_parts,
        Anchor::BOTTOM_CENTER,
        TextOutline {
            width: ft::OUTLINE_WIDTH,
            ..default()
        },
        on_overlay_layer(),
        ChildOf(root),
    ));
}

fn tile_centre(anchor: &Position) -> Vec2 {
    let world = anchor.to_world();
    Vec2::new(world.x + TILE_SIZE / 2.0, world.y - TILE_SIZE / 2.0)
}

/// Where a text's bottom edge is centred, in world units.
fn anchor_world(anchor: &Position, kind: FloatingTextType) -> Vec2 {
    let head_offset = match kind {
        FloatingTextType::HitPoints => 0.0,
        FloatingTextType::PlayerMessage | FloatingTextType::CreatureSay => {
            ft::SPEECH_HEAD_OFFSET_WORLD
        }
    };
    tile_centre(anchor) + Vec2::new(0.0, head_offset)
}

/// The anchor in logical pixels, y down, for comparing blocks with each other.
fn anchor_logical(anchor: &Position, kind: FloatingTextType, scale: f32) -> Vec2 {
    let world = anchor_world(anchor, kind);
    Vec2::new(world.x, -world.y) / scale
}

fn text_font(ui_assets: &GameUiAssets) -> TextFont {
    TextFont {
        font: ui_assets.font.clone(),
        font_size: ft::FONT_SIZE,
        ..default()
    }
    .with_font_smoothing(FontSmoothing::AntiAliased)
}

/// Advances each number's timer, fades its tail, and despawns it at the end.
pub fn tick_hit_points(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(&ChildOf, &mut HitPointsText, &mut TextColor)>,
) {
    for (root, mut hp, mut color) in q.iter_mut() {
        hp.timer.tick(time.delta());
        if hp.timer.is_finished() {
            commands.entity(root.parent()).despawn();
            continue;
        }
        color.0 = hp.color.with_alpha(alpha(hp.timer.fraction()));
    }
}

/// Expires speech lines, rebuilds the composed text when the line set changed, and
/// despawns a block once its last line is gone.
pub fn tick_speech_blocks(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(&ChildOf, &mut SpeechBlock, &mut Text2d)>,
) {
    for (root, mut block, mut text) in q.iter_mut() {
        let before = block.lines.len();
        for (_, timer) in block.lines.iter_mut() {
            timer.tick(time.delta());
        }
        block.lines.retain(|(_, timer)| !timer.is_finished());

        if block.lines.is_empty() {
            commands.entity(root.parent()).despawn();
            continue;
        }
        if block.lines.len() != before {
            text.0 = block.compose();
        }
    }
}

/// Pushes overlapping speech blocks clear of each other, from this frame's
/// measured sizes. Runs after `Text2d` layout.
pub fn resolve_speech_collisions(
    player_pos_q: Query<&Position, With<Player>>,
    viewport_q: Query<Ref<ComputedNode>, With<GameViewport>>,
    mut blocks_q: Query<(Entity, &mut FloatingText, Ref<TextLayoutInfo>), With<SpeechBlock>>,
    mut removed: RemovedComponents<SpeechBlock>,
) {
    let removed_any = removed.read().count() > 0;
    let Ok(viewport) = viewport_q.single() else {
        return;
    };
    let dirty = removed_any
        || viewport.is_changed()
        || blocks_q.iter().any(|(_, _, layout)| layout.is_changed());
    if !dirty {
        return;
    }
    let Some(scale) = world_hud_scale(&viewport) else {
        return;
    };
    let Ok(player_pos) = player_pos_q.single() else {
        return;
    };

    let mut entities = Vec::new();
    let mut layouts = Vec::new();
    for (entity, text, layout) in blocks_q.iter() {
        if text.anchor.z != player_pos.z || layout.size.x <= 0.0 || layout.size.y <= 0.0 {
            continue;
        }
        entities.push(entity);
        layouts.push(BlockLayout {
            anchor_px: anchor_logical(&text.anchor, text.kind, scale),
            size: layout.size,
            spawned_at: text.spawned_at,
        });
    }

    let offsets = resolve_offsets(&layouts);
    for (entity, offset_y) in entities.into_iter().zip(offsets) {
        if let Ok((_, mut text, _)) = blocks_q.get_mut(entity) {
            text.offset_y = offset_y;
        }
    }
}

/// Lifts every floating text by its collision push and rise, and hides text on
/// other floors.
pub fn position_floating_texts(
    player_pos_q: Query<&Position, With<Player>>,
    mut texts_q: Query<(
        &FloatingText,
        Option<&HitPointsText>,
        &mut Transform,
        &mut Visibility,
    )>,
) {
    let Ok(player_pos) = player_pos_q.single() else {
        return;
    };
    for (text, hp, mut transform, mut visibility) in &mut texts_q {
        visibility.set_if_neq(if text.anchor.z == player_pos.z {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
        let lift = text.offset_y + hp.map_or(0.0, |hp| risen(hp.timer.fraction()));
        if transform.translation.y != lift {
            transform.translation.y = lift;
        }
    }
}

pub fn cleanup_session(mut commands: Commands, roots: Query<Entity, With<FloatingTextRoot>>) {
    for root in &roots {
        commands.entity(root).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::visibility::RenderLayers;

    use crate::agent::AgentId;
    use crate::camera::HUD_RENDER_LAYER;
    use crate::conf::viewport::{GAME_VIEW_HEIGHT, GAME_VIEW_WIDTH};

    #[test]
    fn an_explicit_colour_wins_over_the_default() {
        assert_eq!(
            resolve_color(FloatingTextType::HitPoints, Some((10, 20, 30))),
            Color::srgb_u8(10, 20, 30)
        );
    }

    #[test]
    fn hit_points_default_to_white() {
        assert_eq!(
            resolve_color(FloatingTextType::HitPoints, None),
            Color::WHITE
        );
    }

    /// Speech reuses the local-chat yellow so a block and its chat-log line match.
    #[test]
    fn speech_defaults_to_the_local_chat_colour() {
        assert_eq!(
            resolve_color(FloatingTextType::PlayerMessage, None),
            Color::from(LOCAL_CHANNEL_COLOR)
        );
    }

    #[test]
    fn creature_say_is_orange_and_player_speech_is_not() {
        assert_eq!(
            resolve_color(FloatingTextType::CreatureSay, None),
            Color::from(CREATURE_SAY_COLOR)
        );
        assert_ne!(
            resolve_color(FloatingTextType::CreatureSay, None),
            resolve_color(FloatingTextType::PlayerMessage, None)
        );
    }

    #[test]
    fn line_duration_is_clamped_at_both_ends() {
        assert_eq!(line_duration(1), Duration::from_millis(ft::SPEECH_MIN_MS));
        assert_eq!(line_duration(255), Duration::from_millis(ft::SPEECH_MAX_MS));
    }

    #[test]
    fn line_duration_scales_between_the_bounds() {
        // 100 chars × 60 ms = 6000 ms, inside [3000, 8000].
        assert_eq!(line_duration(100), Duration::from_millis(6000));
    }

    /// `risen` had no direct test until mutation testing found that dropping the
    /// `HP_RISE_PX` multiply entirely left the whole suite green. It feeds both the
    /// stagger heights and the rise animation, so a silent regression here would
    /// corrupt both.
    #[test]
    fn a_number_rises_the_full_distance_over_its_life() {
        assert_eq!(risen(0.0), 0.0);
        assert_eq!(risen(0.5), ft::HP_RISE_PX / 2.0);
        assert_eq!(risen(1.0), ft::HP_RISE_PX);
    }

    #[test]
    fn alpha_is_opaque_until_the_fade_starts() {
        assert_eq!(alpha(0.0), 1.0);
        assert_eq!(alpha(ft::HP_FADE_START - 0.01), 1.0);
    }

    #[test]
    fn alpha_reaches_zero_at_the_end_of_life() {
        assert_eq!(alpha(1.0), 0.0);
        assert!(alpha(0.9) > 0.0 && alpha(0.9) < 1.0);
    }

    #[test]
    fn the_first_text_at_a_tile_is_not_staggered() {
        assert_eq!(stagger_offset(&[]), 0.0);
    }

    #[test]
    fn a_second_text_sits_one_clearance_above_the_first() {
        assert_eq!(stagger_offset(&[0.0]), ft::HP_CLEARANCE_PX);
    }

    /// The property OTClient's formula loses. It computes `CLEARANCE - risen`,
    /// putting the newcomer at `12 - risen` while the older sits at `risen` — equal
    /// at `risen == 6`, so the two texts pass through each other in the middle of
    /// the window the rule exists to protect.
    #[test]
    fn staggered_texts_keep_a_full_clearance_at_every_point_in_the_window() {
        let mut risen = 0.0;
        while risen < ft::HP_CLEARANCE_PX {
            let newcomer = stagger_offset(&[risen]);
            assert!(
                (newcomer - risen) >= ft::HP_CLEARANCE_PX - f32::EPSILON,
                "gap collapsed to {} at risen {risen}",
                newcomer - risen
            );
            risen += 1.0;
        }
    }

    /// Three texts in the same instant. Comparing only against the newest would put
    /// the third back at zero, on top of the first.
    #[test]
    fn a_third_simultaneous_text_clears_both_predecessors() {
        assert_eq!(stagger_offset(&[0.0, ft::HP_CLEARANCE_PX]), 24.0);
    }

    /// The last slot the column is allowed to use: a candidate of *exactly*
    /// `HP_MAX_STAGGER_PX` must still be taken. Without this the cap comparison
    /// could be widened to `>=` and nothing would notice.
    #[test]
    fn the_last_slot_below_the_cap_is_still_used() {
        assert_eq!(stagger_offset(&[0.0, 12.0, 24.0]), ft::HP_MAX_STAGGER_PX);
    }

    #[test]
    fn the_column_recycles_to_the_bottom_past_the_cap() {
        // 0, 12, 24, 36 are occupied; 48 would exceed HP_MAX_STAGGER_PX.
        assert_eq!(stagger_offset(&[0.0, 12.0, 24.0, 36.0]), 0.0);
    }

    /// Once every live text has risen clear of the bottom slot, reuse it rather
    /// than stacking forever.
    #[test]
    fn the_bottom_slot_is_reused_once_it_is_free() {
        assert_eq!(stagger_offset(&[12.0, 24.0]), 0.0);
    }

    fn live(
        entity: Entity,
        value: Option<i64>,
        color: Color,
        fraction: f32,
        ms: u64,
    ) -> LiveHitPoints {
        LiveHitPoints {
            entity,
            offset_y: 0.0,
            value,
            color,
            fraction,
            spawned_at: Duration::from_millis(ms),
        }
    }

    #[test]
    fn the_first_number_at_a_tile_just_spawns() {
        match plan_hit_points(&[], Some(5), Color::WHITE) {
            HpArrival::Spawn { offset_y } => assert_eq!(offset_y, 0.0),
            HpArrival::Merge { .. } => panic!("nothing to merge with"),
        }
    }

    #[test]
    fn merge_sums_two_numbers_of_the_same_colour() {
        let existing = Entity::from_raw_u32(1).unwrap();
        let live = [live(existing, Some(-12), Color::WHITE, 0.1, 0)];
        match plan_hit_points(&live, Some(-8), Color::WHITE) {
            HpArrival::Merge { target, sum } => {
                assert_eq!(target, existing);
                assert_eq!(sum, -20);
            }
            HpArrival::Spawn { .. } => panic!("expected a merge"),
        }
    }

    #[test]
    fn a_different_colour_does_not_merge() {
        let live = [live(
            Entity::from_raw_u32(1).unwrap(),
            Some(12),
            Color::WHITE,
            0.0,
            0,
        )];
        match plan_hit_points(&live, Some(8), Color::BLACK) {
            HpArrival::Spawn { offset_y } => {
                assert_eq!(offset_y, ft::HP_CLEARANCE_PX, "must be staggered clear")
            }
            HpArrival::Merge { .. } => panic!("different colours must not merge"),
        }
    }

    #[test]
    fn a_non_numeric_arrival_never_merges() {
        let live = [live(
            Entity::from_raw_u32(1).unwrap(),
            Some(12),
            Color::WHITE,
            0.0,
            0,
        )];
        assert!(matches!(
            plan_hit_points(&live, None, Color::WHITE),
            HpArrival::Spawn { .. }
        ));
    }

    #[test]
    fn a_non_numeric_existing_text_is_not_a_merge_target() {
        let live = [live(
            Entity::from_raw_u32(1).unwrap(),
            None,
            Color::WHITE,
            0.0,
            0,
        )];
        assert!(matches!(
            plan_hit_points(&live, Some(8), Color::WHITE),
            HpArrival::Spawn { .. }
        ));
    }

    #[test]
    fn merging_is_refused_past_the_window() {
        let live = [live(
            Entity::from_raw_u32(1).unwrap(),
            Some(12),
            Color::WHITE,
            ft::HP_MERGE_WINDOW + 0.01,
            0,
        )];
        assert!(matches!(
            plan_hit_points(&live, Some(8), Color::WHITE),
            HpArrival::Spawn { .. }
        ));
    }

    /// Two candidates, only the older one mergeable. Merging must still happen —
    /// picking only the newest would spawn a redundant number next to a text that
    /// was happy to absorb it.
    #[test]
    fn an_older_mergeable_text_is_used_when_the_newest_cannot_merge() {
        let old = Entity::from_raw_u32(1).unwrap();
        let new = Entity::from_raw_u32(2).unwrap();
        let live = [
            live(old, Some(3), Color::WHITE, 0.1, 0),
            live(new, Some(4), Color::BLACK, 0.1, 10),
        ];
        match plan_hit_points(&live, Some(5), Color::WHITE) {
            HpArrival::Merge { target, sum } => {
                assert_eq!(target, old);
                assert_eq!(sum, 8);
            }
            HpArrival::Spawn { .. } => panic!("the older text was mergeable"),
        }
    }

    /// With two mergeable candidates the freshest wins, so a burst collapses into
    /// the number the player is currently looking at.
    #[test]
    fn the_newest_mergeable_text_wins() {
        let old = Entity::from_raw_u32(1).unwrap();
        let new = Entity::from_raw_u32(2).unwrap();
        let live = [
            live(old, Some(3), Color::WHITE, 0.1, 0),
            live(new, Some(4), Color::WHITE, 0.1, 10),
        ];
        match plan_hit_points(&live, Some(5), Color::WHITE) {
            HpArrival::Merge { target, sum } => {
                assert_eq!(target, new);
                assert_eq!(sum, 9);
            }
            HpArrival::Spawn { .. } => panic!("expected a merge"),
        }
    }

    fn block(x: f32, y: f32, w: f32, h: f32, ms: u64) -> BlockLayout {
        BlockLayout {
            anchor_px: Vec2::new(x, y),
            size: Vec2::new(w, h),
            spawned_at: Duration::from_millis(ms),
        }
    }

    #[test]
    fn a_lone_block_is_not_pushed() {
        assert_eq!(
            resolve_offsets(&[block(100.0, 100.0, 40.0, 12.0, 0)]),
            [0.0]
        );
    }

    #[test]
    fn blocks_far_apart_are_not_pushed() {
        let blocks = [
            block(0.0, 100.0, 40.0, 12.0, 0),
            block(300.0, 100.0, 40.0, 12.0, 1),
        ];
        assert_eq!(resolve_offsets(&blocks), [0.0, 0.0]);
    }

    #[test]
    fn overlapping_blocks_are_pushed_apart() {
        // Same anchor, so the newer block overlaps the older exactly.
        let blocks = [
            block(100.0, 100.0, 40.0, 12.0, 0),
            block(100.0, 100.0, 40.0, 12.0, 1),
        ];
        let offsets = resolve_offsets(&blocks);
        assert_eq!(offsets[0], 0.0, "the oldest block holds its position");
        assert_eq!(
            offsets[1],
            12.0 + ft::SPEECH_GAP_PX,
            "the newer block clears the older by its height plus the gap"
        );
    }

    /// Input order is not spawn order. The oldest wins regardless of where it sits
    /// in the slice, and offsets come back in input order.
    #[test]
    fn the_oldest_block_holds_its_position_whatever_the_input_order() {
        let blocks = [
            block(100.0, 100.0, 40.0, 12.0, 50), // newer, listed first
            block(100.0, 100.0, 40.0, 12.0, 10), // older
        ];
        let offsets = resolve_offsets(&blocks);
        assert_eq!(offsets[1], 0.0, "the older block is the anchor");
        assert_eq!(offsets[0], 12.0 + ft::SPEECH_GAP_PX);
    }

    #[test]
    fn a_third_block_clears_both_predecessors() {
        let blocks = [
            block(100.0, 100.0, 40.0, 12.0, 0),
            block(100.0, 100.0, 40.0, 12.0, 1),
            block(100.0, 100.0, 40.0, 12.0, 2),
        ];
        let offsets = resolve_offsets(&blocks);
        assert_eq!(offsets[2], 2.0 * (12.0 + ft::SPEECH_GAP_PX));
    }

    /// Horizontally adjacent but not overlapping: touching edges must not count as
    /// an overlap, or every block in a row would be pushed.
    #[test]
    fn blocks_that_only_touch_are_not_pushed() {
        let blocks = [
            block(0.0, 100.0, 40.0, 12.0, 0),
            block(40.0, 100.0, 40.0, 12.0, 1),
        ];
        assert_eq!(resolve_offsets(&blocks), [0.0, 0.0]);
    }

    /// The vertical mirror of `blocks_that_only_touch_are_not_pushed`. Without it
    /// nothing pins the y-axis clauses of `overlaps` at a boundary: the horizontal
    /// test short-circuits on its first clause, so a `<` widened to `<=` on either
    /// y comparison would go unnoticed. Block B's bottom edge sits exactly on block
    /// A's top edge.
    #[test]
    fn blocks_that_only_touch_vertically_are_not_pushed() {
        let blocks = [
            block(100.0, 100.0, 40.0, 12.0, 0), // occupies y 88..100
            block(100.0, 88.0, 40.0, 12.0, 1),  // occupies y 76..88
        ];
        assert_eq!(resolve_offsets(&blocks), [0.0, 0.0]);
    }

    /// The mirror of `blocks_that_only_touch_are_not_pushed`, for the other x
    /// clause. Short-circuit `&&` means the original test never evaluates
    /// `b.min.x < a.max.x` at all; placing the newer block to the *left* is what
    /// puts that comparison on the boundary.
    #[test]
    fn blocks_that_only_touch_horizontally_from_the_left_are_not_pushed() {
        let blocks = [
            block(40.0, 100.0, 40.0, 12.0, 0), // occupies x 20..60
            block(0.0, 100.0, 40.0, 12.0, 1),  // occupies x -20..20
        ];
        assert_eq!(resolve_offsets(&blocks), [0.0, 0.0]);
    }

    /// The mirror of the above, for the *other* y clause. `overlaps` compares the
    /// two rects in both directions, and one touching case only pins one clause:
    /// with B above A it is `b.min.y < a.max.y`, with B below A it is
    /// `a.min.y < b.max.y`. Both are needed or half the boundary goes unwatched.
    #[test]
    fn blocks_that_only_touch_vertically_from_below_are_not_pushed() {
        let blocks = [
            block(100.0, 100.0, 40.0, 12.0, 0), // occupies y 88..100
            block(100.0, 112.0, 40.0, 12.0, 1), // occupies y 100..112
        ];
        assert_eq!(resolve_offsets(&blocks), [0.0, 0.0]);
    }

    /// Blocks at *different* anchor heights with a partial overlap. Every other
    /// case here shares an anchor y, where pushing up and pushing down happen to
    /// be numerically identical — so only this one pins the direction.
    #[test]
    fn a_partially_overlapping_block_is_pushed_by_exactly_the_overlap() {
        let blocks = [
            block(100.0, 100.0, 40.0, 12.0, 0), // occupies y 88..100
            block(100.0, 95.0, 40.0, 12.0, 1),  // occupies y 83..95, overlapping by 7
        ];
        let offsets = resolve_offsets(&blocks);
        assert_eq!(offsets[0], 0.0);
        assert_eq!(offsets[1], 7.0 + ft::SPEECH_GAP_PX);
    }

    /// A taller block (more queued lines) must be cleared by its real height.
    #[test]
    fn the_push_uses_the_blockers_measured_height() {
        let blocks = [
            block(100.0, 100.0, 40.0, 36.0, 0),
            block(100.0, 100.0, 40.0, 12.0, 1),
        ];
        let offsets = resolve_offsets(&blocks);
        assert_eq!(offsets[1], 36.0 + ft::SPEECH_GAP_PX);
    }

    use crate::game_ui::{GameUiAssets, GameViewport};
    use crate::network::events::ShowFloatingText;

    /// The agent every observer test speaks and bleeds through, standing on
    /// `SPEAKER_TILE`.
    const SPEAKER: &str = "Rizael";

    fn speaker_tile() -> Position {
        Position::new(10, 10, 7)
    }

    /// A world with what the observer needs from the app: the UI font. No `Map` —
    /// the message carries its own tile, and no agent has to exist for the text to
    /// land.
    fn observer_world() -> World {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.insert_resource(GameUiAssets {
            font: Handle::default(),
            window: Default::default(),
            inventory: Default::default(),
            background_dark: Handle::default(),
            background_light: Handle::default(),
            bar_overlay: Handle::default(),
            title_background: Handle::default(),
        });
        world.add_observer(on_floating_text);
        world
    }

    fn hp_texts(world: &mut World) -> Vec<(String, f32)> {
        world
            .query::<(&Text2d, &FloatingText)>()
            .iter(world)
            .map(|(text, ft)| (text.0.clone(), ft.offset_y))
            .collect()
    }

    #[test]
    fn an_arriving_number_hangs_from_a_scaled_root_on_its_tile() {
        let mut world = observer_world();
        world.trigger(ShowFloatingText {
            text: "-25".to_owned(),
            speaker: Some(SPEAKER.to_owned()),
            position: speaker_tile(),
            text_type: FloatingTextType::HitPoints,
            color: None,
        });
        world.flush();

        let spawned = hp_texts(&mut world);
        assert_eq!(spawned, [("-25".to_owned(), 0.0)]);

        let root = world
            .query_filtered::<&ChildOf, With<FloatingText>>()
            .single(&world)
            .unwrap()
            .parent();
        assert!(world.get::<HudScaled>(root).is_some());
        assert_eq!(
            world.get::<Transform>(root).unwrap().translation,
            anchor_world(&speaker_tile(), FloatingTextType::HitPoints).extend(OVERLAY_TEXT_Z)
        );
    }

    #[test]
    fn every_kind_is_drawn_by_the_hud_camera() {
        for kind in [
            FloatingTextType::HitPoints,
            FloatingTextType::PlayerMessage,
            FloatingTextType::CreatureSay,
        ] {
            let mut world = observer_world();
            world.trigger(ShowFloatingText {
                text: "-25".to_owned(),
                speaker: Some(SPEAKER.to_owned()),
                position: speaker_tile(),
                text_type: kind,
                color: None,
            });
            world.flush();

            let mut q = world
                .query_filtered::<&RenderLayers, Or<(With<FloatingText>, With<FloatingTextRoot>)>>(
                );
            let layers: Vec<_> = q.iter(&world).collect();
            assert_eq!(layers.len(), 2, "{kind:?}: a root and its text");
            assert!(
                layers
                    .iter()
                    .all(|l| **l == RenderLayers::layer(HUD_RENDER_LAYER)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn two_mergeable_numbers_leave_one_entity_showing_the_sum() {
        let mut world = observer_world();
        for text in ["-12", "-8"] {
            world.trigger(ShowFloatingText {
                text: text.to_owned(),
                speaker: Some(SPEAKER.to_owned()),
                position: speaker_tile(),
                text_type: FloatingTextType::HitPoints,
                color: Some((255, 255, 255)),
            });
            world.flush();
        }

        assert_eq!(hp_texts(&mut world), [("-20".to_owned(), 0.0)]);
    }

    #[test]
    fn a_second_message_on_a_tile_queues_into_the_block() {
        let mut world = observer_world();
        for text in ["hi there", "how are you"] {
            world.trigger(ShowFloatingText {
                text: text.to_owned(),
                speaker: Some(SPEAKER.to_owned()),
                position: speaker_tile(),
                text_type: FloatingTextType::PlayerMessage,
                color: None,
            });
            world.flush();
        }

        let mut q = world.query::<&SpeechBlock>();
        let block = q.single(&world).unwrap();
        assert_eq!(block.lines.len(), 2, "one block, two lines");
        assert_eq!(block.compose(), "hi there\nhow are you");
    }

    #[test]
    fn a_message_on_another_tile_starts_its_own_block() {
        let mut world = observer_world();
        const NEIGHBOUR: &str = "Kestrel";
        let neighbour_tile = Position::new(11, 10, 7);

        for (speaker, tile, text) in [
            (SPEAKER, speaker_tile(), "hi"),
            (NEIGHBOUR, neighbour_tile, "hello"),
        ] {
            world.trigger(ShowFloatingText {
                text: text.to_owned(),
                speaker: Some(speaker.to_owned()),
                position: tile,
                text_type: FloatingTextType::PlayerMessage,
                color: None,
            });
            world.flush();
        }

        assert_eq!(world.query::<&SpeechBlock>().iter(&world).count(), 2);
    }

    /// The anchor is the tile the message named, and nothing re-reads it — so a
    /// speaker who walks away leaves the text behind on the tile they spoke from,
    /// and their next line starts a new block on the new tile.
    #[test]
    fn a_speaker_who_walks_away_leaves_the_text_behind() {
        let mut world = observer_world();
        let walked_to = Position::new(speaker_tile().x + 1, speaker_tile().y, speaker_tile().z);

        for tile in [speaker_tile(), walked_to.clone()] {
            world.trigger(ShowFloatingText {
                text: "hi".to_owned(),
                speaker: Some(SPEAKER.to_owned()),
                position: tile,
                text_type: FloatingTextType::PlayerMessage,
                color: None,
            });
            world.flush();
        }

        let anchors: Vec<Position> = world
            .query_filtered::<&FloatingText, With<SpeechBlock>>()
            .iter(&world)
            .map(|ft| ft.anchor.clone())
            .collect();
        assert_eq!(anchors.len(), 2, "the first block stayed where it was said");
        assert!(anchors.contains(&speaker_tile()));
        assert!(anchors.contains(&walked_to));
    }

    /// The killing blow: the server sends the damage number for a creature and
    /// removes that creature in the same batch. Nothing names the speaker, no agent
    /// need exist, and the text still lands on the tile the message carried.
    /// Resolving the anchor through `Map` instead is what used to swallow the last
    /// hit of every fight.
    #[test]
    fn a_number_with_no_speaker_lands_on_the_tile_it_names() {
        let mut world = observer_world();
        world.trigger(ShowFloatingText {
            text: "-25".to_owned(),
            speaker: None,
            position: speaker_tile(),
            text_type: FloatingTextType::HitPoints,
            color: None,
        });
        world.flush();

        assert_eq!(hp_texts(&mut world), [("-25".to_owned(), 0.0)]);
    }

    #[test]
    fn the_oldest_line_drops_at_the_cap() {
        let mut world = observer_world();
        for i in 0..=ft::SPEECH_MAX_LINES {
            world.trigger(ShowFloatingText {
                text: format!("line {i}"),
                speaker: Some(SPEAKER.to_owned()),
                position: speaker_tile(),
                text_type: FloatingTextType::PlayerMessage,
                color: None,
            });
            world.flush();
        }

        let mut q = world.query::<&SpeechBlock>();
        let block = q.single(&world).unwrap();
        assert_eq!(block.lines.len(), ft::SPEECH_MAX_LINES);
        assert!(
            !block.compose().contains("line 0"),
            "the first line must have been evicted, got {:?}",
            block.compose()
        );
        assert!(block.compose().contains("line 5"));
    }

    /// The mode is part of the key too, again following `StaticText::addMessage`.
    /// Merging the two would put the orange creature-say inside the yellow
    /// player block, and one entity carries one `TextColor`, so the orange would
    /// never render at all.
    #[test]
    fn a_creature_say_does_not_join_a_player_speech_block() {
        let mut world = observer_world();

        for kind in [
            FloatingTextType::PlayerMessage,
            FloatingTextType::CreatureSay,
        ] {
            world.trigger(ShowFloatingText {
                text: "hi".to_owned(),
                speaker: Some(SPEAKER.to_owned()),
                position: speaker_tile(),
                text_type: kind,
                color: None,
            });
            world.flush();
        }

        assert_eq!(
            world.query::<&SpeechBlock>().iter(&world).count(),
            2,
            "the two modes were merged into one block"
        );
    }

    /// A block is keyed on the speaker as well as the tile, following OTClient's
    /// `StaticText::addMessage`, which refuses to append when the name differs.
    /// Two agents standing on one tile therefore get a block each rather than
    /// interleaving their lines into one.
    #[test]
    fn two_speakers_on_one_tile_get_a_block_each() {
        let mut world = observer_world();
        const NEIGHBOUR: &str = "Kestrel";

        for speaker in [SPEAKER, NEIGHBOUR] {
            world.trigger(ShowFloatingText {
                text: "hi".to_owned(),
                speaker: Some(speaker.to_owned()),
                position: speaker_tile(),
                text_type: FloatingTextType::PlayerMessage,
                color: None,
            });
            world.flush();
        }

        assert_eq!(world.query::<&SpeechBlock>().iter(&world).count(), 2);
    }

    #[test]
    fn the_same_speaker_in_the_same_mode_still_composes_one_block() {
        let mut world = observer_world();

        for text in ["first", "second"] {
            world.trigger(ShowFloatingText {
                text: text.to_owned(),
                speaker: Some(SPEAKER.to_owned()),
                position: speaker_tile(),
                text_type: FloatingTextType::CreatureSay,
                color: None,
            });
            world.flush();
        }

        let mut q = world.query::<&SpeechBlock>();
        let block = q.single(&world).unwrap();
        assert_eq!(block.lines.len(), 2);
    }

    use bevy::ecs::system::RunSystemOnce;

    fn advance(world: &mut World, ms: u64) {
        let mut time = world.resource_mut::<Time>();
        time.advance_by(Duration::from_millis(ms));
    }

    #[test]
    fn a_number_despawns_when_its_timer_finishes() {
        let mut world = observer_world();
        world.trigger(ShowFloatingText {
            text: "-1".to_owned(),
            speaker: Some(SPEAKER.to_owned()),
            position: speaker_tile(),
            text_type: FloatingTextType::HitPoints,
            color: None,
        });
        world.flush();

        advance(&mut world, ft::HP_DURATION_MS + 1);
        world.run_system_once(tick_hit_points).unwrap();

        assert_eq!(world.query::<&HitPointsText>().iter(&world).count(), 0);
    }

    #[test]
    fn a_number_fades_over_its_tail() {
        let mut world = observer_world();
        world.trigger(ShowFloatingText {
            text: "-1".to_owned(),
            speaker: Some(SPEAKER.to_owned()),
            position: speaker_tile(),
            text_type: FloatingTextType::HitPoints,
            color: None,
        });
        world.flush();

        // 95% through: past HP_FADE_START, not yet expired.
        advance(&mut world, (ft::HP_DURATION_MS as f32 * 0.95) as u64);
        world.run_system_once(tick_hit_points).unwrap();

        let mut q = world.query::<&TextColor>();
        let color = q.single(&world).unwrap();
        let a = color.0.alpha();
        assert!(a > 0.0 && a < 1.0, "expected a partial fade, got {a}");
    }

    #[test]
    fn an_expired_line_leaves_the_block_and_the_rest_stay() {
        let mut world = observer_world();
        // A short line, then a long one that outlives it.
        world.trigger(ShowFloatingText {
            text: "hi".to_owned(),
            speaker: Some(SPEAKER.to_owned()),
            position: speaker_tile(),
            text_type: FloatingTextType::PlayerMessage,
            color: None,
        });
        world.flush();
        world.trigger(ShowFloatingText {
            text: "x".repeat(200),
            speaker: Some(SPEAKER.to_owned()),
            position: speaker_tile(),
            text_type: FloatingTextType::PlayerMessage,
            color: None,
        });
        world.flush();

        advance(&mut world, ft::SPEECH_MIN_MS + 1);
        world.run_system_once(tick_speech_blocks).unwrap();

        let mut q = world.query::<&SpeechBlock>();
        let block = q.single(&world).unwrap();
        assert_eq!(block.lines.len(), 1, "the short line expired");
        assert!(block.compose().starts_with('x'));
    }

    /// A merge must not restart the absorbing number's timer, or a sustained
    /// stream of hits produces an immortal number. The comment on the merge arm
    /// says so; this is what stops a later refactor "fixing" it.
    #[test]
    fn merging_does_not_extend_the_numbers_life() {
        let mut world = observer_world();
        let hit = |world: &mut World| {
            world.trigger(ShowFloatingText {
                text: "-1".to_owned(),
                speaker: Some(SPEAKER.to_owned()),
                position: speaker_tile(),
                text_type: FloatingTextType::HitPoints,
                color: Some((255, 255, 255)),
            });
            world.flush();
        };

        hit(&mut world);
        advance(&mut world, 300);
        world.run_system_once(tick_hit_points).unwrap();
        hit(&mut world); // inside the merge window, so this merges
        advance(&mut world, 701); // 1001 ms since the *first* hit
        world.run_system_once(tick_hit_points).unwrap();

        assert_eq!(
            world.query::<&HitPointsText>().iter(&world).count(),
            0,
            "the merged number must still die on the original timer"
        );
    }

    #[test]
    fn a_block_despawns_when_its_last_line_expires() {
        let mut world = observer_world();
        world.trigger(ShowFloatingText {
            text: "hi".to_owned(),
            speaker: Some(SPEAKER.to_owned()),
            position: speaker_tile(),
            text_type: FloatingTextType::PlayerMessage,
            color: None,
        });
        world.flush();

        advance(&mut world, ft::SPEECH_MAX_MS + 1);
        world.run_system_once(tick_speech_blocks).unwrap();

        assert_eq!(world.query::<&SpeechBlock>().iter(&world).count(), 0);
    }

    /// The composed text must be rewritten only when a line actually left. A changed
    /// `Text2d` is laid out again, and a changed layout wakes the collision pass, so
    /// an unconditional write here would make it re-resolve every frame for nothing.
    #[test]
    fn ticking_without_an_expiry_does_not_touch_the_text() {
        let mut world = observer_world();
        world.trigger(ShowFloatingText {
            text: "hi".to_owned(),
            speaker: Some(SPEAKER.to_owned()),
            position: speaker_tile(),
            text_type: FloatingTextType::PlayerMessage,
            color: None,
        });
        world.flush();
        world.clear_trackers();

        advance(&mut world, 100);
        world.run_system_once(tick_speech_blocks).unwrap();

        let mut q = world.query_filtered::<Entity, Changed<Text2d>>();
        assert_eq!(
            q.iter(&world).count(),
            0,
            "no line expired, so Text must not have been rewritten"
        );
    }

    /// The bug this pins: `Position::to_world` is the tile's top-left *corner*,
    /// so anchoring to it put every floating text half a tile up and to the left.
    #[test]
    fn the_anchor_is_the_tile_centre_not_its_corner() {
        let anchor = Position::new(100, 100, 7);
        let corner = anchor.to_world().truncate();

        assert_eq!(
            anchor_world(&anchor, FloatingTextType::HitPoints),
            corner + Vec2::new(TILE_SIZE / 2.0, -TILE_SIZE / 2.0)
        );
    }

    /// The head offset is in world units, on the root, so it scales with the view:
    /// double the viewport, halve the scale, double the on-screen gap. An offset
    /// in logical pixels would hold its size and drift off the sprite's head as the
    /// window grows.
    #[test]
    fn the_speech_head_offset_scales_with_the_viewport() {
        let anchor = Position::new(100, 100, 7);
        let gap = |scale: f32| {
            anchor_logical(&anchor, FloatingTextType::HitPoints, scale).y
                - anchor_logical(&anchor, FloatingTextType::PlayerMessage, scale).y
        };

        assert_eq!(
            gap(1.0),
            ft::SPEECH_HEAD_OFFSET_WORLD,
            "speech sits above the tile"
        );
        assert_eq!(gap(0.5), gap(1.0) * 2.0);
    }

    /// OTClient anchors every static text the same way regardless of mode, and so
    /// does this.
    #[test]
    fn creature_say_hangs_at_the_same_height_as_speech() {
        let anchor = speaker_tile();
        assert_eq!(
            anchor_world(&anchor, FloatingTextType::CreatureSay),
            anchor_world(&anchor, FloatingTextType::PlayerMessage)
        );
    }

    /// `resolve_offsets` works y-down; a tile further south must compare as lower.
    #[test]
    fn a_southern_tile_is_lower_in_logical_pixels() {
        let north = anchor_logical(
            &Position::new(100, 100, 7),
            FloatingTextType::HitPoints,
            1.0,
        );
        let south = anchor_logical(
            &Position::new(100, 101, 7),
            FloatingTextType::HitPoints,
            1.0,
        );
        assert_eq!(south.y - north.y, TILE_SIZE);
    }

    fn placement_world(player_z: u8) -> World {
        let mut world = World::new();
        world.spawn((
            GameViewport,
            crate::overlay::a_viewport(Vec2::new(GAME_VIEW_WIDTH, GAME_VIEW_HEIGHT), 1.0),
        ));
        world.spawn((
            Player {
                agent_id: AgentId(1),
            },
            Position::new(100, 100, player_z),
        ));
        world
    }

    fn a_number(world: &mut World, elapsed_ms: u64, offset_y: f32) -> Entity {
        let mut timer = Timer::new(Duration::from_millis(ft::HP_DURATION_MS), TimerMode::Once);
        timer.tick(Duration::from_millis(elapsed_ms));
        world
            .spawn((
                FloatingText {
                    kind: FloatingTextType::HitPoints,
                    speaker: None,
                    anchor: Position::new(100, 100, 7),
                    spawned_at: Duration::ZERO,
                    offset_y,
                },
                HitPointsText {
                    value: Some(5),
                    color: Color::WHITE,
                    timer,
                },
                Transform::default(),
                Visibility::Hidden,
            ))
            .id()
    }

    fn lift(world: &World, entity: Entity) -> f32 {
        world.get::<Transform>(entity).unwrap().translation.y
    }

    #[test]
    fn a_number_rises_as_its_timer_advances() {
        let mut world = placement_world(7);
        let at_rest = a_number(&mut world, 0, 0.0);
        let halfway = a_number(&mut world, ft::HP_DURATION_MS / 2, 0.0);

        world.run_system_once(position_floating_texts).unwrap();

        assert_eq!(lift(&world, at_rest), 0.0);
        assert_eq!(lift(&world, halfway), risen(0.5));
    }

    #[test]
    fn a_collision_offset_lifts_the_text() {
        let mut world = placement_world(7);
        let pushed = a_number(&mut world, 0, 14.0);

        world.run_system_once(position_floating_texts).unwrap();

        assert_eq!(lift(&world, pushed), 14.0);
    }

    #[test]
    fn a_text_shows_only_on_the_players_floor() {
        for (player_z, expected) in [(7, Visibility::Inherited), (8, Visibility::Hidden)] {
            let mut world = placement_world(player_z);
            let text = a_number(&mut world, 0, 0.0);

            world.run_system_once(position_floating_texts).unwrap();

            assert_eq!(
                *world.get::<Visibility>(text).unwrap(),
                expected,
                "player on floor {player_z}"
            );
        }
    }

    fn a_block(world: &mut World, tile: Position, spawned_ms: u64, size: Vec2) -> Entity {
        world
            .spawn((
                FloatingText {
                    kind: FloatingTextType::PlayerMessage,
                    speaker: None,
                    anchor: tile,
                    spawned_at: Duration::from_millis(spawned_ms),
                    offset_y: 0.0,
                },
                SpeechBlock {
                    lines: VecDeque::new(),
                },
                TextLayoutInfo { size, ..default() },
            ))
            .id()
    }

    #[test]
    fn a_block_is_pushed_clear_by_its_neighbours_measured_height() {
        let mut world = placement_world(7);
        let size = Vec2::new(40.0, 12.0);
        let first = a_block(&mut world, Position::new(100, 100, 7), 0, size);
        let second = a_block(&mut world, Position::new(100, 100, 7), 1, size);

        world.run_system_once(resolve_speech_collisions).unwrap();

        assert_eq!(world.get::<FloatingText>(first).unwrap().offset_y, 0.0);
        assert_eq!(
            world.get::<FloatingText>(second).unwrap().offset_y,
            size.y + ft::SPEECH_GAP_PX
        );
    }

    #[test]
    fn leaving_the_game_clears_every_floating_text() {
        let mut world = observer_world();
        world.trigger(ShowFloatingText {
            text: "-25".to_owned(),
            speaker: None,
            position: speaker_tile(),
            text_type: FloatingTextType::HitPoints,
            color: None,
        });
        world.flush();

        world.run_system_once(cleanup_session).unwrap();

        assert_eq!(world.query::<&FloatingText>().iter(&world).count(), 0);
        assert_eq!(world.query::<&FloatingTextRoot>().iter(&world).count(), 0);
    }
}
