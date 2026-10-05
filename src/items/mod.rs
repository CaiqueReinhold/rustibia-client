use bevy::{prelude::*, sprite_render::Material2dPlugin};

use crate::{
    core::{AnimationSet, GameState, InstanceManager, SessionCleanup},
    items::{instancing::ItemState, material::ItemInstance},
};

mod container;
mod events;
mod fluid;
mod instancing;
pub mod inventory;
mod item;
mod material;
mod session;
mod split_dialog;
mod ui_item;

pub use container::{ContainerId, LootContainerUI};
pub use events::*;
pub use fluid::fluid_cell;
pub use instancing::ChangedTileQueue;
pub use item::{InventorySlot, Item, ItemConfig, ItemFlag, ItemId, ItemPlacement};

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            Material2dPlugin::<material::ItemMaterial>::default(),
            // The crate ships every widget as one `UiWidgetsPlugins` bundle;
            // this codebase adds the one plugin each widget it uses needs, the
            // way `GameUiPlugin` adds `ScrollbarPlugin`.
            bevy::ui_widgets::SliderPlugin,
        ))
        .init_resource::<InstanceManager<ItemInstance>>()
        .init_resource::<ChangedTileQueue>()
        .init_resource::<ItemState>()
        .add_systems(Startup, instancing::setup_resources)
        .add_systems(
            OnEnter(GameState::InGame),
            inventory::spawn_inventory_ui.after(crate::game_ui::spawn_main_ui),
        )
        .add_systems(
            Update,
            (
                instancing::process_tile_changed.after(crate::map::evict_outside_viewport),
                ui_item::move_dragged_item,
                split_dialog::sync_split_slider,
                container::container_content_changed,
                inventory::update_inventory_ui,
                inventory::update_capacity,
            )
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            Update,
            ui_item::animate_ui_items
                .after(AnimationSet)
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            Update,
            instancing::update_item_instances
                .after(AnimationSet)
                .run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            PostUpdate,
            instancing::upload_instance_buffer.run_if(in_state(GameState::InGame)),
        )
        .add_systems(
            OnExit(GameState::InGame),
            session::cleanup_session.in_set(SessionCleanup),
        )
        .add_observer(instancing::on_remove_item)
        .add_observer(instancing::on_sheet_evicted)
        .add_observer(split_dialog::on_open_split_dialog)
        .add_observer(split_dialog::on_split_dialog_button)
        .add_observer(ui_item::item_drag_started)
        .add_observer(ui_item::item_drag_ended)
        .add_observer(container::on_open_container)
        .add_observer(container::on_update_container)
        .add_observer(container::on_container_closed_by_server)
        .add_observer(container::on_container_ui_closed)
        .add_observer(container::on_open_parent_container);
    }
}
