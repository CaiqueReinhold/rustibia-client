use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageLoaderSettings;
use bevy::prelude::*;
use serde_json::*;

use crate::items::ItemId;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, serde::Deserialize)]
#[repr(transparent)]
pub struct OutfitId(pub u16);
/// The four dyeable regions of an outfit, in the order the wire uses them. Named
/// rather than a 4-tuple: `color1` said nothing about which region it was, and all
/// four are interchangeable to the type checker as bare `u8`s.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct OutfitColors {
    pub head: u8,
    pub body: u8,
    pub legs: u8,
    pub feet: u8,
}

impl OutfitColors {
    pub fn new(head: u8, body: u8, legs: u8, feet: u8) -> Self {
        Self {
            head,
            body,
            legs,
            feet,
        }
    }

    /// The layout the outfit shader reads: one region per byte, head in the low bits.
    pub fn packed(self) -> u32 {
        self.head as u32
            | ((self.body as u32) << 8)
            | ((self.legs as u32) << 16)
            | ((self.feet as u32) << 24)
    }
}
/// Ids of the `effect` and `missile` appearance categories. Both are sparse and
/// small (effects reach 309, missiles 62) and neither shares a namespace with
/// items or outfits, so they stay separate maps rather than one merged one.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, serde::Deserialize)]
#[repr(transparent)]
pub struct EffectId(pub u16);

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, serde::Deserialize)]
#[repr(transparent)]
pub struct MissileId(pub u16);

#[derive(Debug)]
pub struct OutfitSprite {
    // pub id: OutfitId,
    pub still_sprite: Arc<SpriteConfig>,
    pub moving_sprite: Arc<SpriteConfig>,
}

#[derive(Resource, Debug)]
pub struct Appearances {
    sheets: HashMap<String, SpriteSheet>,
    items: HashMap<ItemId, Arc<SpriteConfig>>,
    outfits: HashMap<OutfitId, OutfitSprite>,
    effects: HashMap<EffectId, Arc<SpriteConfig>>,
    missiles: HashMap<MissileId, Arc<SpriteConfig>>,
    asset_server: AssetServer,
}

impl Appearances {
    pub(super) fn new(
        sheets: HashMap<String, SpriteSheet>,
        configs: SpriteConfigs,
        asset_server: AssetServer,
    ) -> Self {
        Appearances {
            sheets,
            items: configs.items,
            outfits: configs.outfits,
            effects: configs.effects,
            missiles: configs.missiles,
            asset_server,
        }
    }

    /// Every id reaching here came from an [`crate::items::Item`], and an
    /// `Item` is only built after its id was found in `ItemConfigs` — so a
    /// miss means items.json and sprite.json disagree, which is a broken
    /// install rather than anything the server said.
    pub fn get_item(&self, id: ItemId) -> Arc<SpriteConfig> {
        Arc::clone(
            self.items.get(&id).unwrap_or_else(|| {
                panic!("item {id:?} is in items.json but missing from sprite.json")
            }),
        )
    }

    pub fn get_outfit(&self, id: OutfitId) -> Option<&OutfitSprite> {
        self.outfits.get(&id)
    }

    /// `None` means the server named an effect this client's assets do not have,
    /// the same failure `get_outfit` reports — a client too old for the server.
    pub fn get_effect(&self, id: EffectId) -> Option<&Arc<SpriteConfig>> {
        self.effects.get(&id)
    }

    pub fn get_missile(&self, id: MissileId) -> Option<&Arc<SpriteConfig>> {
        self.missiles.get(&id)
    }

    pub fn get_sheet(&self, group: &str) -> &SpriteSheet {
        let sheet = self.sheets.get(group).unwrap();
        sheet.texture.get_or_init(|| {
            self.asset_server.load_with_settings::<Image, _>(
                format!("sheets/{}", sheet.sheet_name),
                |s: &mut ImageLoaderSettings| {
                    s.asset_usage = RenderAssetUsages::RENDER_WORLD;
                },
            )
        });
        sheet
    }
}

#[derive(Debug)]
pub struct SpriteSheet {
    pub sheet_name: String,
    pub grid_size: Vec2,
    pub sprite_size: Vec2,
    texture: OnceLock<Handle<Image>>,
}

