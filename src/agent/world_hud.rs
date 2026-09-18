use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::{FontSmoothing, LineHeight};
use bevy_text_outline::TextOutline;

use crate::agent::{AgentHud, DisplayName, Health, HealthState, HudBar, Mana, WorldHud};
use crate::conf::agent::{HUD_BAR_HEIGHT, HUD_BAR_WIDTH};
use crate::conf::ui::ui_colors;
use crate::overlay::{HudScaled, on_overlay_layer};

pub const NAME_FONT_SIZE: f32 = 11.0;
pub const NAME_LINE_HEIGHT: f32 = 13.0;
const NAME_TO_BARS_GAP: f32 = 2.0;
const BAR_BORDER: f32 = 1.0;

/// Offsets from the HUD root, in logical pixels, y up.
#[derive(Debug, PartialEq)]
pub struct WorldHudLayout {
    pub name_bottom: f32,
    pub health_bar: Option<f32>,
    pub mana_bar: Option<f32>,
}

pub fn world_hud_layout(has_health: bool, has_mana: bool) -> WorldHudLayout {
    let bars = has_health as u8 + has_mana as u8;
    let bars_height = if bars == 0 {
        0.0
    } else {
        NAME_TO_BARS_GAP + bars as f32 * HUD_BAR_HEIGHT
    };
    let top = (NAME_LINE_HEIGHT + bars_height) / 2.0;
    let name_bottom = top - NAME_LINE_HEIGHT;

    let mut next_bar = name_bottom - NAME_TO_BARS_GAP - HUD_BAR_HEIGHT / 2.0;
    let mut place = |present: bool| {
        present.then(|| {
            let y = next_bar;
            next_bar -= HUD_BAR_HEIGHT;
            y
        })
    };
    WorldHudLayout {
        name_bottom,
        health_bar: place(has_health),
        mana_bar: place(has_mana),
    }
}

pub fn world_fill_size(ratio: f32) -> Vec2 {
    Vec2::new(
        ratio * (HUD_BAR_WIDTH - 2.0 * BAR_BORDER),
        HUD_BAR_HEIGHT - 2.0 * BAR_BORDER,
    )
}

pub fn spawn_world_hud(
    commands: &mut Commands,
    agent: Entity,
    font: &Handle<Font>,
    name: &str,
    health: Option<&Health>,
    mana: Option<&Mana>,
    world_y_offset: f32,
) -> AgentHud {
    let layout = world_hud_layout(health.is_some(), mana.is_some());
    let root = commands
        .spawn((
            WorldHud,
            HudScaled,
            Transform::from_xyz(0.0, world_y_offset, 0.0),
            Visibility::Hidden,
            on_overlay_layer(),
            ChildOf(agent),
        ))
        .id();

    let state = health.map_or(HealthState::Full, |h| HealthState::from_ratio(h.ratio()));
    let name = commands
        .spawn((
            DisplayName,
            state,
            Text2d::new(name),
            TextFont {
                font: font.clone(),
                font_size: NAME_FONT_SIZE,
                ..default()
            }
            .with_font_smoothing(FontSmoothing::AntiAliased),
            LineHeight::Px(NAME_LINE_HEIGHT),
            TextColor(state.color()),
            TextOutline::default(),
            Anchor::BOTTOM_CENTER,
            Transform::from_xyz(0.0, layout.name_bottom, 2.0),
            on_overlay_layer(),
            ChildOf(root),
        ))
        .id();

    let health_bar = health.zip(layout.health_bar).map(|(health, y)| {
        let state = HealthState::from_ratio(health.ratio());
        spawn_bar(commands, root, y, health.ratio(), state.color(), state)
    });
    let mana_bar = mana.zip(layout.mana_bar).map(|(mana, y)| {
        spawn_bar(
            commands,
            root,
            y,
            mana.ratio(),
            ui_colors::MANA_BAR_COLOR.into(),
            (),
        )
    });

    AgentHud {
        root,
        name,
        health_bar,
        mana_bar,
    }
}

fn spawn_bar(
    commands: &mut Commands,
    root: Entity,
    y: f32,
    ratio: f32,
    color: Color,
    extra: impl Bundle,
) -> Entity {
    let frame = commands
        .spawn((
            Sprite::from_color(Color::BLACK, Vec2::new(HUD_BAR_WIDTH, HUD_BAR_HEIGHT)),
            Transform::from_xyz(0.0, y, 1.0),
            on_overlay_layer(),
            ChildOf(root),
        ))
        .id();
    commands
        .spawn((
            HudBar { ratio },
            Sprite::from_color(color, world_fill_size(ratio)),
            Anchor::CENTER_LEFT,
            Transform::from_xyz(-HUD_BAR_WIDTH / 2.0 + BAR_BORDER, 0.0, 1.0),
            on_overlay_layer(),
            ChildOf(frame),
            extra,
        ))
        .id()
}

