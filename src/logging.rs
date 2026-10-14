use std::fs::{self, File};
use std::sync::Mutex;

use bevy::log::BoxedLayer;
use bevy::log::tracing_subscriber::{Layer, fmt};
use bevy::prelude::*;

use crate::conf::paths::data_dir;

/// Production builds have no console, so their log and panics go to `client.log` in the data
/// dir, keeping the previous run's as `client.old.log`.
pub fn file_layer(_app: &mut App) -> Option<BoxedLayer> {
    if cfg!(debug_assertions) {
        return None;
    }
    let dir = data_dir();
    fs::create_dir_all(&dir).ok()?;
    let path = dir.join("client.log");
    let _ = fs::rename(&path, dir.join("client.old.log"));
    let file = File::create(&path).ok()?;

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        error!("{info}");
        previous_hook(info);
    }));

    Some(
        fmt::layer()
            .with_ansi(false)
            .with_writer(Mutex::new(file))
            .boxed(),
    )
}