impl SpriteSheet {
    pub fn texture(&self) -> &Handle<Image> {
        self.texture
            .get()
            .expect("sprite sheet texture not initialized — access via Appearances::get_sheet")
    }

    /// A sheet with no texture loaded, for tests that only read its name or its grid.
    #[cfg(test)]
    pub fn for_test(sheet_name: &str, grid_size: Vec2, sprite_size: Vec2) -> Self {
        Self {
            sheet_name: sheet_name.to_string(),
            grid_size,
            sprite_size,
            texture: OnceLock::new(),
        }
    }
}

/// What happens when an animation runs off its last phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationLoop {
    /// Wrap to phase zero forever. Every animated outfit is this.
    Infinite,
    /// Walk back down to phase zero, then up again.
    PingPong,
    /// Run `count` times and then stop, holding the last phase. No outfit is
    /// counted, so this can never freeze a creature; it is what makes an effect
    /// a one-shot, and 188 items genuinely play once too.
    Counted { count: u32 },
}

#[derive(Debug)]
pub enum SpriteAnimation {
    Static,
    Uniform {
        loop_mode: AnimationLoop,
        phase_count: u32,
        phase_duration: Duration,
    },
    NonUniform {
        loop_mode: AnimationLoop,
        phases: Vec<UVec2>,
    },
}

impl SpriteAnimation {
    pub fn total_animation_phases(&self) -> u32 {
        match self {
            SpriteAnimation::Static => 1,
            SpriteAnimation::Uniform { phase_count, .. } => *phase_count,
            SpriteAnimation::NonUniform { phases, .. } => phases.len() as u32,
        }
    }

    pub fn loop_mode(&self) -> AnimationLoop {
        match self {
            SpriteAnimation::Static => AnimationLoop::Infinite,
            SpriteAnimation::Uniform { loop_mode, .. }
            | SpriteAnimation::NonUniform { loop_mode, .. } => *loop_mode,
        }
    }

    /// How long `phase` is displayed.
    ///
    /// A non-uniform phase carries a `[min, max]` range. 911 of the 919 effect
    /// phases have `min == max`, but the 8 that differ — and the 491 item phases
    /// that do — are sampled, which is what stops a room full of torches
    /// flickering in lockstep.
    pub fn phase_duration(&self, phase: u32) -> Duration {
        match self {
            SpriteAnimation::Static => Duration::ZERO,
            SpriteAnimation::Uniform { phase_duration, .. } => *phase_duration,
            SpriteAnimation::NonUniform { phases, .. } => match phases.get(phase as usize) {
                Some(range) if range.y > range.x => {
                    Duration::from_millis(fastrand::u32(range.x..=range.y) as u64)
                }
                Some(range) => Duration::from_millis(range.x as u64),
                None => Duration::ZERO,
            },
        }
    }

    /// Whether the config gives `phase` no time at all.
    ///
    /// Read off the range and never off a sample: whether a phase is skipped
    /// must not depend on a dice roll.
    pub fn phase_is_untimed(&self, phase: u32) -> bool {
        match self {
            SpriteAnimation::Static => true,
            SpriteAnimation::Uniform { phase_duration, .. } => phase_duration.is_zero(),
            SpriteAnimation::NonUniform { phases, .. } => match phases.get(phase as usize) {
                Some(range) => range.x == 0 && range.y == 0,
                None => true,
            },
        }
    }

    /// True when no phase has any time on it, so nothing will ever move this
    /// animation forward. Static animations, and any config that is all zeros.
    ///
    /// `SpriteAnimator::new` uses this to decide whether to arm a real timer at
    /// all: when it's true, the timer is left on its zero-duration `Once`
    /// sentinel, which is what makes `tick_sprite_animators` skip the animator
    /// without ever ticking it. It says nothing about whether a walk over the
    /// phases terminates -- that is the walk's own loop bound.
    pub fn never_advances(&self) -> bool {
        (0..self.total_animation_phases()).all(|phase| self.phase_is_untimed(phase))
    }