pub fn resize_world_hud_fill(mut fills_q: Query<(&mut Sprite, &HudBar), Changed<HudBar>>) {
    for (mut sprite, bar) in &mut fills_q {
        sprite.custom_size = Some(world_fill_size(bar.ratio));
    }
}

pub fn update_world_hud_bar_colors(
    mut fills_q: Query<(&HealthState, &mut Sprite), (With<HudBar>, Changed<HealthState>)>,
) {
    for (state, mut sprite) in &mut fills_q {
        sprite.color = state.color();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::visibility::RenderLayers;

    use crate::camera::HUD_RENDER_LAYER;

    fn column_centre(layout: &WorldHudLayout) -> f32 {
        let top = layout.name_bottom + NAME_LINE_HEIGHT;
        let lowest_bar = layout.mana_bar.or(layout.health_bar);
        let bottom = lowest_bar.map_or(layout.name_bottom, |y| y - HUD_BAR_HEIGHT / 2.0);
        (top + bottom) / 2.0
    }

    #[test]
    fn a_one_bar_and_a_two_bar_column_centre_on_the_root() {
        assert_eq!(column_centre(&world_hud_layout(true, false)), 0.0);
        assert_eq!(column_centre(&world_hud_layout(true, true)), 0.0);
    }

    #[test]
    fn a_name_without_bars_centres_on_the_root() {
        assert_eq!(column_centre(&world_hud_layout(false, false)), 0.0);
    }

    #[test]
    fn the_name_sits_the_gap_above_the_top_bar() {
        let layout = world_hud_layout(true, true);
        let top_of_bar = layout.health_bar.unwrap() + HUD_BAR_HEIGHT / 2.0;
        assert_eq!(layout.name_bottom - top_of_bar, NAME_TO_BARS_GAP);
    }

    #[test]
    fn the_mana_bar_sits_directly_under_the_health_bar() {
        let layout = world_hud_layout(true, true);
        assert_eq!(
            layout.health_bar.unwrap() - layout.mana_bar.unwrap(),
            HUD_BAR_HEIGHT
        );
    }

    #[test]
    fn a_half_fill_is_half_the_inner_width() {
        let full = world_fill_size(1.0);
        let half = world_fill_size(0.5);
        assert_eq!(full, Vec2::new(HUD_BAR_WIDTH - 2.0, HUD_BAR_HEIGHT - 2.0));
        assert_eq!(half.x, full.x / 2.0);
        assert_eq!(half.y, full.y);
    }

    #[test]
    fn a_world_fill_shrinks_with_its_ratio() {
        let mut world = World::new();
        let fill = world
            .spawn((
                HudBar { ratio: 0.5 },
                Sprite::from_color(Color::WHITE, Vec2::ZERO),
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(resize_world_hud_fill);

        schedule.run(&mut world);

        assert_eq!(
            world.get::<Sprite>(fill).unwrap().custom_size,
            Some(world_fill_size(0.5))
        );
    }

    #[test]
    fn a_world_fill_takes_its_health_colour() {
        let mut world = World::new();
        let fill = world
            .spawn((
                HudBar { ratio: 0.1 },
                HealthState::Lowest,
                Sprite::from_color(Color::WHITE, Vec2::ZERO),
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_world_hud_bar_colors);

        schedule.run(&mut world);

        assert_eq!(
            world.get::<Sprite>(fill).unwrap().color,
            HealthState::Lowest.color()
        );
    }

    #[test]
    fn every_part_of_a_world_hud_is_drawn_by_the_hud_camera() {
        let mut world = World::new();
        let agent = world.spawn_empty().id();
        let mut commands = world.commands();
        spawn_world_hud(
            &mut commands,
            agent,
            &Handle::default(),
            "Rat",
            Some(&Health {
                current: 5,
                max: 10,
            }),
            Some(&Mana { current: 1, max: 2 }),
            30.0,
        );
        world.flush();

        let parts: Vec<Entity> = world
            .query_filtered::<Entity, Or<(With<WorldHud>, With<Text2d>, With<Sprite>)>>()
            .iter(&world)
            .collect();
        assert_eq!(
            parts.len(),
            1 + 1 + 4,
            "root, name, two frames and two fills"
        );
        for part in parts {
            assert_eq!(
                world.get::<RenderLayers>(part),
                Some(&RenderLayers::layer(HUD_RENDER_LAYER))
            );
            assert_eq!(world.get::<Pickable>(part), Some(&Pickable::IGNORE));
        }
    }
}
