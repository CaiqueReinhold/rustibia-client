use std::f32::consts::TAU;
use std::time::Duration;

use bevy::prelude::*;
use bevy::text::FontSmoothing;
use bevy_text_outline::TextOutline;

use crate::conf::ui::{cooldown as conf, ui_colors};
use crate::core::{CooldownState, SpellCooldowns, SpellGroup, SpellId};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CooldownOverlay {
    Spell { id: SpellId, group: SpellGroup },
    Group(SpellGroup),
}

impl CooldownOverlay {
    fn state(self, cooldowns: &SpellCooldowns, now: Duration) -> Option<CooldownState> {
        match self {
            CooldownOverlay::Spell { id, group } => cooldowns.spell(id, group, now),
            CooldownOverlay::Group(group) => cooldowns.group(group, now),
        }
    }
}

#[derive(Component, Clone, Copy, Debug)]
pub(super) struct CooldownTimer(CooldownOverlay);

#[derive(Resource)]
pub(crate) struct CooldownAssets {
    groups: Handle<Image>,
    group_layout: Handle<TextureAtlasLayout>,
}

pub(super) fn setup_cooldown_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let group_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(conf::GROUP_SHEET_CELL),
        conf::GROUP_SHEET_COLUMNS,
        conf::GROUP_SHEET_ROWS,
        None,
        None,
    ));
    commands.insert_resource(CooldownAssets {
        groups: asset_server.load("ui/cooldowns.png"),
        group_layout,
    });
}

pub(super) fn group_icon(assets: &CooldownAssets, group: SpellGroup) -> ImageNode {
    ImageNode::from_atlas_image(
        assets.groups.clone(),
        TextureAtlas {
            layout: assets.group_layout.clone(),
            index: group_icon_index(group),
        },
    )
}

/// The sheet's second row holds the coloured icons.
fn group_icon_index(group: SpellGroup) -> usize {
    conf::GROUP_SHEET_COLUMNS as usize + group.index()
}

