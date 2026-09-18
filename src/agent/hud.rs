use bevy::prelude::*;

use crate::agent::components::{Agent, AgentHud, HealthState};
use crate::agent::{DisplayName, Health, HudBar, Mana};
use crate::map::Position;
use crate::player::components::Player;

pub fn update_hud_visibility(
    player_pos_q: Query<&Position, With<Player>>,
    agents_q: Query<(&AgentHud, &Position), With<Agent>>,
    mut visibility_q: Query<&mut Visibility>,
) {
    let Ok(player_pos) = player_pos_q.single() else {
        return;
    };
    for (hud, position) in &agents_q {
        let wanted = if position.z == player_pos.z {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if let Ok(mut visibility) = visibility_q.get_mut(hud.root) {
            visibility.set_if_neq(wanted);
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

    #[test]
    fn a_hud_shows_only_on_the_players_floor() {
        for (floor, expected) in [(7, Visibility::Visible), (6, Visibility::Hidden)] {
            let mut world = World::new();
            world.spawn((
                Player {
                    agent_id: AgentId(1),
                },
                Position::new(100, 100, 7),
            ));
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
}
