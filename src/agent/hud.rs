use bevy::prelude::*;

use crate::agent::components::{Agent, AgentHud, HealthState, Hud};
use crate::agent::{DisplayName, Health, HudBar, Mana};
use crate::camera::GameCamera;
use crate::conf::viewport::{GAME_VIEW_HEIGHT, GAME_VIEW_WIDTH};
use crate::game_ui::{
    GameViewport,
    scaling::{logical_size, snap_to_physical},
};
use crate::map::Position;
use crate::player::components::Player;

pub fn attach_huds_to_viewport(
    mut commands: Commands,
    viewport_q: Query<Entity, With<GameViewport>>,
    orphan_huds: Query<Entity, (With<Hud>, Without<ChildOf>)>,
) {
    let Ok(viewport) = viewport_q.single() else {
        return;
    };
    for hud in orphan_huds.iter() {
        commands.entity(viewport).add_child(hud);
    }
}

/// Places every agent's HUD over its sprite, in viewport-local pixels.
///
/// Reads `Transform`, **not** `GlobalTransform`. This runs in `PostUpdate` before
/// `UiSystems::Layout` — which is where it has to be, because that is the system
/// that consumes the `UiTransform` written below, and `bevy_ui` orders layout
/// *before* `TransformSystems::Propagate`. So at this point `GlobalTransform`
/// still holds last frame's values, while the sprites render from this frame's.
/// Reading it put every label one frame behind the creature it names — invisible
/// on the player, whose offset from the camera never changes, and a visible
/// shimmy on everything else. Agents are children of a floor entity whose
/// transform is only ever identity (`map::floors`), so the two are equal in
/// value; only the freshness differs.
pub fn update_hud_positions(
    mut commands: Commands,
    game_cam_q: Query<&Transform, With<GameCamera>>,
    player_pos_q: Query<&Position, With<Player>>,
    agents_q: Query<(&Transform, &AgentHud, &Position), With<Agent>>,
    mut hud_q: Query<(&mut UiTransform, &ComputedNode), With<Hud>>,
    viewport_q: Query<&ComputedNode, With<GameViewport>>,
) {
    let Ok(game_cam_tf) = game_cam_q.single() else {
        return;
    };
    let Ok(computed) = viewport_q.single() else {
        return;
    };
    let Ok(player_pos) = player_pos_q.single() else {
        return;
    };

    let cam_pos = game_cam_tf.translation.truncate();

    for (agent_tf, display_name, position) in agents_q.iter() {
        if position.z != player_pos.z {
            commands
                .entity(display_name.main_entity)
                .insert(Visibility::Hidden);
            continue;
        }

        commands
            .entity(display_name.main_entity)
            .insert(Visibility::Visible);

        let Ok((mut tranf, hud_node)) = hud_q.get_mut(display_name.main_entity) else {
            continue;
        };

        let world_pos = agent_tf.translation.truncate();
        let snapped = hud_translation(
            world_pos,
            cam_pos,
            computed,
            logical_size(hud_node),
            display_name.world_y_offset,
        );
        tranf.translation = Val2::new(Val::Px(snapped.x), Val::Px(snapped.y));
    }
}

fn hud_translation(
    world_pos: Vec2,
    cam_pos: Vec2,
    viewport: &ComputedNode,
    hud_size: Vec2,
    world_y_offset: f32,
) -> Vec2 {
    // Normalize to [0, 1] UV within the game view (mirrors update_hover_state in reverse).
    let uv = Vec2::new(
        (world_pos.x - cam_pos.x) / GAME_VIEW_WIDTH + 0.5,
        (0.5 - (world_pos.y - cam_pos.y) / GAME_VIEW_HEIGHT) - world_y_offset / GAME_VIEW_HEIGHT,
    );

    // HUD nodes are children of the GameViewport, so this is viewport-local
    // (y-down, top-left origin). The viewport's Overflow::clip() then pixel-clips
    // the HUD at the view edge as the agent walks out of frame.
    let local_px = uv * logical_size(viewport) - hud_size / 2.0;

    // Snapped on the physical grid, not the logical one: `Val::Px` is logical,
    // so `.round()` here snapped in steps of the display scale factor and put
    // the label off the grid its own glyphs land on at anything but 100%.
    snap_to_physical(viewport, local_px)
}

