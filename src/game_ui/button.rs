use bevy::prelude::*;

use crate::{
    conf::ui::ui_colors::{self, FONT_COLOR_CONTENT},
    game_ui::GameUiAssets,
};

const INACTIVE: Color = Color::NONE;
const HOVERED: Color = Color::Srgba(Srgba::new(1.0, 1.0, 1.0, 0.06));

#[derive(Component)]
pub struct PanelButton;

#[derive(Component)]
pub struct PanelButtonHighlight;

pub fn spawn_panel_button(
    commands: &mut Commands,
    label: impl Into<String>,
    image: Option<Handle<Image>>,
    ui_assets: &GameUiAssets,
    size: Option<(f32, f32)>,
) -> Entity {
    let inner = if let Some(image) = image {
        commands
            .spawn((Node::default(), ImageNode::new(image)))
            .id()
    } else {
        commands
            .spawn((Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },))
            .with_child((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                ImageNode::new(ui_assets.background_light.clone()).with_mode(
                    NodeImageMode::Tiled {
                        tile_x: true,
                        tile_y: true,
                        stretch_value: 1.0,
                    },
                ),
                children![(
                    Text::new(label.into()),
                    TextFont {
                        font: ui_assets.font.clone(),
                        font_size: 11.0,
                        weight: FontWeight::BOLD,
                        ..default()
                    },
                    TextColor(FONT_COLOR_CONTENT.into()),
                )],
            ))
            .id()
    };
    commands
        .spawn((
            PanelButton,
            Button,
            Node {
                min_width: size.map(|s| Val::Px(s.0)).unwrap_or_default(),
                height: size.map(|s| Val::Px(s.1)).unwrap_or_default(),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
            },
        ))
        .add_child(inner)
        .with_child((
            PanelButtonHighlight,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(INACTIVE),
        ))
        .id()
}

pub fn panel_button_hover(
    mut commands: Commands,
    buttons: Query<(Entity, &Interaction, &Children), (Changed<Interaction>, With<PanelButton>)>,
    mut highlights: Query<&mut BackgroundColor, With<PanelButtonHighlight>>,
) {
    for (entity, interaction, children) in &buttons {
        match interaction {
            Interaction::Pressed => {
                commands.entity(entity).insert(BorderColor {
                    top: ui_colors::DARK_BORDER_COLOR.into(),
                    right: ui_colors::LIGHT_BORDER_COLOR.into(),
                    bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                    left: ui_colors::DARK_BORDER_COLOR.into(),
                });
            }
            _ => {
                commands.entity(entity).insert(BorderColor {
                    top: ui_colors::LIGHT_BORDER_COLOR.into(),
                    right: ui_colors::DARK_BORDER_COLOR.into(),
                    bottom: ui_colors::DARK_BORDER_COLOR.into(),
                    left: ui_colors::LIGHT_BORDER_COLOR.into(),
                });
            }
        }
        let mut highlights = highlights.iter_many_mut(children);
        while let Some(mut color) = highlights.fetch_next() {
            color.0 = match interaction {
                Interaction::Hovered => HOVERED,
                _ => INACTIVE,
            };
        }
    }
}
