use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};

use super::state::Settings;

pub(super) fn apply_present_mode(
    settings: Res<Settings>,
    mut windows_q: Query<&mut Window, With<PrimaryWindow>>,
) {
    let wanted = if settings.vsync {
        PresentMode::AutoVsync
    } else {
        PresentMode::AutoNoVsync
    };
    for mut window in &mut windows_q {
        if window.present_mode != wanted {
            window.present_mode = wanted;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn vsync_sets_the_primary_windows_present_mode() {
        for (vsync, expected) in [
            (true, PresentMode::AutoVsync),
            (false, PresentMode::AutoNoVsync),
        ] {
            let mut world = World::new();
            world.insert_resource(Settings { vsync, ..default() });
            let window = world
                .spawn((
                    Window {
                        present_mode: PresentMode::Fifo,
                        ..default()
                    },
                    PrimaryWindow,
                ))
                .id();

            world.run_system_once(apply_present_mode).unwrap();

            assert_eq!(
                world.get::<Window>(window).unwrap().present_mode,
                expected,
                "vsync {vsync}"
            );
        }
    }
}