pub fn update_display_name_health_state(
    actors_q: Query<(&AgentHud, &Health), Changed<Health>>,
    mut state_q: Query<&mut HealthState, With<DisplayName>>,
) {
    for (actor_hud, health) in actors_q.iter() {
        if let Ok(mut state) = state_q.get_mut(actor_hud.display_name) {
            *state = HealthState::from_ratio(health.ratio());
        }
    }
}

pub fn update_display_name_color(
    mut display_names_q: Query<
        (&HealthState, &mut TextColor),
        (With<DisplayName>, Changed<HealthState>),
    >,
) {
    for (health_state, mut color) in display_names_q.iter_mut() {
        color.0 = health_state.color();
    }
}

pub fn update_hud_bar_ratios(
    agents_q: Query<
        (&AgentHud, Option<&Health>, Option<&Mana>),
        Or<(Changed<Health>, Changed<Mana>)>,
    >,
    mut hud_bars_q: Query<&mut HudBar>,
) {
    for (agent_hud, health, mana) in agents_q.iter() {
        if let Some(health) = health
            && let Ok(mut hud_bar) = hud_bars_q.get_mut(agent_hud.health_bar.unwrap())
        {
            hud_bar.ratio = health.ratio();
        }
        if let Some(mana) = mana
            && let Ok(mut hud_bar) = hud_bars_q.get_mut(agent_hud.mana_bar.unwrap())
        {
            hud_bar.ratio = mana.ratio();
        }
    }
}

pub fn update_hud_bar_health_state(
    mut health_q: Query<(&HudBar, &mut HealthState), Changed<HudBar>>,
) {
    for (bar, mut state) in health_q.iter_mut() {
        *state = HealthState::from_ratio(bar.ratio);
    }
}

pub fn update_hud_bar_colors(
    mut hud_bars_q: Query<
        (&HealthState, &mut BackgroundColor),
        (With<HudBar>, Changed<HealthState>),
    >,
) {
    for (health_state, mut bg) in hud_bars_q.iter_mut() {
        bg.0 = health_state.color();
    }
}

