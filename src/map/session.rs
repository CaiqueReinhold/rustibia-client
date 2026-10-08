use bevy::prelude::*;

use crate::map::minimap::{MinimapData, SaveTimer, flush_dirty_chunks};
use crate::map::minimap_ui::{MinimapImageHandle, MinimapWindow, MinimapZoom};
use crate::map::storage::Map;
use crate::map::viewport::ViewportCenter;

/// Resets the map to an empty world and drops the per-session minimap view.
pub(super) fn cleanup_session(mut commands: Commands, mut minimap: ResMut<MinimapData>) {
    // The save timer only ticks while in-game, so anything explored since the last
    // tick would be lost — and the explored map is meant to survive the session.
    flush_dirty_chunks(&mut minimap);

    commands.insert_resource(Map::default());
    commands.insert_resource(ViewportCenter::default());
    commands.insert_resource(SaveTimer::default());
    commands.remove_resource::<MinimapImageHandle>();
    commands.remove_resource::<MinimapZoom>();
    commands.remove_resource::<MinimapWindow>();
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn session_world() -> World {
        let mut world = World::new();
        world.init_resource::<MinimapData>();
        world.init_resource::<Map>();
        world
    }

    /// All three are re-created by `setup_minimap` on the next `OnEnter(InGame)`,
    /// against a fresh texture. Their absence is what "no session" means, and a
    /// `MinimapWindow` that outlived its texture would report a floor as painted
    /// that nothing had painted.
    #[test]
    fn cleanup_drops_the_minimap_view_resources() {
        let mut world = session_world();
        world.insert_resource(MinimapZoom(0));
        world.insert_resource(MinimapWindow::default());

        world.run_system_once(cleanup_session).unwrap();

        assert!(world.get_resource::<MinimapZoom>().is_none());
        assert!(world.get_resource::<MinimapWindow>().is_none());
    }

    #[test]
    fn cleanup_forgets_the_viewport_center() {
        let mut world = session_world();
        world.insert_resource(ViewportCenter(Some(crate::map::Position::new(1, 2, 7))));

        world.run_system_once(cleanup_session).unwrap();

        assert_eq!(world.resource::<ViewportCenter>().0, None);
    }
}
