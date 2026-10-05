use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;

use crate::core::{Appearances, SpriteSheet};

#[cfg(debug_assertions)]
mod metrics;

pub const IDLE_WINDOW: Duration = Duration::from_secs(5 * 60);

/// The sprite sheet group this entity draws from.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct SheetUser(pub String);

#[derive(Event, Debug, Clone)]
pub struct SheetEvicted {
    pub group: String,
    pub sheet_name: String,
}

#[derive(Resource, Default)]
pub struct SheetEviction {
    last_used: HashMap<String, Duration>,
    #[cfg(debug_assertions)]
    metrics: metrics::SheetMetrics,
}

pub(super) fn sweep_sheets(
    mut commands: Commands,
    mut appearances: ResMut<Appearances>,
    mut eviction: ResMut<SheetEviction>,
    users_q: Query<&SheetUser>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed();
    let users: HashSet<&str> = users_q.iter().map(|user| user.0.as_str()).collect();
    let appearances = appearances.bypass_change_detection();
    let eviction = &mut *eviction;

    let resident: Vec<(&str, &SpriteSheet)> = appearances.resident_sheets().collect();
    observe(
        &mut eviction.last_used,
        resident.iter().map(|(group, _)| *group),
        &users,
        now,
    );
    #[cfg(debug_assertions)]
    eviction.metrics.observe(&resident, &users, now);

    for group in stale_groups(&eviction.last_used, &users, now) {
        eviction.last_used.remove(&group);
        let Some(sheet) = appearances.evict(&group) else {
            continue;
        };
        #[cfg(debug_assertions)]
        eviction.metrics.evicted(&group, sheet, now);
        commands.trigger(SheetEvicted {
            sheet_name: sheet.sheet_name.clone(),
            group,
        });
    }
}

pub(super) fn observe<'a>(
    last_used: &mut HashMap<String, Duration>,
    resident: impl IntoIterator<Item = &'a str>,
    users: &HashSet<&str>,
    now: Duration,
) {
    for group in resident {
        if users.contains(group) {
            last_used.insert(group.to_owned(), now);
        } else {
            last_used.entry(group.to_owned()).or_insert(now);
        }
    }
}

pub(super) fn stale_groups(
    last_used: &HashMap<String, Duration>,
    users: &HashSet<&str>,
    now: Duration,
) -> Vec<String> {
    last_used
        .iter()
        .filter(|(group, used)| {
            !users.contains(group.as_str()) && now.saturating_sub(**used) >= IDLE_WINDOW
        })
        .map(|(group, _)| group.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core::sprite::test_asset_server;
    use crate::core::{Appearances, SpriteConfigs, SpriteSheet};
    use bevy::ecs::system::RunSystemOnce;

    const START: Duration = Duration::from_secs(10);
    const SECOND: Duration = Duration::from_secs(1);

    fn tracked(group: &str, at: Duration) -> HashMap<String, Duration> {
        HashMap::from([(group.to_owned(), at)])
    }

    #[test]
    fn an_unused_sheet_goes_stale_at_the_window_and_not_before() {
        let last_used = tracked("item-a", START);
        let nobody = HashSet::new();

        assert!(stale_groups(&last_used, &nobody, START + IDLE_WINDOW - SECOND).is_empty());
        assert_eq!(
            stale_groups(&last_used, &nobody, START + IDLE_WINDOW),
            vec!["item-a".to_owned()]
        );
    }

    #[test]
    fn a_sheet_with_a_user_is_never_stale() {
        let last_used = tracked("item-a", START);
        let users = HashSet::from(["item-a"]);

        assert!(stale_groups(&last_used, &users, START + IDLE_WINDOW * 10).is_empty());
    }

    #[test]
    fn a_newly_resident_sheet_starts_its_window_when_first_seen() {
        let mut last_used = HashMap::new();
        let nobody = HashSet::new();

        observe(&mut last_used, ["item-a"], &nobody, START);
        observe(&mut last_used, ["item-a"], &nobody, START + SECOND);

        assert_eq!(last_used, tracked("item-a", START));
        assert!(stale_groups(&last_used, &nobody, START + SECOND).is_empty());
    }

    #[test]
    fn a_used_sheet_restarts_its_window() {
        let mut last_used = tracked("item-a", START);

        observe(
            &mut last_used,
            ["item-a"],
            &HashSet::from(["item-a"]),
            START + IDLE_WINDOW,
        );

        assert_eq!(last_used, tracked("item-a", START + IDLE_WINDOW));
    }

    #[derive(Resource, Default)]
    struct Evicted(Vec<(String, String)>);

    #[test]
    fn the_sweep_evicts_only_the_sheet_nobody_draws_from() {
        let sheets = HashMap::from([
            (
                "item-a".to_owned(),
                SpriteSheet::for_test("item-a.png", Vec2::ONE, Vec2::splat(32.0)),
            ),
            (
                "item-b".to_owned(),
                SpriteSheet::for_test("item-b.png", Vec2::ONE, Vec2::splat(32.0)),
            ),
        ]);
        let appearances = Appearances::new(sheets, SpriteConfigs::default(), test_asset_server());
        appearances.get_sheet("item-a");
        appearances.get_sheet("item-b");

        let mut world = World::new();
        world.insert_resource(appearances);
        world.init_resource::<SheetEviction>();
        world.init_resource::<Evicted>();
        world.insert_resource(Time::<Real>::default());
        world.add_observer(|event: On<SheetEvicted>, mut seen: ResMut<Evicted>| {
            seen.0.push((event.group.clone(), event.sheet_name.clone()));
        });
        world.spawn(SheetUser("item-a".to_owned()));

        world.run_system_once(sweep_sheets).unwrap();
        world
            .resource_mut::<Time<Real>>()
            .advance_by(IDLE_WINDOW - SECOND);
        world.run_system_once(sweep_sheets).unwrap();
        assert!(world.resource::<Evicted>().0.is_empty());

        world.resource_mut::<Time<Real>>().advance_by(SECOND);
        world.run_system_once(sweep_sheets).unwrap();

        assert_eq!(
            world.resource::<Evicted>().0,
            vec![("item-b".to_owned(), "item-b.png".to_owned())]
        );
        let resident: Vec<String> = world
            .resource::<Appearances>()
            .resident_sheets()
            .map(|(group, _)| group.to_owned())
            .collect();
        assert_eq!(resident, vec!["item-a".to_owned()]);
    }
}