pub fn resize_hud_fill(mut hud_bars_q: Query<(&mut Node, &HudBar), Changed<HudBar>>) {
    for (mut node, hud_bar) in hud_bars_q.iter_mut() {
        node.width = Val::Percent(hud_bar.ratio * 100.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Change detection needs the system's `last_run` to survive between runs, and a
    /// `RunSystemOnce` initialises a fresh system every call — under which `Changed<T>`
    /// matches everything and a filter bug cannot fail a test. A `Schedule` keeps it.
    fn a_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(update_hud_bar_ratios);
        schedule
    }

    fn a_hud_agent(world: &mut World) -> (Entity, Entity, Entity) {
        let health_bar = world.spawn(HudBar { ratio: 1.0 }).id();
        let mana_bar = world.spawn(HudBar { ratio: 1.0 }).id();
        let display_name = world.spawn_empty().id();
        let main_entity = world.spawn_empty().id();
        let agent = world
            .spawn((
                AgentHud {
                    main_entity,
                    health_bar: Some(health_bar),
                    mana_bar: Some(mana_bar),
                    display_name,
                    world_y_offset: 0.0,
                },
                Health {
                    current: 100,
                    max: 100,
                },
                Mana {
                    current: 100,
                    max: 100,
                },
            ))
            .id();
        (agent, health_bar, mana_bar)
    }

    /// The bug this pins: the system writes both bars but woke only on `Changed<Health>`,
    /// so the mana bar sat at whatever ratio the last hit left it at. Casting and drinking
    /// move the mana and nothing else, which is a state a player stays in indefinitely.
    #[test]
    fn a_mana_only_change_moves_the_mana_bar() {
        let mut world = World::new();
        let (agent, health_bar, mana_bar) = a_hud_agent(&mut world);
        let mut schedule = a_schedule();

        schedule.run(&mut world);
        world.clear_trackers();

        world.get_mut::<Mana>(agent).unwrap().current = 50;
        schedule.run(&mut world);

        assert_eq!(world.get::<HudBar>(mana_bar).unwrap().ratio, 0.5);
        assert_eq!(
            world.get::<HudBar>(health_bar).unwrap().ratio,
            1.0,
            "the life was untouched"
        );
    }

    /// Builds a viewport `ComputedNode` of `logical` size at `scale_factor`, the same
    /// shape `game_ui::scaling`'s tests use.
    fn a_viewport(scale_factor: f32) -> ComputedNode {
        ComputedNode {
            size: Vec2::new(GAME_VIEW_WIDTH, GAME_VIEW_HEIGHT) * scale_factor,
            inverse_scale_factor: 1.0 / scale_factor,
            ..Default::default()
        }
    }

    /// The bug: the HUD column is `width: auto`, so a long name widens it, and
    /// `UiTransform` places a node by its **centre**. Two agents standing on the same
    /// tile with different name lengths must still put their HUDs over the same point --
    /// before this, the wider one drifted right by half the extra width.
    ///
    /// 30.0 is `HUD_BAR_WIDTH`, the floor a short name cannot shrink the column past;
    /// 66.0 is roughly what a twelve-character name measures at font size 11.
    #[test]
    fn a_long_name_and_a_short_one_centre_on_the_same_point() {
        let viewport = a_viewport(1.0);
        let world = Vec2::new(320.0, -160.0);
        let cam = Vec2::new(300.0, -150.0);

        let centre_of = |width: f32| {
            hud_translation(world, cam, &viewport, Vec2::new(width, 21.0), 30.5).x + width / 2.0
        };

        assert_eq!(
            centre_of(30.0),
            centre_of(66.0),
            "a longer name must not move the HUD off its agent"
        );
    }

    /// The correction is a real subtraction, not a no-op that the assertion above would
    /// also accept if `hud_translation` ignored `hud_width` entirely.
    #[test]
    fn a_wider_hud_starts_further_left() {
        let viewport = a_viewport(1.0);
        let world = Vec2::new(320.0, -160.0);
        let cam = Vec2::new(300.0, -150.0);

        let narrow = hud_translation(world, cam, &viewport, Vec2::new(30.0, 21.0), 30.5).x;
        let wide = hud_translation(world, cam, &viewport, Vec2::new(66.0, 21.0), 30.5).x;

        assert_eq!(narrow - wide, 18.0, "half the 36px difference in width");
    }

    /// Half a width can be a half pixel, and the snap is what keeps glyphs on the
    /// physical grid — so it has to happen *after* the correction, not before.
    #[test]
    fn the_centred_position_still_lands_on_the_physical_grid() {
        for scale_factor in [1.0, 1.25, 1.5, 2.0] {
            let viewport = a_viewport(scale_factor);
            let x = hud_translation(
                Vec2::new(320.0, -160.0),
                Vec2::new(300.0, -150.0),
                &viewport,
                Vec2::new(33.0, 21.0),
                30.5,
            );
            // Tolerance, not equality: multiplying back by the scale factor reintroduces
            // the float error the snap just removed, so an exact compare fails on the
            // round trip rather than on the grid.
            let physical = x * scale_factor;
            let off_grid = (physical - physical.round()).abs().max_element();
            assert!(
                off_grid < 1e-3,
                "physical {physical} is {off_grid} off the grid at {scale_factor}x"
            );
        }
    }

    /// The vertical half of the same correction, and the bug it closes: a creature carries
    /// one bar and a player two, so their columns are different heights. Both must end up
    /// centred on the same point above their agent -- the per-bar constant this replaced
    /// added a whole `HUD_BAR_HEIGHT` where half of one was wanted, so the two drifted
    /// apart by a bar and a creature's label sat low.
    #[test]
    fn a_one_bar_and_a_two_bar_hud_centre_on_the_same_point() {
        let viewport = a_viewport(1.0);
        let world = Vec2::new(320.0, -160.0);
        let cam = Vec2::new(300.0, -150.0);

        // A name line plus one bar, against a name line plus two.
        let creature = Vec2::new(30.0, 17.0);
        let player = Vec2::new(30.0, 21.0);

        let centre_of =
            |size: Vec2| hud_translation(world, cam, &viewport, size, 30.5).y + size.y / 2.0;

        assert_eq!(
            centre_of(creature),
            centre_of(player),
            "the number of bars must not move where the column is anchored"
        );
    }

    /// The other half of the `Or`: health alone must still wake it.
    #[test]
    fn a_health_only_change_moves_the_health_bar() {
        let mut world = World::new();
        let (agent, health_bar, mana_bar) = a_hud_agent(&mut world);
        let mut schedule = a_schedule();

        schedule.run(&mut world);
        world.clear_trackers();

        world.get_mut::<Health>(agent).unwrap().current = 25;
        schedule.run(&mut world);

        assert_eq!(world.get::<HudBar>(health_bar).unwrap().ratio, 0.25);
        assert_eq!(
            world.get::<HudBar>(mana_bar).unwrap().ratio,
            1.0,
            "the mana was untouched"
        );
    }
}
