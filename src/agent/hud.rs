use bevy::prelude::*;

use crate::agent::components::{Agent, AgentHud, HealthState};
use crate::agent::{DisplayName, Health, HudBar, Mana};
use crate::game_ui::Settings;
use crate::map::{Position, ViewportCenter, viewport::is_kept};
use crate::player::components::Player;

pub fn update_hud_visibility(
    center: Res<ViewportCenter>,
    player_pos_q: Query<&Position, With<Player>>,
    agents_q: Query<(&AgentHud, &Position), With<Agent>>,
    mut visibility_q: Query<&mut Visibility>,
) {
    let Ok(player_pos) = player_pos_q.single() else {
        return;
    };
    for (hud, position) in &agents_q {
        let in_window = center
            .0
            .as_ref()
            .is_none_or(|c| is_kept(c, Some(player_pos), position));
        let wanted = if position.z == player_pos.z && in_window {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Ok(mut visibility) = visibility_q.get_mut(hud.root) {
            visibility.set_if_neq(wanted);
        }
    }
}

pub fn apply_hud_settings(
    settings: Res<Settings>,
    huds_q: Query<Ref<AgentHud>>,
    parents_q: Query<&ChildOf>,
    mut visibility_q: Query<&mut Visibility>,
) {
    let mut show = |entity: Entity, shown: bool| {
        if let Ok(mut visibility) = visibility_q.get_mut(entity) {
            visibility.set_if_neq(if shown {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
        }
    };
    for hud in &huds_q {
        if !settings.is_changed() && !hud.is_added() {
            continue;
        }
        show(hud.name, settings.show_names);
        for fill in [hud.health_bar, hud.mana_bar].into_iter().flatten() {
            // The bar entities are the fills; `world_hud::spawn_bar` puts each inside a black frame.
            let frame = parents_q
                .get(fill)
                .map_or(fill, |child_of| child_of.parent());
            show(frame, settings.show_health_bars);
        }
    }
}

pub fn update_display_name_health_state(
    actors_q: Query<(&AgentHud, &Health), Changed<Health>>,
    mut state_q: Query<&mut HealthState, With<DisplayName>>,
) {
    for (actor_hud, health) in actors_q.iter() {
        if let Ok(mut state) = state_q.get_mut(actor_hud.name) {
            state.set_if_neq(HealthState::from_ratio(health.ratio()));
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
            && let Some(bar) = agent_hud.health_bar
            && let Ok(mut hud_bar) = hud_bars_q.get_mut(bar)
        {
            hud_bar.ratio = health.ratio();
        }
        if let Some(mana) = mana
            && let Some(bar) = agent_hud.mana_bar
            && let Ok(mut hud_bar) = hud_bars_q.get_mut(bar)
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
    use bevy::ecs::system::RunSystemOnce;

    use crate::agent::AgentId;
    use crate::map::ViewportCenter;

    /// Change detection needs the system's `last_run` to survive between runs, and a
    /// `RunSystemOnce` initialises a fresh system every call — under which `Changed<T>`
    /// matches everything and a filter bug cannot fail a test. A `Schedule` keeps it.
    fn a_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(update_hud_bar_ratios);
        schedule
    }

    fn a_hud_agent(world: &mut World) -> (Entity, AgentHud) {
        let hud = AgentHud {
            root: world.spawn(Visibility::Hidden).id(),
            name: world.spawn((DisplayName, HealthState::Full)).id(),
            health_bar: Some(world.spawn(HudBar { ratio: 1.0 }).id()),
            mana_bar: Some(world.spawn(HudBar { ratio: 1.0 }).id()),
        };
        let agent = world
            .spawn((
                hud.clone(),
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
        (agent, hud)
    }

    fn ratio(world: &World, bar: Option<Entity>) -> f32 {
        world.get::<HudBar>(bar.unwrap()).unwrap().ratio
    }

    /// The bug this pins: the system writes both bars but woke only on `Changed<Health>`,
    /// so the mana bar sat at whatever ratio the last hit left it at. Casting and drinking
    /// move the mana and nothing else, which is a state a player stays in indefinitely.
    #[test]
    fn a_mana_only_change_moves_the_mana_bar() {
        let mut world = World::new();
        let (agent, hud) = a_hud_agent(&mut world);
        let mut schedule = a_schedule();

        schedule.run(&mut world);
        world.clear_trackers();

        world.get_mut::<Mana>(agent).unwrap().current = 50;
        schedule.run(&mut world);

        assert_eq!(ratio(&world, hud.mana_bar), 0.5);
        assert_eq!(ratio(&world, hud.health_bar), 1.0, "the life was untouched");
    }

    /// The other half of the `Or`: health alone must still wake it.
    #[test]
    fn a_health_only_change_moves_the_health_bar() {
        let mut world = World::new();
        let (agent, hud) = a_hud_agent(&mut world);
        let mut schedule = a_schedule();

        schedule.run(&mut world);
        world.clear_trackers();

        world.get_mut::<Health>(agent).unwrap().current = 25;
        schedule.run(&mut world);

        assert_eq!(ratio(&world, hud.health_bar), 0.25);
        assert_eq!(ratio(&world, hud.mana_bar), 1.0, "the mana was untouched");
    }

    #[test]
    fn a_health_change_recolours_the_name() {
        let mut world = World::new();
        let (agent, hud) = a_hud_agent(&mut world);
        world.get_mut::<Health>(agent).unwrap().current = 10;

        world
            .run_system_once(update_display_name_health_state)
            .unwrap();

        assert_eq!(
            *world.get::<HealthState>(hud.name).unwrap(),
            HealthState::Lowest
        );
    }

    fn a_world_with_a_player_at_100_100_7(center: Option<Position>) -> World {
        let mut world = World::new();
        world.insert_resource(ViewportCenter(center));
        world.spawn((
            Player {
                agent_id: AgentId(1),
            },
            Position::new(100, 100, 7),
        ));
        world
    }

    #[test]
    fn a_hud_shows_only_on_the_players_floor() {
        for (floor, expected) in [(7, Visibility::Visible), (6, Visibility::Hidden)] {
            let mut world = a_world_with_a_player_at_100_100_7(None);
            let (agent, hud) = a_hud_agent(&mut world);
            world
                .entity_mut(agent)
                .insert((Agent::default(), Position::new(101, 100, floor)));

            world.run_system_once(update_hud_visibility).unwrap();

            assert_eq!(
                *world.get::<Visibility>(hud.root).unwrap(),
                expected,
                "floor {floor}"
            );
        }
    }

    #[test]
    fn a_hud_hides_with_an_agent_outside_the_viewport() {
        let mut world = a_world_with_a_player_at_100_100_7(Some(Position::new(100, 100, 7)));
        let (agent, hud) = a_hud_agent(&mut world);
        world
            .entity_mut(agent)
            .insert((Agent::default(), Position::new(110, 100, 7)));

        world.run_system_once(update_hud_visibility).unwrap();

        assert_eq!(
            *world.get::<Visibility>(hud.root).unwrap(),
            Visibility::Hidden
        );
    }

    #[test]
    fn a_hud_stays_on_screen_while_the_players_floor_change_is_deferred() {
        let mut world = a_world_with_a_player_at_100_100_7(Some(Position::new(100, 100, 8)));
        let (agent, hud) = a_hud_agent(&mut world);
        world
            .entity_mut(agent)
            .insert((Agent::default(), Position::new(101, 100, 7)));

        world.run_system_once(update_hud_visibility).unwrap();

        assert_eq!(
            *world.get::<Visibility>(hud.root).unwrap(),
            Visibility::Visible
        );
    }

    fn a_framed_hud(world: &mut World) -> AgentHud {
        let root = world.spawn(Visibility::Hidden).id();
        let name = world
            .spawn((DisplayName, Visibility::Inherited, ChildOf(root)))
            .id();
        let bar = |world: &mut World| {
            let frame = world.spawn((Visibility::Inherited, ChildOf(root))).id();
            world
                .spawn((HudBar { ratio: 1.0 }, Visibility::Inherited, ChildOf(frame)))
                .id()
        };
        let health_bar = Some(bar(world));
        let mana_bar = Some(bar(world));
        let hud = AgentHud {
            root,
            name,
            health_bar,
            mana_bar,
        };
        world.spawn(hud.clone());
        hud
    }

    fn frame_of(world: &World, fill: Option<Entity>) -> Entity {
        world.get::<ChildOf>(fill.unwrap()).unwrap().parent()
    }

    fn visibility(world: &World, entity: Entity) -> Visibility {
        *world.get::<Visibility>(entity).unwrap()
    }

    fn a_settings_schedule() -> Schedule {
        let mut schedule = Schedule::default();
        schedule.add_systems(apply_hud_settings);
        schedule
    }

    #[test]
    fn names_and_bars_follow_their_own_setting() {
        let mut world = World::new();
        world.init_resource::<Settings>();
        let hud = a_framed_hud(&mut world);
        let mut schedule = a_settings_schedule();
        schedule.run(&mut world);

        world.resource_mut::<Settings>().show_names = false;
        schedule.run(&mut world);

        assert_eq!(visibility(&world, hud.name), Visibility::Hidden);
        assert_eq!(
            visibility(&world, frame_of(&world, hud.health_bar)),
            Visibility::Inherited
        );

        {
            let mut settings = world.resource_mut::<Settings>();
            settings.show_names = true;
            settings.show_health_bars = false;
        }
        schedule.run(&mut world);

        assert_eq!(visibility(&world, hud.name), Visibility::Inherited);
        assert_eq!(
            visibility(&world, frame_of(&world, hud.health_bar)),
            Visibility::Hidden
        );
        assert_eq!(
            visibility(&world, frame_of(&world, hud.mana_bar)),
            Visibility::Hidden
        );
    }

    #[test]
    fn a_hud_spawned_after_the_change_starts_hidden() {
        let mut world = World::new();
        world.init_resource::<Settings>();
        let mut schedule = a_settings_schedule();
        world.resource_mut::<Settings>().show_names = false;
        schedule.run(&mut world);

        let hud = a_framed_hud(&mut world);
        schedule.run(&mut world);

        assert_eq!(visibility(&world, hud.name), Visibility::Hidden);
    }
}
