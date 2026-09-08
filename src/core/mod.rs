use bevy::prelude::*;
use bevy::sprite_render::Material2dPlugin;

mod animation;
mod assets;
mod effects;
mod floating_text;
mod instances;
mod items;
mod missiles;
mod session;
mod spells;
mod sprite;
mod systems;
mod text;

pub use animation::*;
pub use assets::*;
pub use floating_text::FloatingTextType;
pub use instances::*;
pub use items::ItemConfigs;
pub use session::{EndGameSession, SessionCleanup, SessionEndReason, SessionEnding};
pub use spells::{SpellId, SpellTarget};
pub use sprite::*;
pub use systems::PingState;
pub use text::{ChatMessageType, SayTarget, TextMessageType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, States, Default)]
pub enum GameState {
    #[default]
    LoadingAssets,
    LoginScreen,
    Connecting,
    InGame,
}

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<effects::EffectMaterial>::default())
            .init_resource::<systems::PingState>()
            .init_resource::<systems::PingTimer>()
            .init_resource::<InstanceManager<effects::EffectInstance>>()
            .add_systems(
                Startup,
                (assets::start_load_tasks, effects::setup_resources),
            )
            .add_systems(
                FixedUpdate,
                (
                    assets::pool_load_task.run_if(resource_exists::<LoadTasks>),
                    assets::pool_all_assets_loaded.run_if(resource_exists::<GameAssetsLoaded>),
                    systems::send_ping.run_if(in_state(GameState::InGame)),
                ),
            )
            .add_systems(
                Update,
                text::despawn_text_messages.run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    floating_text::tick_hit_points,
                    floating_text::tick_speech_blocks,
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PostUpdate,
                (
                    floating_text::resolve_speech_collisions,
                    floating_text::position_floating_texts,
                )
                    .chain()
                    .before(bevy::ui::UiSystems::Layout)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                tick_sprite_animators
                    .in_set(AnimationSet)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                Update,
                (
                    effects::despawn_finished_effects,
                    effects::update_effect_instances,
                )
                    .after(AnimationSet)
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PostUpdate,
                effects::upload_effect_buffer.run_if(in_state(GameState::InGame)),
            )
            .add_observer(text::on_text_message)
            .add_observer(floating_text::on_floating_text)
            .add_systems(
                Update,
                missiles::fly_missiles.run_if(in_state(GameState::InGame)),
            )
            .add_observer(effects::on_show_effect)
            .add_observer(effects::on_remove_effect)
            .add_observer(missiles::on_launch_missile)
            .add_observer(missiles::on_remove_missile)
            .configure_sets(OnExit(GameState::InGame), SessionCleanup)
            .add_systems(
                OnExit(GameState::InGame),
                (
                    session::cleanup_session,
                    effects::cleanup_session,
                    missiles::cleanup_session,
                )
                    .in_set(SessionCleanup),
            );

        #[cfg(feature = "debug")]
        app.add_systems(
            Update,
            floating_text::debug_spawn_floating_text.run_if(in_state(GameState::InGame)),
        );
    }
}
