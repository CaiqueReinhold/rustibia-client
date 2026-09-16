use bevy::prelude::*;

use crate::conf::ui::{cooldown as cooldown_conf, status as conf, ui_colors};
use crate::core::{PlayerStatus, PlayerStatuses};

#[derive(Component, Clone, Copy, Debug)]
pub(super) struct StatusIcon(PlayerStatus);

#[derive(Resource)]
pub(crate) struct StatusAssets {
    sheet: Handle<Image>,
    layout: Handle<TextureAtlasLayout>,
}

pub(super) fn setup_status_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(conf::SHEET_CELL),
        conf::SHEET_COLUMNS,
        1,
        None,
        None,
    ));
    commands.insert_resource(StatusAssets {
        sheet: asset_server.load("ui/statuses.png"),
        layout,
    });
}

fn status_icon(assets: &StatusAssets, status: PlayerStatus) -> ImageNode {
    ImageNode::from_atlas_image(
        assets.sheet.clone(),
        TextureAtlas {
            layout: assets.layout.clone(),
            index: status as usize,
        },
    )
}

pub(super) fn spawn_status_bar(
    commands: &mut Commands,
    background: &Handle<Image>,
    assets: &StatusAssets,
) -> Entity {
    let icons = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(conf::ICON_GAP),
                padding: UiRect::left(Val::Px(conf::PADDING)),
                ..default()
            },
            ImageNode {
                image: background.clone(),
                image_mode: NodeImageMode::Tiled {
                    tile_x: true,
                    tile_y: true,
                    stretch_value: 1.0,
                },
                ..default()
            },
        ))
        .id();

    for status in PlayerStatus::ALL {
        let icon = commands
            .spawn((
                StatusIcon(status),
                status_icon(assets, status),
                Node {
                    width: Val::Px(conf::ICON_SIZE),
                    height: Val::Px(conf::ICON_SIZE),
                    flex_shrink: 0.0,
                    display: Display::None,
                    ..default()
                },
            ))
            .id();
        commands.entity(icons).add_child(icon);
    }

    commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(cooldown_conf::ICON_SIZE),
                border: UiRect::all(Val::Px(conf::BORDER)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
        ))
        .add_child(icons)
        .id()
}

pub(super) fn update_status_icons(
    statuses: Res<PlayerStatuses>,
    mut icons: Query<(&StatusIcon, &mut Node)>,
) {
    for (icon, mut node) in &mut icons {
        let display = if statuses.contains(icon.0) {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn test_assets() -> StatusAssets {
        StatusAssets {
            sheet: Handle::default(),
            layout: Handle::default(),
        }
    }

    #[test]
    fn the_sheet_has_a_column_per_status() {
        assert_eq!(conf::SHEET_COLUMNS as usize, PlayerStatus::ALL.len());
    }

    #[test]
    fn a_status_icon_is_the_cell_of_its_bit() {
        for status in PlayerStatus::ALL {
            let icon = status_icon(&test_assets(), status);

            assert_eq!(
                icon.texture_atlas.unwrap().index,
                status.bit().trailing_zeros() as usize
            );
        }
    }

    #[test]
    fn the_bar_holds_every_icon_hidden_in_bit_order() {
        let mut world = World::new();
        let bar = world
            .run_system_once(|mut commands: Commands| {
                spawn_status_bar(&mut commands, &Handle::default(), &test_assets())
            })
            .unwrap();

        let inner = world.get::<Children>(bar).unwrap()[0];
        let icons: Vec<_> = world
            .get::<Children>(inner)
            .unwrap()
            .iter()
            .map(|icon| {
                (
                    world.get::<StatusIcon>(icon).unwrap().0,
                    world.get::<Node>(icon).unwrap().display,
                )
            })
            .collect();
        assert_eq!(
            icons,
            PlayerStatus::ALL.map(|status| (status, Display::None))
        );
    }

    #[test]
    fn only_the_icons_whose_bits_are_set_are_shown() {
        let mut world = World::new();
        world.insert_resource(PlayerStatuses(
            PlayerStatus::Hungry.bit() | PlayerStatus::Burning.bit(),
        ));
        let icons: Vec<_> = PlayerStatus::ALL
            .into_iter()
            .map(|status| {
                world
                    .spawn((
                        StatusIcon(status),
                        Node {
                            display: Display::None,
                            ..default()
                        },
                    ))
                    .id()
            })
            .collect();
        let shown = |world: &World| -> Vec<PlayerStatus> {
            icons
                .iter()
                .filter(|icon| world.get::<Node>(**icon).unwrap().display == Display::Flex)
                .map(|icon| world.get::<StatusIcon>(*icon).unwrap().0)
                .collect()
        };

        world.run_system_once(update_status_icons).unwrap();
        assert_eq!(shown(&world), [PlayerStatus::Hungry, PlayerStatus::Burning]);

        world.insert_resource(PlayerStatuses::default());
        world.run_system_once(update_status_icons).unwrap();
        assert_eq!(shown(&world), []);
    }
}
