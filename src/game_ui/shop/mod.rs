pub(super) mod state;
mod window;

pub use state::{ShopOffer, ShopState};

use bevy::prelude::*;

use crate::core::GameState;

pub struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShopState>()
            .add_observer(state::on_shop_received)
            .add_observer(state::on_shop_closed)
            .add_observer(window::on_close_clicked)
            .add_systems(
                Update,
                (
                    window::open_or_close_window.run_if(resource_changed::<ShopState>),
                    window::read_search,
                    window::refresh_window,
                    window::sync_shop_slider,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame)),
            );
    }
}
