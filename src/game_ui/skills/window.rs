use bevy::prelude::*;

use crate::agent::HudBar;
use crate::conf::ui::{skills as conf, ui_colors};
use crate::game_ui::GameUiAssets;
use crate::game_ui::skills::{DISPLAY_ORDER, SkillProgress, SkillType, SkillsState};

/// Sits on the window's content entity, which is where `AddUIWindow` puts the
/// `UiWindowRef` the toggle looks for.
#[derive(Component)]
pub struct SkillsWindow;

#[derive(Component)]
pub struct SkillRow {
    pub skill: SkillType,
    pub value_text: Entity,
    pub bar: Entity,
}

#[derive(Component)]
pub struct ExperienceRow {
    pub value_text: Entity,
}

pub fn spawn_skills_content(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    state: &SkillsState,
) -> Entity {
    let content = commands
        .spawn((
            SkillsWindow,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(conf::ROW_GAP),
                padding: UiRect::all(Val::Px(conf::PADDING)),
                ..default()
            },
            ImageNode {
                image: ui_assets.background_dark.clone(),
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 1.0,
                },
                ..default()
            },
        ))
        .id();

    let experience = spawn_experience_row(commands, ui_assets, state.experience);
    commands.entity(content).add_child(experience);

    for skill in DISPLAY_ORDER {
        let Some(progress) = state.skills.get(&skill) else {
            continue;
        };
        let row = spawn_skill_row(commands, ui_assets, skill, *progress);
        commands.entity(content).add_child(row);
    }

    content
}

fn spawn_label(commands: &mut Commands, ui_assets: &GameUiAssets, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: conf::FONT_SIZE,
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .id()
}

fn spawn_experience_row(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    experience: u64,
) -> Entity {
    let label = spawn_label(commands, ui_assets, "Experience");
    let value_text = spawn_label(commands, ui_assets, &group_thousands(experience));

    commands
        .spawn((
            ExperienceRow { value_text },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
        ))
        .add_child(label)
        .add_child(value_text)
        .id()
}

fn spawn_skill_row(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    skill: SkillType,
    progress: SkillProgress,
) -> Entity {
    let label = spawn_label(commands, ui_assets, skill.label());
    let value_text = spawn_label(commands, ui_assets, &progress.level.to_string());

    let heading = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .add_child(label)
        .add_child(value_text)
        .id();

    let bar = commands
        .spawn((
            HudBar {
                ratio: progress.ratio(),
            },
            Node {
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode {
                image: ui_assets.bar_overlay.clone(),
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: false,
                    stretch_value: 1.0,
                },
                ..default()
            },
            BackgroundColor(ui_colors::MANA_BAR_COLOR.into()),
        ))
        .id();

    let bar_frame = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(conf::BAR_HEIGHT),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
        ))
        .add_child(bar)
        .id();

    commands
        .spawn((
            SkillRow {
                skill,
                value_text,
                bar,
            },
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .add_child(heading)
        .add_child(bar_frame)
        .id()
}

pub fn update_skills_window(
    state: Res<SkillsState>,
    skill_rows: Query<&SkillRow>,
    experience_rows: Query<&ExperienceRow>,
    mut bar_q: Query<&mut HudBar>,
    mut text_q: Query<&mut Text>,
) {
    for row in &skill_rows {
        let Some(progress) = state.skills.get(&row.skill) else {
            continue;
        };
        if let Ok(mut text) = text_q.get_mut(row.value_text) {
            text.0 = progress.level.to_string();
        }
        if let Ok(mut bar) = bar_q.get_mut(row.bar) {
            bar.ratio = progress.ratio();
        }
    }

    for row in &experience_rows {
        if let Ok(mut text) = text_q.get_mut(row.value_text) {
            text.0 = group_thousands(state.experience);
        }
    }
}

fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_are_grouped_from_the_right() {
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(1_000), "1,000");
        assert_eq!(group_thousands(12_345), "12,345");
        assert_eq!(group_thousands(1_234_567), "1,234,567");
    }

    use bevy::ecs::system::RunSystemOnce;

    use crate::game_ui::assets::{UiInventory, UiWindow};

    fn world_with(state: SkillsState) -> World {
        let mut world = World::new();
        world.insert_resource(GameUiAssets {
            font: Handle::default(),
            name_font: Handle::default(),
            window: UiWindow::default(),
            inventory: UiInventory::default(),
            background_dark: Handle::default(),
            background_light: Handle::default(),
            bar_overlay: Handle::default(),
            title_background: Handle::default(),
        });
        world.insert_resource(state);
        world
    }

    /// Rows follow `DISPLAY_ORDER`, not the map's iteration order, and a skill
    /// the snapshot omitted is skipped rather than drawn at a default level.
    #[test]
    fn only_the_skills_the_server_sent_get_a_row() {
        let mut state = SkillsState::default();
        state.skills.insert(
            SkillType::Sword,
            SkillProgress {
                level: 12,
                percent_bp: 4909,
            },
        );
        let mut world = world_with(state);

        world
            .run_system_once(
                |mut commands: Commands, assets: Res<GameUiAssets>, state: Res<SkillsState>| {
                    spawn_skills_content(&mut commands, &assets, &state);
                },
            )
            .unwrap();

        let mut rows = world.query::<&SkillRow>();
        let drawn: Vec<SkillType> = rows.iter(&world).map(|row| row.skill).collect();
        assert_eq!(drawn, vec![SkillType::Sword]);

        let mut experience = world.query::<&ExperienceRow>();
        assert_eq!(
            experience.iter(&world).count(),
            1,
            "the experience row is unconditional"
        );
    }

    /// The window is built once and then patched in place.
    #[test]
    fn the_update_writes_the_level_and_the_ratio() {
        let mut state = SkillsState::default();
        state.skills.insert(
            SkillType::Sword,
            SkillProgress {
                level: 12,
                percent_bp: 4909,
            },
        );
        let mut world = world_with(state);
        world
            .run_system_once(
                |mut commands: Commands, assets: Res<GameUiAssets>, state: Res<SkillsState>| {
                    spawn_skills_content(&mut commands, &assets, &state);
                },
            )
            .unwrap();

        world.resource_mut::<SkillsState>().skills.insert(
            SkillType::Sword,
            SkillProgress {
                level: 13,
                percent_bp: 2_500,
            },
        );
        world.run_system_once(update_skills_window).unwrap();

        let mut rows = world.query::<&SkillRow>();
        let row = rows.iter(&world).next().unwrap();
        let (value_text, bar) = (row.value_text, row.bar);
        assert_eq!(world.get::<Text>(value_text).unwrap().0, "13");
        assert_eq!(world.get::<HudBar>(bar).unwrap().ratio, 0.25);
    }
}