    /// One pass over every phase's duration, sampling each phase independently.
    /// Not the same value an animator would sample while actually running: a
    /// `Counted { count }` loop's real lifetime is `count` such passes, not
    /// one, and each caller gets its own independent samples.
    ///
    /// Reads it to size an effect entity's lifetime for the loop modes that
    /// never finish on their own. All 13 non-`COUNTED` effects are `Uniform`,
    /// so for every real caller today the sampling is moot and the result is
    /// exact.
    pub fn pass_duration(&self) -> Duration {
        (0..self.total_animation_phases())
            .map(|phase| self.phase_duration(phase))
            .sum()
    }
}

#[derive(Debug)]
pub struct SpriteConfig {
    pub id: u16,
    pub group: String,
    pub pattern_x: u32,
    pub pattern_y: u32,
    pub pattern_z: u32,
    pub layers: u32,
    pub sprite_ids: Vec<u32>,
    pub animation: SpriteAnimation,
    pub boxes: Vec<Rect>,
    pub shift: Vec2,
}

#[derive(Debug, Default)]
pub struct SpriteConfigs {
    pub items: HashMap<ItemId, Arc<SpriteConfig>>,
    pub outfits: HashMap<OutfitId, OutfitSprite>,
    pub effects: HashMap<EffectId, Arc<SpriteConfig>>,
    pub missiles: HashMap<MissileId, Arc<SpriteConfig>>,
}

pub fn read_sprites_config() -> SpriteConfigs {
    let Ok(contents) = fs::read_to_string("assets/configs/sprite.json") else {
        panic!("Could not read sprites file");
    };
    let sprites: Value = serde_json::from_str(&contents).unwrap();

    let mut items: HashMap<ItemId, Arc<SpriteConfig>> = HashMap::new();
    for conf in sprites["items"].as_array().unwrap().iter() {
        let sprite = read_sprite_config(conf);
        items.insert(ItemId(sprite.id), Arc::new(sprite));
    }
    let mut outfits: HashMap<OutfitId, OutfitSprite> = HashMap::new();
    for out in sprites["outfits"].as_array().unwrap().iter() {
        let id = OutfitId(out["id"].as_u64().unwrap() as u16);
        let still_sprite = read_sprite_config(&out["still_sprite"]);
        let moving_sprite = read_sprite_config(&out["moving_sprite"]);
        outfits.insert(
            id,
            OutfitSprite {
                // id,
                still_sprite: Arc::new(still_sprite),
                moving_sprite: Arc::new(moving_sprite),
            },
        );
    }

    SpriteConfigs {
        items,
        outfits,
        effects: read_flat_configs(&sprites["effects"], EffectId),
        missiles: read_flat_configs(&sprites["missiles"], MissileId),
    }
}

/// Effects and missiles are flat lists of configs keyed by their own id — no
/// still/moving split, one frame group each.
fn read_flat_configs<K: Eq + std::hash::Hash>(
    value: &Value,
    key: impl Fn(u16) -> K,
) -> HashMap<K, Arc<SpriteConfig>> {
    let mut configs = HashMap::new();
    let Some(entries) = value.as_array() else {
        return configs;
    };
    for conf in entries.iter() {
        let sprite = read_sprite_config(conf);
        configs.insert(key(sprite.id), Arc::new(sprite));
    }
    configs
}