pub(super) fn spawn_cooldown_overlay(
    commands: &mut Commands,
    tracks: CooldownOverlay,
    timer_font: Option<&Handle<Font>>,
) -> Entity {
    let overlay = commands
        .spawn((
            tracks,
            BackgroundGradient::default(),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    if let Some(font) = timer_font {
        let timer = commands
            .spawn((
                CooldownTimer(tracks),
                Text::default(),
                TextFont {
                    font: font.clone(),
                    font_size: conf::FONT_SIZE,
                    ..default()
                }
                .with_font_smoothing(FontSmoothing::None),
                TextColor(Color::WHITE),
                TextOutline {
                    width: 1.0,
                    ..default()
                },
                Pickable::IGNORE,
            ))
            .id();
        commands.entity(overlay).add_child(timer);
    }
    overlay
}

/// Rounded up to a tenth of a second.
fn timer_label(remaining: Duration) -> String {
    let tenths = remaining.as_nanos().div_ceil(100_000_000);
    format!("{}.{}", tenths / 10, tenths % 10)
}

fn shade_gradient(recovered: f32) -> BackgroundGradient {
    let edge = recovered * TAU;
    let shade = Color::from(ui_colors::COOLDOWN_SHADE);
    ConicGradient::new(
        UiPosition::CENTER,
        vec![
            AngularColorStop::new(Color::NONE, 0.0),
            AngularColorStop::new(Color::NONE, edge),
            AngularColorStop::new(shade, edge),
            AngularColorStop::new(shade, TAU),
        ],
    )
    .into()
}

pub(super) fn update_cooldown_overlays(
    time: Res<Time<Real>>,
    cooldowns: Res<SpellCooldowns>,
    mut overlays: Query<(&CooldownOverlay, &mut BackgroundGradient)>,
    mut timers: Query<(&CooldownTimer, &mut Text)>,
) {
    let now = time.elapsed();
    for (tracks, mut gradient) in &mut overlays {
        let shade = tracks
            .state(&cooldowns, now)
            .map_or_else(BackgroundGradient::default, |state| {
                shade_gradient(state.recovered)
            });
        gradient.set_if_neq(shade);
    }
    for (timer, mut text) in &mut timers {
        let label = timer
            .0
            .state(&cooldowns, now)
            .map_or_else(String::new, |state| timer_label(state.remaining));
        text.set_if_neq(Text(label));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn the_timer_rounds_up_to_a_tenth() {
        assert_eq!(timer_label(ms(1940)), "2.0");
        assert_eq!(timer_label(ms(2000)), "2.0");
        assert_eq!(timer_label(ms(100)), "0.1");
        assert_eq!(timer_label(ms(10)), "0.1");
        assert_eq!(timer_label(ms(12_050)), "12.1");
        assert_eq!(timer_label(Duration::from_micros(400)), "0.1");
        assert_eq!(timer_label(Duration::from_micros(1_000_400)), "1.1");
    }

    #[test]
    fn the_shade_starts_where_recovery_has_reached() {
        let BackgroundGradient(gradients) = shade_gradient(0.25);
        let [Gradient::Conic(conic)] = gradients.as_slice() else {
            panic!("expected one conic gradient, got {gradients:?}");
        };

        let angles: Vec<_> = conic.stops.iter().map(|stop| stop.angle).collect();
        assert_eq!(
            angles,
            [Some(0.0), Some(TAU / 4.0), Some(TAU / 4.0), Some(TAU)]
        );
        assert_eq!(conic.stops[1].color, Color::NONE);
        assert_eq!(conic.stops[2].color, Color::from(ui_colors::COOLDOWN_SHADE));
        assert_eq!(conic.start, 0.0);
    }

    #[test]
    fn a_cooling_overlay_is_shaded_and_timed_then_cleared_when_ready() {
        let mut world = World::new();
        world.init_resource::<Time<Real>>();
        let mut cooldowns = SpellCooldowns::default();
        cooldowns.start_group(SpellGroup::Attack, Duration::ZERO, ms(2000));
        world.insert_resource(cooldowns);

        let tracks = CooldownOverlay::Spell {
            id: SpellId(2),
            group: SpellGroup::Attack,
        };
        let overlay = world.spawn((tracks, BackgroundGradient::default())).id();
        let timer = world.spawn((CooldownTimer(tracks), Text::default())).id();

        world.resource_mut::<Time<Real>>().advance_by(ms(500));
        world.run_system_once(update_cooldown_overlays).unwrap();
        assert_eq!(
            world.get::<BackgroundGradient>(overlay),
            Some(&shade_gradient(0.25))
        );
        assert_eq!(world.get::<Text>(timer).unwrap().0, "1.5");

        world.resource_mut::<Time<Real>>().advance_by(ms(1500));
        world.run_system_once(update_cooldown_overlays).unwrap();
        assert_eq!(
            world.get::<BackgroundGradient>(overlay),
            Some(&BackgroundGradient::default())
        );
        assert_eq!(world.get::<Text>(timer).unwrap().0, "");
    }

    #[test]
    fn a_group_icon_is_its_cell_in_the_coloured_row() {
        assert_eq!(group_icon_index(SpellGroup::Attack), 4);
        assert_eq!(group_icon_index(SpellGroup::Healing), 5);
        assert_eq!(group_icon_index(SpellGroup::Support), 6);
    }

    #[test]
    fn only_an_overlay_given_a_font_gets_a_timer() {
        let spell = CooldownOverlay::Spell {
            id: SpellId(2),
            group: SpellGroup::Attack,
        };
        let mut world = World::new();
        world
            .run_system_once(move |mut commands: Commands| {
                spawn_cooldown_overlay(
                    &mut commands,
                    CooldownOverlay::Group(SpellGroup::Healing),
                    None,
                );
                spawn_cooldown_overlay(&mut commands, spell, Some(&Handle::default()));
            })
            .unwrap();

        let overlays = world.query::<&CooldownOverlay>().iter(&world).count();
        let timers: Vec<_> = world
            .query::<&CooldownTimer>()
            .iter(&world)
            .map(|timer| timer.0)
            .collect();
        assert_eq!(overlays, 2);
        assert_eq!(timers, [spell]);

        let (_, child_of) = world
            .query::<(&CooldownTimer, &ChildOf)>()
            .single(&world)
            .unwrap();
        assert_eq!(
            world.get::<CooldownOverlay>(child_of.parent()),
            Some(&spell)
        );
    }
}
