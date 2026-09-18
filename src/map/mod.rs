use bevy::prelude::*;

use crate::core::{GameState, SessionCleanup};

mod draw_order;
pub mod events;
mod floors;
pub mod minimap;
pub mod minimap_ui;
mod position;
mod session;
mod storage;
mod tile_agents;

pub use crate::map::position::Position;
pub use crate::map::storage::Map;
pub use draw_order::{
    DRAW_KEY_MAX, DrawLayer, DrawOrder, DrawOrigin, DrawRank, TARGET_SQUARE_LOCAL_Z, drawn_tile,
};
pub use floors::FloorEntities;
pub use minimap::MinimapData;
pub use tile_agents::sync_tile_agents;

/// Everything that turns a `DrawOrder` into a z. Systems that write a
/// `DrawOrder` belong before it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DrawOrderSet;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app
            // .add_plugins(Material2dPlugin::<material::TerrainMaterial>::default())
            // .init_resource::<chunks::LoadedChunks>()
            // .init_resource::<chunks::LoadedMaterials>()
            .init_resource::<storage::Map>()
            .init_resource::<DrawOrigin>()
            .init_resource::<minimap::MinimapData>()
            .init_resource::<minimap::SaveTimer>()
            .add_plugins(minimap_ui::MinimapPlugin)
            .add_observer(events::on_describe_map)
            .add_observer(events::on_tile_changed)
            .add_systems(Startup, (minimap::load_from_disk, floors::setup_floors))
            .add_systems(
                PreUpdate,
                sync_tile_agents.run_if(in_state(GameState::InGame)),
            )
            .add_systems(PostUpdate, floors::update_floors_visibility)
            // Before propagation, so the frame's spawns and steps reach the
            // render world already ordered.
            .add_systems(
                PostUpdate,
                (draw_order::update_draw_origin, draw_order::apply_draw_order)
                    .chain()
                    .in_set(DrawOrderSet)
                    .before(TransformSystems::Propagate),
            )
            .add_systems(
                FixedUpdate,
                minimap::save_dirty_chunks.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                OnExit(GameState::InGame),
                session::cleanup_session.in_set(SessionCleanup),
            );
    }
}
