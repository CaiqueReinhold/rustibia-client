use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSmoothing, TextBounds};
use bevy_text_outline::TextOutline;

use crate::conf::ui::ui_colors;
use crate::conf::viewport::{GAME_VIEW_HEIGHT, GAME_VIEW_WIDTH};
use crate::game_ui::{GameUiAssets, GameViewport};
use crate::network::events::ShowTextMessage;
use crate::overlay::{ViewportTextRoot, on_overlay_layer, world_hud_scale};

const ACTION_DENIED_ABOVE_BOTTOM: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMessageType {
    ActionDenied,
    Look,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatMessageType {
    Local,
    Private,
    Channel,
}

/// Who an outbound `Say` is addressed to. One discriminant rather than a message type
/// plus an overloaded target field, so a channel id and a recipient cannot be confused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SayTarget {
    Local,
    Channel(u16),
    Player(String),
}

impl SayTarget {
    pub fn message_type(&self) -> ChatMessageType {
        match self {
            SayTarget::Local => ChatMessageType::Local,
            SayTarget::Channel(_) => ChatMessageType::Channel,
            SayTarget::Player(_) => ChatMessageType::Private,
        }
    }
}

#[derive(Component, Debug)]
pub struct TextMessage {
    timer: Timer,
    message_type: TextMessageType,
}

/// The top of a message's first line, in logical pixels from the view's centre, y up.
fn message_top(message_type: TextMessageType, scale: f32) -> f32 {
    match message_type {
        TextMessageType::ActionDenied => {
            -GAME_VIEW_HEIGHT / scale / 2.0 + ACTION_DENIED_ABOVE_BOTTOM
        }
        TextMessageType::Look => 0.0,
    }
}

fn message_width(scale: f32) -> f32 {
    GAME_VIEW_WIDTH / scale
}

pub fn on_text_message(
    event: On<ShowTextMessage>,
    mut commands: Commands,
    root_q: Single<Entity, With<ViewportTextRoot>>,
    ui_assets: Res<GameUiAssets>,
    message_q: Query<(Entity, &TextMessage)>,
) {
    let timer = match event.message_type {
        TextMessageType::ActionDenied => Timer::from_seconds(2.0, TimerMode::Once),
        TextMessageType::Look => Timer::from_seconds(5.0, TimerMode::Once),
    };
    let color = match event.message_type {
        TextMessageType::ActionDenied => Color::WHITE,
        TextMessageType::Look => ui_colors::FONT_COLOR_LOOK_MSG.into(),
    };

    for (entity, msg) in message_q {
        if msg.message_type == event.message_type {
            commands.entity(entity).despawn();
        }
    }

    commands.spawn((
        TextMessage {
            timer,
            message_type: event.message_type,
        },
        Text2d::new(event.text.clone()),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: 11.0,
            ..default()
        }
        .with_font_smoothing(FontSmoothing::AntiAliased),
        TextColor(color),
        TextLayout::new_with_justify(Justify::Center),
        TextBounds::default(),
        TextOutline::default(),
        Anchor::TOP_CENTER,
        on_overlay_layer(),
        ChildOf(*root_q),
    ));
}

/// Keeps every message at its place in the view and wrapped to its width, which
/// both move in logical pixels when the view is resized.
pub fn place_text_messages(
    viewport_q: Query<&ComputedNode, With<GameViewport>>,
    mut message_q: Query<(&TextMessage, &mut Transform, &mut TextBounds)>,
) {
    let Some(scale) = viewport_q.single().ok().and_then(world_hud_scale) else {
        return;
    };
    for (message, mut transform, mut bounds) in &mut message_q {
        let top = message_top(message.message_type, scale);
        if transform.translation.y != top {
            transform.translation.y = top;
        }
        let width = Some(message_width(scale));
        if bounds.width != width {
            bounds.width = width;
        }
    }
}

pub fn despawn_text_messages(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut TextMessage)>,
) {
    for (entity, mut text_message) in q.iter_mut() {
        text_message.timer.tick(time.delta());
        if text_message.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

pub fn cleanup_session(mut commands: Commands, messages: Query<Entity, With<TextMessage>>) {
    for entity in &messages {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    use crate::overlay::a_viewport;

    #[test]
    fn a_denial_sits_the_same_distance_above_the_bottom_at_every_size() {
        for scale in [1.0, 0.5] {
            let bottom = -GAME_VIEW_HEIGHT / scale / 2.0;
            assert_eq!(
                message_top(TextMessageType::ActionDenied, scale) - bottom,
                ACTION_DENIED_ABOVE_BOTTOM,
                "scale {scale}"
            );
        }
    }

    #[test]
    fn a_look_message_starts_at_the_centre() {
        assert_eq!(message_top(TextMessageType::Look, 0.5), 0.0);
    }

    #[test]
    fn a_message_wraps_to_the_view_width() {
        assert_eq!(message_width(1.0), GAME_VIEW_WIDTH);
        assert_eq!(message_width(0.5), GAME_VIEW_WIDTH * 2.0);
    }

    fn message_world() -> World {
        let mut world = World::new();
        world.insert_resource(GameUiAssets {
            font: Handle::default(),
            window: Default::default(),
            inventory: Default::default(),
            background_dark: Handle::default(),
            background_light: Handle::default(),
            bar_overlay: Handle::default(),
            title_background: Handle::default(),
        });
        world.spawn(ViewportTextRoot);
        world.spawn((
            GameViewport,
            a_viewport(
                Vec2::new(GAME_VIEW_WIDTH * 2.0, GAME_VIEW_HEIGHT * 2.0),
                1.0,
            ),
        ));
        world.add_observer(on_text_message);
        world
    }

    fn show(world: &mut World, message_type: TextMessageType) {
        world.trigger(ShowTextMessage {
            text: "You see a rat.".to_owned(),
            message_type,
        });
        world.flush();
    }

    #[test]
    fn a_message_is_placed_under_the_viewport_root() {
        let mut world = message_world();
        show(&mut world, TextMessageType::ActionDenied);

        world.run_system_once(place_text_messages).unwrap();

        let root = world
            .query_filtered::<Entity, With<ViewportTextRoot>>()
            .single(&world)
            .unwrap();
        let (child_of, transform, bounds) = world
            .query_filtered::<(&ChildOf, &Transform, &TextBounds), With<TextMessage>>()
            .single(&world)
            .unwrap();
        assert_eq!(child_of.parent(), root);
        assert_eq!(
            transform.translation.y,
            message_top(TextMessageType::ActionDenied, 0.5)
        );
        assert_eq!(bounds.width, Some(GAME_VIEW_WIDTH * 2.0));
    }

    #[test]
    fn a_new_message_replaces_one_of_its_type() {
        let mut world = message_world();
        show(&mut world, TextMessageType::Look);
        show(&mut world, TextMessageType::Look);
        show(&mut world, TextMessageType::ActionDenied);

        let mut kinds: Vec<_> = world
            .query::<&TextMessage>()
            .iter(&world)
            .map(|m| m.message_type)
            .collect();
        kinds.sort_by_key(|k| *k as u8);
        assert_eq!(
            kinds,
            [TextMessageType::ActionDenied, TextMessageType::Look]
        );
    }

    #[test]
    fn leaving_the_game_clears_every_message() {
        let mut world = message_world();
        show(&mut world, TextMessageType::Look);

        world.run_system_once(cleanup_session).unwrap();

        assert_eq!(world.query::<&TextMessage>().iter(&world).count(), 0);
    }
}