fn read_sprite_config(conf: &Value) -> SpriteConfig {
    let id = conf["id"].as_u64().unwrap() as u16;
    let group = conf["group"].as_str().unwrap().to_string();
    let pattern_x = conf["pattern_x"].as_u64().unwrap() as u32;
    let pattern_y = conf["pattern_y"].as_u64().unwrap() as u32;
    let pattern_z = conf["pattern_z"].as_u64().unwrap() as u32;
    let layers = conf["layers"].as_u64().unwrap() as u32;
    let sprite_ids: Vec<u32> = conf["sprite_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e.as_u64().unwrap() as u32)
        .collect();
    let animation = read_animation(&conf["animation"]);
    let mut boxes: Vec<Rect> = Vec::new();
    for b in conf["boxes"].as_array().unwrap().iter() {
        boxes.push(Rect {
            min: Vec2 {
                x: b[0].as_u64().unwrap() as f32,
                y: b[1].as_u64().unwrap() as f32,
            },
            max: Vec2 {
                x: b[2].as_u64().unwrap() as f32,
                y: b[3].as_u64().unwrap() as f32,
            },
        });
    }

    // Absent for the great majority of appearances; `(0, 0)` then means "draw where
    // the bounding box says", which is what the flag's absence means in the protobuf.
    let shift = match &conf["shift"] {
        Value::Array(s) => Vec2::new(s[0].as_u64().unwrap() as f32, s[1].as_u64().unwrap() as f32),
        _ => Vec2::ZERO,
    };

    SpriteConfig {
        id,
        group,
        pattern_x,
        pattern_y,
        pattern_z,
        layers,
        sprite_ids,
        boxes,
        animation,
        shift,
    }
}

fn read_animation(value: &Value) -> SpriteAnimation {
    match value {
        Value::Null => SpriteAnimation::Static,
        Value::Object(anim) => {
            // `loop_count` accompanies COUNTED only; the protobuf leaves it unset for
            // the endless modes. A COUNTED animation without one is malformed data,
            // so it is read as a single run rather than as "loop forever" — that way
            // a broken effect disappears instead of burning a slot permanently.
            let loop_mode = match anim["loop_type"].as_str() {
                Some("PINGPONG") => AnimationLoop::PingPong,
                Some("COUNTED") => AnimationLoop::Counted {
                    count: anim["loop_count"].as_u64().unwrap_or(1).max(1) as u32,
                },
                _ => AnimationLoop::Infinite,
            };
            match &anim["phases"] {
                Value::Array(anim_phases) => {
                    let mut phases: Vec<UVec2> = Vec::new();
                    for phase in anim_phases.iter() {
                        phases.push(UVec2::new(
                            phase[0].as_u64().unwrap() as u32,
                            phase[1].as_u64().unwrap() as u32,
                        ));
                    }
                    SpriteAnimation::NonUniform { loop_mode, phases }
                }
                _ => {
                    let phase_count = anim["phase_count"].as_u64().unwrap() as u32;
                    let phase_duration =
                        Duration::from_millis(anim["phase_duration"].as_u64().unwrap());
                    SpriteAnimation::Uniform {
                        loop_mode,
                        phase_count,
                        phase_duration,
                    }
                }
            }
        }
        _ => SpriteAnimation::Static,
    }
}

pub fn read_sprite_sheets() -> HashMap<String, SpriteSheet> {
    let Ok(contents) = fs::read_to_string("assets/configs/sheets.json") else {
        panic!("Could not read sheets file");
    };
    let sheets: Value = serde_json::from_str(&contents).unwrap();

    let mut sheets_map: HashMap<String, SpriteSheet> = HashMap::new();

    for sheet in sheets.as_array().unwrap().iter() {
        let grid_size = Vec2::new(
            sheet["grid_size"][0].as_u64().unwrap() as f32,
            sheet["grid_size"][1].as_u64().unwrap() as f32,
        );
        let sprite_size = Vec2::new(
            sheet["sprite_size"][0].as_u64().unwrap() as f32,
            sheet["sprite_size"][1].as_u64().unwrap() as f32,
        );
        let sheet_name = sheet["sheet_name"].as_str().unwrap().to_string();
        let group = sheet["group"].as_str().unwrap().to_string();

        sheets_map.insert(
            group,
            SpriteSheet {
                sheet_name,
                grid_size,
                sprite_size,
                texture: OnceLock::new(),
            },
        );
    }

    sheets_map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_json(extra: &str) -> Value {
        serde_json::from_str(&format!(
            r#"{{
                "id": 1, "group": "g",
                "pattern_x": 4, "pattern_y": 1, "pattern_z": 1, "layers": 1,
                "sprite_ids": [0, 1, 2, 3],
                "animation": null,
                "boxes": [[0, 0, 32, 32], [0, 0, 32, 32], [0, 0, 32, 32], [0, 0, 32, 32]]
                {extra}
            }}"#
        ))
        .unwrap()
    }

    /// `shift` is the appearance's art displacement (OTClient's `m_displacement`).
    /// Player outfits carry `(8, 8)`; it is what the agent shader subtracts.
    #[test]
    fn shift_is_read_from_the_config() {
        let config = read_sprite_config(&config_json(r#", "shift": [8, 8]"#));

        assert_eq!(config.shift, Vec2::new(8.0, 8.0));
    }

    /// The flag is absent for most appearances — the demon among them, which is why
    /// a hard-coded padding fitted the player and pushed the demon off its tile.
    /// Absent must mean zero, not "keep the last value" and not a panic.
    #[test]
    fn a_missing_shift_is_zero() {
        let config = read_sprite_config(&config_json(""));

        assert_eq!(config.shift, Vec2::ZERO);
    }

    /// The shipped `sprite.json` is generated by a separate repository, so nothing
    /// but this stops the two drifting: a renamed key or a category the generator
    /// stopped emitting would otherwise only surface as a panic at startup.
    ///
    /// It reads the real 11 MB file, which is why it is the one slow test here.
    #[test]
    fn the_shipped_config_loads() {
        let configs = read_sprites_config();

        assert_eq!(configs.items.len(), 41841);
        assert_eq!(configs.outfits.len(), 1404);
        assert_eq!(configs.effects.len(), 207);
        assert_eq!(configs.missiles.len(), 56);

        // Effect 1 is the red hit splash: six phases, played once.
        let effect = configs
            .effects
            .get(&EffectId(1))
            .expect("effect 1 is present");
        assert_eq!(effect.animation.total_animation_phases(), 6);
        assert_eq!(
            effect.animation.loop_mode(),
            AnimationLoop::Counted { count: 1 }
        );

        // Missiles are static and 3x3 -- eight flight directions plus the centre --
        // and `boxes` is indexed by pattern_x, so it is shorter than an outfit's.
        let missile = configs
            .missiles
            .get(&MissileId(1))
            .expect("missile 1 is present");
        assert!(matches!(missile.animation, SpriteAnimation::Static));
        assert_eq!(missile.pattern_x, 3);
        assert_eq!(missile.boxes.len(), 3);
    }

    /// Three assumptions the effect renderer is built on, all read off the
    /// shape of the shipped data rather than documented anywhere it generates
    /// from.
    #[test]
    fn every_effect_is_shaped_the_way_the_renderer_assumes() {
        let configs = read_sprites_config();

        for effect in configs.effects.values() {
            // The instance's bbox is `boxes[pattern_x]`. Were they ever indexed
            // per phase, or by pattern_x * pattern_y, every patterned effect
            // would draw a wrong crop of its cell.
            //
            // This cannot tell the two pattern axes apart: every shipped effect
            // is square (pattern_x == pattern_y), so a swap of the axes at the
            // `init_instance` call site would pass this just as well. Proven
            // wrong only by the fact that no rectangular effect exists to show
            // it — not proof the indexing is axis-correct.
            assert_eq!(
                effect.boxes.len(),
                effect.pattern_x as usize,
                "effect {} has {} boxes for {} x-patterns",
                effect.id,
                effect.boxes.len(),
                effect.pattern_x
            );

            // An all-zero animated config would take `lifetime`'s
            // `never_advances` arm and get `STATIC_DURATION` instead of a real
            // playthrough -- rendering as a silent 300 ms still rather than an
            // animation. `settle_on_timed_phase` itself no longer depends on
            // this (its own doc records that `never_advances` plays no part in
            // its termination); this assertion is about the lifetime, not the
            // walk. Effect 221's `[0, 0]` tail is padding; a whole animation of
            // zeros would be something else.
            if !matches!(effect.animation, SpriteAnimation::Static) {
                assert!(
                    !effect.animation.never_advances(),
                    "effect {} is animated but no phase has any time on it",
                    effect.id
                );
            }

            // A `[0, n]` range is "timed" by `phase_is_untimed`, so it does not
            // trip `never_advances` and `lifetime` hands the effect no ttl --
            // but `phase_duration` samples it and can roll 0, which parks the
            // animator on `tick_sprite_animators`'s zero-duration guard for
            // good. Nothing would ever collect the entity or its instance
            // slot. Item 40576 ships `[0, 800]`, so this is a shape the data
            // really produces.
            if let SpriteAnimation::NonUniform { phases, .. } = &effect.animation {
                for (i, range) in phases.iter().enumerate() {
                    assert!(
                        range.x > 0 || range.y == 0,
                        "effect {} phase {i} has range {range:?}: a zero sample would freeze it forever",
                        effect.id
                    );
                }
            }
        }
    }

    fn animation_json(body: &str) -> SpriteAnimation {
        read_animation(&serde_json::from_str::<Value>(body).unwrap())
    }

    /// The loop mode drives whether an effect is a one-shot or a permanent field
    /// effect, so it has to survive the trip through the config.
    #[test]
    fn a_counted_animation_carries_its_count() {
        let animation = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": 5,
                "phase_count": 3, "phase_duration": 100, "phases": null}"#,
        );

        assert_eq!(animation.loop_mode(), AnimationLoop::Counted { count: 5 });
        assert_eq!(animation.total_animation_phases(), 3);
    }

    #[test]
    fn the_endless_loop_types_are_distinguished() {
        let infinite = animation_json(
            r#"{"loop_type": "INFINITE", "loop_count": null,
                "phase_count": 2, "phase_duration": 100, "phases": null}"#,
        );
        let pingpong = animation_json(
            r#"{"loop_type": "PINGPONG", "loop_count": null,
                "phase_count": 2, "phase_duration": 100, "phases": null}"#,
        );

        assert_eq!(infinite.loop_mode(), AnimationLoop::Infinite);
        assert_eq!(pingpong.loop_mode(), AnimationLoop::PingPong);
    }

    /// A COUNTED animation with no count is malformed. Reading it as a single run
    /// makes the effect disappear; reading it as endless would leak the slot.
    #[test]
    fn a_counted_animation_without_a_count_runs_once() {
        let animation = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": null,
                "phase_count": 2, "phase_duration": 100, "phases": null}"#,
        );

        assert_eq!(animation.loop_mode(), AnimationLoop::Counted { count: 1 });
    }

    /// A count of zero would finish the animation before its first phase is shown.
    #[test]
    fn a_zero_count_is_treated_as_one_run() {
        let animation = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": 0,
                "phase_count": 2, "phase_duration": 100, "phases": null}"#,
        );

        assert_eq!(animation.loop_mode(), AnimationLoop::Counted { count: 1 });
    }

    /// Missiles have no animation at all, and a static sprite must not pretend to
    /// have a loop mode that matters.
    #[test]
    fn a_missing_animation_is_static() {
        let animation = animation_json("null");

        assert!(matches!(animation, SpriteAnimation::Static));
        assert_eq!(animation.total_animation_phases(), 1);
    }

    /// A non-zero shift must not disturb the bounding box it is applied on top of:
    /// the two are independent inputs to `calculate_world_pos`.
    #[test]
    fn shift_does_not_disturb_the_boxes() {
        let shifted = read_sprite_config(&config_json(r#", "shift": [8, 8]"#));
        let unshifted = read_sprite_config(&config_json(""));

        assert_eq!(shifted.boxes, unshifted.boxes);
    }

    /// A uniform animation holds every phase for the same time, so the phase
    /// index must not change the answer.
    #[test]
    fn a_uniform_animation_reports_one_duration_for_every_phase() {
        let animation = animation_json(
            r#"{"loop_type": "INFINITE", "loop_count": null,
                "phase_count": 3, "phase_duration": 120, "phases": null}"#,
        );

        for phase in 0..3 {
            assert_eq!(animation.phase_duration(phase), Duration::from_millis(120));
        }
    }

    /// 77 of the 207 effects are non-uniform. Reading one shared duration off
    /// them is what left them frozen on phase 0.
    #[test]
    fn a_non_uniform_animation_reports_each_phases_own_duration() {
        let animation = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": 1, "phase_count": null,
                "phase_duration": null, "phases": [[100, 100], [250, 250], [50, 50]]}"#,
        );

        assert_eq!(animation.phase_duration(0), Duration::from_millis(100));
        assert_eq!(animation.phase_duration(1), Duration::from_millis(250));
        assert_eq!(animation.phase_duration(2), Duration::from_millis(50));
    }

    /// Effect 41 carries `[1, 250]`, and 491 item phases carry ranges too — the
    /// torches and campfires. The range is the point: sampling is what keeps
    /// them out of lockstep.
    ///
    /// A 301-value range makes 64 samples landing on the same value fail
    /// only by astronomical bad luck, so the distinctness check is not a
    /// flake risk. Without it, deleting the sampling arm entirely -- always
    /// returning `range.x` -- passed every other test in this file; this is
    /// the one that actually exercises `fastrand`.
    #[test]
    fn a_ranged_phase_samples_inside_its_range() {
        let animation = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": 1, "phase_count": null,
                "phase_duration": null, "phases": [[100, 400]]}"#,
        );

        let mut samples = std::collections::HashSet::new();
        for _ in 0..64 {
            let sampled = animation.phase_duration(0);
            assert!(
                sampled >= Duration::from_millis(100) && sampled <= Duration::from_millis(400),
                "sampled {sampled:?} outside [100ms, 400ms]"
            );
            samples.insert(sampled);
        }
        assert!(
            samples.len() > 1,
            "64 samples over a 301-value range all came back identical"
        );
    }

    /// Emptiness decides whether a phase is skipped, so it must be read off the
    /// range and never off a sample — otherwise a `[0, n]` phase would be
    /// skipped or kept depending on the roll.
    #[test]
    fn a_phase_is_untimed_only_when_its_whole_range_is_zero() {
        let animation = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": 1, "phase_count": null,
                "phase_duration": null, "phases": [[100, 100], [0, 0], [0, 250]]}"#,
        );

        assert!(!animation.phase_is_untimed(0));
        assert!(animation.phase_is_untimed(1));
        assert!(
            !animation.phase_is_untimed(2),
            "a [0, 250] phase can still take time"
        );
    }

    /// The `Uniform` arm has its own zero check, separate from `NonUniform`'s
    /// range check -- a `phase_duration: 0` config is what an item with a
    /// static-looking but technically animated appearance would produce.
    #[test]
    fn a_uniform_phase_is_untimed_only_at_zero_duration() {
        let zero_duration = animation_json(
            r#"{"loop_type": "INFINITE", "loop_count": null,
                "phase_count": 2, "phase_duration": 0, "phases": null}"#,
        );
        let timed = animation_json(
            r#"{"loop_type": "INFINITE", "loop_count": null,
                "phase_count": 2, "phase_duration": 100, "phases": null}"#,
        );

        assert!(zero_duration.phase_is_untimed(0));
        assert!(!timed.phase_is_untimed(0));
    }

    /// This is the guard that makes the skip loop in `SpriteAnimator` safe: an
    /// animation with no phase worth waiting on is never ticked at all.
    #[test]
    fn an_animation_with_no_timed_phase_never_advances() {
        let all_zero = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": 1, "phase_count": null,
                "phase_duration": null, "phases": [[0, 0], [0, 0]]}"#,
        );
        let padded_tail = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": 1, "phase_count": null,
                "phase_duration": null, "phases": [[100, 100], [0, 0]]}"#,
        );

        assert!(all_zero.never_advances());
        assert!(
            animation_json("null").never_advances(),
            "a static animation"
        );
        assert!(
            !padded_tail.never_advances(),
            "effect 221's shape still runs"
        );
    }

    /// The lifetime of an endless effect is one pass over its phases.
    #[test]
    fn a_pass_sums_every_phase() {
        let uniform = animation_json(
            r#"{"loop_type": "INFINITE", "loop_count": null,
                "phase_count": 8, "phase_duration": 100, "phases": null}"#,
        );
        let non_uniform = animation_json(
            r#"{"loop_type": "COUNTED", "loop_count": 1, "phase_count": null,
                "phase_duration": null, "phases": [[100, 100], [250, 250]]}"#,
        );

        assert_eq!(uniform.pass_duration(), Duration::from_millis(800));
        assert_eq!(non_uniform.pass_duration(), Duration::from_millis(350));
        assert_eq!(animation_json("null").pass_duration(), Duration::ZERO);
    }
}
