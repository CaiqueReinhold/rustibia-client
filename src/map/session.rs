use bevy::prelude::*;

use crate::conf::map::{MAX_FLOOR, MIN_FLOOR};
use crate::map::floors::FloorEntities;
use crate::map::minimap::{MinimapData, SaveTimer, flush_dirty_chunks};
use crate::map::minimap_ui::{MinimapImageHandle, MinimapWindow, MinimapZoom};
use crate::map::storage::Map;

/// Resets the map to an empty world and drops the per-session minimap view.
///
/// The floor entities themselves are spawned at `Startup` and outlive every
/// session, so they are not despawned — their item children are despawned by
/// `items::session::cleanup_session`, and all this does is undo the visibility the
/// occlusion system left behind.
pub(super) fn cleanup_session(
    mut commands: Commands,
    mut minimap: ResMut<MinimapData>,
    floors: Res<FloorEntities>,
) {
    // The save timer only ticks while in-game, so anything explored since the last
    // tick would be lost — and the explored map is meant to survive the session.
    flush_dirty_chunks(&mut minimap);

    commands.insert_resource(Map::default());
    commands.insert_resource(SaveTimer::default());
    commands.remove_resource::<MinimapImageHandle>();
    commands.remove_resource::<MinimapZoom>();
    commands.remove_resource::<MinimapWindow>();

    for floor in MIN_FLOOR..=MAX_FLOOR {
        commands
            .entity(floors.floors[floor as usize])
            .insert(Visibility::Visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn world_with_floors() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<MinimapData>();
        world.init_resource::<Map>();
        let floors: Vec<Entity> = (MIN_FLOOR..=MAX_FLOOR)
            .map(|_| world.spawn(Visibility::Hidden).id())
            .collect();
        let first = floors[0];
        world.insert_resource(FloorEntities {
            floors: floors.try_into().unwrap(),
        });
        (world, first)
    }

    /// The floors are spawned once at startup and outlive every session, so the
    /// per-session thing to undo is the visibility the floor-occlusion system left
    /// on them — otherwise the next session starts with floors hidden until the
    /// player moves.
    #[test]
    fn cleanup_makes_every_floor_visible_again() {
        let (mut world, floor) = world_with_floors();

        world.run_system_once(cleanup_session).unwrap();

        assert_eq!(world.get::<Visibility>(floor), Some(&Visibility::Visible));
    }

    /// All three are re-created by `setup_minimap` on the next `OnEnter(InGame)`,
    /// against a fresh texture. Their absence is what "no session" means, and a
    /// `MinimapWindow` that outlived its texture would report a floor as painted
    /// that nothing had painted.
    #[test]
    fn cleanup_drops_the_minimap_view_resources() {
        let (mut world, _) = world_with_floors();
        world.insert_resource(MinimapZoom(0));
        world.insert_resource(MinimapWindow::default());

        world.run_system_once(cleanup_session).unwrap();

        assert!(world.get_resource::<MinimapZoom>().is_none());
        assert!(world.get_resource::<MinimapWindow>().is_none());
    }
}
