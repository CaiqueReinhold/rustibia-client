use bevy::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use crate::core::sprite::{AnimationLoop, SpriteConfig};

pub const MAX_LAYERS: usize = 6;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationSet;

#[derive(Component)]
pub struct SpriteAnimator {
    pub config: Arc<SpriteConfig>,
    pub current_sprite_ids: [u32; MAX_LAYERS],
    pub timer: Timer,
    pub current_phase: u32,
    pub pattern_x: u32,
    pub pattern_y: u32,
    pub pattern_z: u32,
    pub moving_animation: bool,
    loops_completed: u32,
    descending: bool,
    finished: bool,
}

impl SpriteAnimator {
    pub fn new(config: Arc<SpriteConfig>, pattern_x: u32, pattern_y: u32, pattern_z: u32) -> Self {
        let mut s = SpriteAnimator {
            config,
            current_sprite_ids: [0; MAX_LAYERS],
            // Replaced below unless the animation never advances at all, in which
            // case a zero-duration timer is what `tick_sprite_animators` skips.
            timer: Timer::new(Duration::ZERO, TimerMode::Once),
            current_phase: 0,
            pattern_x,
            pattern_y,
            pattern_z,
            moving_animation: false,
            loops_completed: 0,
            descending: false,
            finished: false,
        };
        if !s.config.animation.never_advances() {
            s.settle_on_timed_phase();
            s.timer = Timer::new(
                s.config.animation.phase_duration(s.current_phase),
                TimerMode::Repeating,
            );
        }
        resolve_simple_sprite_ids(&mut s);
        s
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// One phase forward, then settle onto a phase that is actually displayed,
    /// then re-point the timer at however long that phase lasts.
    fn advance(&mut self) {
        self.step();
        self.settle_on_timed_phase();
        self.timer
            .set_duration(self.config.animation.phase_duration(self.current_phase));
    }

    /// Moves off any phase the config gives no time to, so the animator always
    /// rests on one that is actually displayed -- forward while the run is still
    /// going, backward once it has finished on padding.
    ///
    /// The `2n` bound covers PingPong, whose walk is a cycle of length `2n - 2`
    /// rather than `n`: it revisits every interior phase twice per lap.
    fn settle_on_timed_phase(&mut self) {
        let phase_count = self.config.animation.total_animation_phases();
        for _ in 0..2 * phase_count {
            if self.finished || !self.config.animation.phase_is_untimed(self.current_phase) {
                break;
            }
            self.step();
        }

        if self.finished && self.config.animation.phase_is_untimed(self.current_phase) {
            self.rest_on_last_timed_phase();
        }
    }

    /// Walks backward from a finished run's untimed tail to the last phase the
    /// config actually gave time to -- the frame a despawn observer sees, and,
    /// for an item, the frame it is stuck showing for good. Without it effect
    /// 221 rests on its padding, as do items 22679 and 24925.
    fn rest_on_last_timed_phase(&mut self) {
        while self.current_phase > 0 && self.config.animation.phase_is_untimed(self.current_phase) {
            self.current_phase -= 1;
        }
    }

    /// One phase forward, honouring the loop mode, without re-arming the timer --
    /// `settle_on_timed_phase` calls this for every internal hop.
    fn step(&mut self) {
        let phase_count = self.config.animation.total_animation_phases();
        // A one-phase animation has nowhere to advance to. Counted still has to
        // finish, or a one-phase effect would hang around for ever.
        if phase_count <= 1 {
            if let AnimationLoop::Counted { count } = self.config.animation.loop_mode() {
                self.loops_completed += 1;
                self.finished = self.loops_completed >= count;
            }
            return;
        }

        match self.config.animation.loop_mode() {
            AnimationLoop::Infinite => {
                self.current_phase = (self.current_phase + 1) % phase_count;
            }
            AnimationLoop::PingPong => {
                if self.descending {
                    self.current_phase -= 1;
                    self.descending = self.current_phase > 0;
                } else {
                    self.current_phase += 1;
                    self.descending = self.current_phase >= phase_count - 1;
                }
            }
            AnimationLoop::Counted { count } => {
                let next = self.current_phase + 1;
                if next < phase_count {
                    self.current_phase = next;
                    return;
                }
                self.loops_completed += 1;
                if self.loops_completed >= count {
                    self.finished = true;
                } else {
                    self.current_phase = 0;
                }
            }
        }
    }
}

pub fn resolve_simple_sprite_ids(animator: &mut SpriteAnimator) {
    let config = &animator.config;
    let phase = animator.current_phase;
    for layer in 0..config.layers.min(MAX_LAYERS as u32) as usize {
        let index = (((phase * config.pattern_z + animator.pattern_z) * config.pattern_y
            + animator.pattern_y)
            * config.pattern_x
            + animator.pattern_x)
            * config.layers
            + layer as u32;
        animator.current_sprite_ids[layer] =
            config.sprite_ids.get(index as usize).copied().unwrap_or(0);
    }
}

pub fn tick_sprite_animators(time: Res<Time>, mut query: Query<&mut SpriteAnimator>) {
    for mut animator in &mut query {
        if animator.timer.duration().is_zero()
            || animator.moving_animation
            || animator.is_finished()
        {
            continue;
        }

        // Ticking through the `Mut` would mark the animator `Changed` every
        // frame, and every `Changed<SpriteAnimator>` filter downstream would stop
        // filtering. The advance below is the only observable change, so it is
        // the only one flagged.
        let inner = animator.bypass_change_detection();
        inner.timer.tick(time.delta());
        if !inner.timer.just_finished() {
            continue;
        }

        inner.advance();
        resolve_simple_sprite_ids(inner);
        animator.set_changed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::sprite::SpriteAnimation;
    use bevy::ecs::system::RunSystemOnce;

    const PHASE: Duration = Duration::from_millis(100);

    /// One layer, no patterns, so the sprite id is just the phase index times ten.
    fn config(phase_count: u32, loop_mode: AnimationLoop) -> Arc<SpriteConfig> {
        Arc::new(SpriteConfig {
            id: 1,
            group: "test".to_string(),
            pattern_x: 1,
            pattern_y: 1,
            pattern_z: 1,
            layers: 1,
            sprite_ids: (0..phase_count).map(|p| (p + 1) * 10).collect(),
            animation: SpriteAnimation::Uniform {
                loop_mode,
                phase_count,
                phase_duration: PHASE,
            },
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        })
    }

    /// Two phases, infinite: the shape the change-detection tests were written for.
    fn two_phase_config() -> Arc<SpriteConfig> {
        config(2, AnimationLoop::Infinite)
    }

    /// Drives `advance` directly over `steps` phase advances and reports the phase
    /// after each one, paired with whether the animation had finished by then.
    fn advance_phases(config: Arc<SpriteConfig>, steps: usize) -> Vec<(u32, bool)> {
        let mut animator = SpriteAnimator::new(config, 0, 0, 0);
        (0..steps)
            .map(|_| {
                animator.advance();
                (animator.current_phase, animator.is_finished())
            })
            .collect()
    }

    fn spawn_animator(world: &mut World) -> Entity {
        world.insert_resource(Time::<()>::default());
        let entity = world
            .spawn(SpriteAnimator::new(two_phase_config(), 0, 0, 0))
            .id();
        world.clear_trackers();
        entity
    }

    /// Runs one frame and reports the animator's change tick and sprite id.
    fn tick(world: &mut World, entity: Entity, delta: Duration) -> (u32, u32) {
        world.resource_mut::<Time<()>>().advance_by(delta);
        world.run_system_once(tick_sprite_animators).unwrap();
        (changed_tick(world, entity), sprite_id(world, entity))
    }

    fn changed_tick(world: &World, entity: Entity) -> u32 {
        world
            .entity(entity)
            .get_change_ticks::<SpriteAnimator>()
            .unwrap()
            .changed
            .get()
    }

    fn sprite_id(world: &World, entity: Entity) -> u32 {
        world
            .entity(entity)
            .get::<SpriteAnimator>()
            .unwrap()
            .current_sprite_ids[0]
    }

    /// The whole point: a frame that only advances the timer must not mark the
    /// animator changed, or the `Changed<SpriteAnimator>` filters downstream
    /// match everything and every animated entity is re-derived, compared, and
    /// re-labelled each frame.
    #[test]
    fn a_tick_without_a_phase_advance_is_not_a_change() {
        let mut world = World::new();
        let entity = spawn_animator(&mut world);
        let before = changed_tick(&world, entity);

        let (changed, sprite_id) = tick(&mut world, entity, PHASE / 10);

        assert_eq!(changed, before, "no phase advance, so no change");
        assert_eq!(sprite_id, 10, "still on the first phase");
    }

    /// …and the bypass must not swallow the elapsed time: the partial ticks
    /// still accumulate, and the frame that completes the phase does report a
    /// change.
    #[test]
    fn the_phase_advance_accumulates_and_is_reported_as_a_change() {
        let mut world = World::new();
        let entity = spawn_animator(&mut world);
        let before = changed_tick(&world, entity);

        for _ in 0..4 {
            let (changed, sprite_id) = tick(&mut world, entity, PHASE / 5);
            assert_eq!(changed, before, "still short of a full phase");
            assert_eq!(sprite_id, 10);
        }

        let (changed, sprite_id) = tick(&mut world, entity, PHASE / 5);
        assert_ne!(changed, before, "the phase advanced");
        assert_eq!(sprite_id, 20, "and the sprite followed it");
    }

    /// Every animated outfit is infinite, so this is the path creatures take:
    /// wrap forever and never report finished.
    #[test]
    fn an_infinite_animation_wraps_and_never_finishes() {
        let phases = advance_phases(config(3, AnimationLoop::Infinite), 7);

        assert_eq!(
            phases,
            vec![
                (1, false),
                (2, false),
                (0, false),
                (1, false),
                (2, false),
                (0, false),
                (1, false)
            ]
        );
    }

    /// A counted animation holds its last phase rather than snapping back, so the
    /// frame a despawn observer sees is the one the artist ended on.
    #[test]
    fn a_counted_animation_stops_on_its_last_phase() {
        let phases = advance_phases(config(3, AnimationLoop::Counted { count: 1 }), 4);

        assert_eq!(
            phases,
            vec![(1, false), (2, false), (2, true), (2, true)],
            "phases 0,1,2 then finished, holding 2"
        );
    }

    /// `count` is a number of runs, not of phases: 186 of the 207 effects are
    /// count 1, but a handful run several times before they are over.
    #[test]
    fn a_counted_animation_runs_once_per_count() {
        let phases = advance_phases(config(2, AnimationLoop::Counted { count: 2 }), 5);

        assert_eq!(
            phases,
            vec![(1, false), (0, false), (1, false), (1, true), (1, true)],
            "0,1 then 0,1 -- two runs -- then finished"
        );
    }

    /// A single-phase counted animation has nowhere to advance to, but it still has
    /// to finish or the effect it belongs to would never be despawned.
    #[test]
    fn a_single_phase_counted_animation_still_finishes() {
        let phases = advance_phases(config(1, AnimationLoop::Counted { count: 1 }), 2);

        assert_eq!(phases, vec![(0, true), (0, true)]);
    }

    /// A single-phase endless animation must not finish, and must not divide by or
    /// modulo its way into a panic.
    #[test]
    fn a_single_phase_endless_animation_holds_still() {
        assert_eq!(
            advance_phases(config(1, AnimationLoop::Infinite), 3),
            vec![(0, false), (0, false), (0, false)]
        );
        assert_eq!(
            advance_phases(config(1, AnimationLoop::PingPong), 3),
            vec![(0, false), (0, false), (0, false)]
        );
    }

    /// Ping-pong reverses at both ends without repeating the endpoint, which is
    /// what separates it from a wrap.
    #[test]
    fn a_pingpong_animation_walks_back_down() {
        let phases = advance_phases(config(4, AnimationLoop::PingPong), 8);

        assert_eq!(
            phases.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
            vec![1, 2, 3, 2, 1, 0, 1, 2]
        );
        assert!(
            phases.iter().all(|(_, finished)| !*finished),
            "ping-pong never ends"
        );
    }

    /// A non-uniform config: three phases of 100 ms, 250 ms and 50 ms, whose
    /// sprite ids are 10, 20, 30.
    fn non_uniform_config() -> Arc<SpriteConfig> {
        Arc::new(SpriteConfig {
            id: 2,
            group: "test".to_string(),
            pattern_x: 1,
            pattern_y: 1,
            pattern_z: 1,
            layers: 1,
            sprite_ids: vec![10, 20, 30],
            animation: SpriteAnimation::NonUniform {
                loop_mode: AnimationLoop::Counted { count: 1 },
                phases: vec![
                    UVec2::new(100, 100),
                    UVec2::new(250, 250),
                    UVec2::new(50, 50),
                ],
            },
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        })
    }

    fn spawn_with(world: &mut World, config: Arc<SpriteConfig>) -> Entity {
        world.insert_resource(Time::<()>::default());
        let entity = world.spawn(SpriteAnimator::new(config, 0, 0, 0)).id();
        world.clear_trackers();
        entity
    }

    /// Before this, a non-uniform animator got a zero-duration timer, was
    /// skipped by the guard in `tick_sprite_animators`, and sat on phase 0 for
    /// ever — 77 of the 207 effects and 946 items.
    #[test]
    fn a_non_uniform_animation_advances_on_each_phases_own_duration() {
        let mut world = World::new();
        let entity = spawn_with(&mut world, non_uniform_config());

        // Phase 0 lasts 100 ms.
        assert_eq!(tick(&mut world, entity, Duration::from_millis(99)).1, 10);
        assert_eq!(tick(&mut world, entity, Duration::from_millis(1)).1, 20);

        // Phase 1 lasts 250 ms, not another 100.
        assert_eq!(tick(&mut world, entity, Duration::from_millis(100)).1, 20);
        assert_eq!(tick(&mut world, entity, Duration::from_millis(149)).1, 20);
        assert_eq!(tick(&mut world, entity, Duration::from_millis(1)).1, 30);
    }

    /// A counted animation holds its last phase and then reports finished.
    ///
    /// Note the tick sizes: `tick_sprite_animators` advances at most ONE phase
    /// per frame, however much time is handed to it, so a run has to be walked
    /// phase by phase rather than jumped in one big delta.
    #[test]
    fn a_non_uniform_counted_animation_finishes_after_its_last_phase() {
        let mut world = World::new();
        let entity = spawn_with(&mut world, non_uniform_config());

        tick(&mut world, entity, Duration::from_millis(100)); // -> phase 1
        tick(&mut world, entity, Duration::from_millis(250)); // -> phase 2
        tick(&mut world, entity, Duration::from_millis(49));
        assert!(
            !world
                .entity(entity)
                .get::<SpriteAnimator>()
                .unwrap()
                .is_finished(),
            "1 ms of the last phase still to run"
        );

        tick(&mut world, entity, Duration::from_millis(1));
        assert!(
            world
                .entity(entity)
                .get::<SpriteAnimator>()
                .unwrap()
                .is_finished()
        );
    }

    /// Effect 221's shape: real phases, then a tail of `[0, 0]` padding. The
    /// tail must cost one frame, not stall the animator for ever -- a stalled
    /// counted animation never reports finished, and the effect entity that
    /// waits on it is never despawned.
    #[test]
    fn a_zero_duration_tail_is_crossed_in_one_tick() {
        let config = Arc::new(SpriteConfig {
            id: 3,
            group: "test".to_string(),
            pattern_x: 1,
            pattern_y: 1,
            pattern_z: 1,
            layers: 1,
            sprite_ids: vec![10, 20, 30, 40],
            animation: SpriteAnimation::NonUniform {
                loop_mode: AnimationLoop::Counted { count: 1 },
                phases: vec![
                    UVec2::new(100, 100),
                    UVec2::new(100, 100),
                    UVec2::ZERO,
                    UVec2::ZERO,
                ],
            },
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        });
        let mut world = World::new();
        let entity = spawn_with(&mut world, config);

        tick(&mut world, entity, Duration::from_millis(100));
        tick(&mut world, entity, Duration::from_millis(100));

        let animator = world.entity(entity).get::<SpriteAnimator>().unwrap();
        assert!(
            animator.is_finished(),
            "the padded tail must not hold the animation open"
        );
        assert_eq!(
            animator.current_sprite_ids[0], 20,
            "must rest on phase 1's sprite, the last one the config timed -- \
             not phase 3's padding"
        );
    }

    /// A leading empty phase is displayed for zero time, so the animator must
    /// already be past it when it is first drawn.
    #[test]
    fn a_leading_empty_phase_is_skipped_at_construction() {
        let config = Arc::new(SpriteConfig {
            id: 4,
            group: "test".to_string(),
            pattern_x: 1,
            pattern_y: 1,
            pattern_z: 1,
            layers: 1,
            sprite_ids: vec![10, 20],
            animation: SpriteAnimation::NonUniform {
                loop_mode: AnimationLoop::Infinite,
                phases: vec![UVec2::ZERO, UVec2::new(100, 100)],
            },
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        });

        let animator = SpriteAnimator::new(config, 0, 0, 0);

        assert_eq!(animator.current_phase, 1);
        assert_eq!(animator.current_sprite_ids[0], 20);
    }

    /// `settle_on_timed_phase` never runs at all here: `never_advances` (see `new`)
    /// leaves the timer on its zero-duration `Once` sentinel for an all-empty
    /// config, and `tick_sprite_animators` skips a zero-duration timer
    /// outright. That is what holds this still -- not the skip loop's bound,
    /// which this test does not exercise.
    #[test]
    fn an_all_empty_animation_holds_still_without_spinning() {
        let config = Arc::new(SpriteConfig {
            id: 5,
            group: "test".to_string(),
            pattern_x: 1,
            pattern_y: 1,
            pattern_z: 1,
            layers: 1,
            sprite_ids: vec![10, 20],
            animation: SpriteAnimation::NonUniform {
                loop_mode: AnimationLoop::Infinite,
                phases: vec![UVec2::ZERO, UVec2::ZERO],
            },
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        });
        let mut world = World::new();
        let entity = spawn_with(&mut world, config);

        let (_, sprite_id) = tick(&mut world, entity, Duration::from_secs(10));

        assert_eq!(sprite_id, 10, "nothing to advance to");
    }

    /// PingPong's walk is a cycle of length `2n - 2`, not `n`: for these 4
    /// phases that's 6 steps, not 4. A bound of `n` exhausts after the config's
    /// only timed phase (index 0) has been passed once each direction but
    /// before the walk gets back to it, stranding the animator on phase 1 --
    /// untimed -- for ever: `tick_sprite_animators` skips a zero-duration timer,
    /// so a stranded animator freezes silently rather than erroring.
    #[test]
    fn a_pingpong_walk_recovers_a_timed_phase_past_the_short_bound() {
        let config = Arc::new(SpriteConfig {
            id: 6,
            group: "test".to_string(),
            pattern_x: 1,
            pattern_y: 1,
            pattern_z: 1,
            layers: 1,
            sprite_ids: vec![10, 20, 30, 40],
            animation: SpriteAnimation::NonUniform {
                loop_mode: AnimationLoop::PingPong,
                phases: vec![UVec2::new(100, 100), UVec2::ZERO, UVec2::ZERO, UVec2::ZERO],
            },
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        });
        let mut world = World::new();
        let entity = spawn_with(&mut world, config);

        // One full lap: 0 -> 1 -> 2 -> 3 -> 2 -> 1 -> 0, back to the only
        // timed phase, all inside the 100 ms tick that fires phase 0's timer.
        let (_, sprite_id) = tick(&mut world, entity, Duration::from_millis(100));

        assert_eq!(
            sprite_id, 10,
            "must recover phase 0, not strand on phase 1's padding"
        );
    }

    /// A finished animator stops consuming ticks, so a one-shot effect waiting to
    /// be despawned costs nothing and cannot re-dirty the instance buffer.
    #[test]
    fn a_finished_animator_stops_ticking() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let entity = world
            .spawn(SpriteAnimator::new(
                config(2, AnimationLoop::Counted { count: 1 }),
                0,
                0,
                0,
            ))
            .id();
        world.clear_trackers();

        // Two phase boundaries: one to reach the last phase, one to finish.
        tick(&mut world, entity, PHASE);
        tick(&mut world, entity, PHASE);
        assert!(
            world
                .entity(entity)
                .get::<SpriteAnimator>()
                .unwrap()
                .is_finished()
        );

        world.clear_trackers();
        let before = changed_tick(&world, entity);
        let (changed, _) = tick(&mut world, entity, PHASE * 10);

        assert_eq!(
            changed, before,
            "a finished animator reports no more changes"
        );
    }

    #[test]
    fn a_static_animator_never_changes() {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        let config = Arc::new(SpriteConfig {
            id: 1,
            group: "test".to_string(),
            pattern_x: 1,
            pattern_y: 1,
            pattern_z: 1,
            layers: 1,
            sprite_ids: vec![10],
            animation: SpriteAnimation::Static,
            boxes: Vec::new(),
            shift: Vec2::ZERO,
        });
        let entity = world.spawn(SpriteAnimator::new(config, 0, 0, 0)).id();
        world.clear_trackers();
        let before = changed_tick(&world, entity);

        let (changed, _) = tick(&mut world, entity, PHASE * 10);

        assert_eq!(changed, before);
    }
}
