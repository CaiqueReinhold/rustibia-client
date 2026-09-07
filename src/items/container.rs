use std::sync::Arc;

use bevy::prelude::*;

use crate::{
    conf::ui::{ITEM_SLOT_SIZE, LOOT_CONTAINER_DEFAULT_HEIGHT, ui_colors},
    core::{Appearances, ItemConfigs},
    game_ui::{AddUIWindow, CloseUIWindow, GameUiAssets, ReplaceUIWindowContent, UiWindowRef},
    items::{Item, ItemId, OpenParentContainer, ui_item::spawn_ui_item},
    network::{
        ClientMessage, SendMessage,
        events::{ClientOutdated, ContainerClosed, OpenContainer, UpdateContainer},
    },
    player::{ContainerNavTarget, MouseHoverState},
};

/// An open container as this player's session names it. Session-local to the server, and reused.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct ContainerId(pub u16);

#[derive(Resource)]
pub struct PreventContainerCloseEvent {
    container_id: ContainerId,
}

#[derive(Component)]
pub struct LootContainerUI {
    pub container_id: ContainerId,
    pub capacity: usize,
    pub items: Vec<Arc<Item>>,
}

impl LootContainerUI {
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }
}

#[derive(Component)]
pub struct ContainerSlot {
    index: usize,
}

fn on_enter_slot(
    event: On<Pointer<Over>>,
    mut commands: Commands,
    mut hover_state: ResMut<MouseHoverState>,
    container_q: Query<(&ChildOf, &ContainerSlot)>,
) {
    commands.entity(event.entity).insert(Outline {
        width: Val::Px(1.0),
        offset: Val::Px(0.0),
        color: Color::from(ui_colors::ITEM_SLOT_OUTLINE_HOVERED),
    });
    let Ok((container, slot)) = container_q.get(event.entity) else {
        return;
    };
    hover_state.container = Some(container.parent());
    hover_state.container_slot = Some(slot.index);
}

fn on_leave_slot(
    event: On<Pointer<Out>>,
    mut commands: Commands,
    mut hover_state: ResMut<MouseHoverState>,
) {
    commands.entity(event.entity).insert(Outline {
        width: Val::Px(1.0),
        offset: Val::Px(0.0),
        color: Color::from(ui_colors::ITEM_SLOT_OUTLINE),
    });
    hover_state.container = None;
    hover_state.container_slot = None;
}

/// `None` when the server named an item this client doesn't have — the caller
/// raises [`ClientOutdated`] rather than showing a container with holes in it.
fn as_item_vec(items: &[Option<(ItemId, u8)>], configs: &ItemConfigs) -> Option<Vec<Arc<Item>>> {
    let mut items_vec = Vec::new();
    for it in items.iter() {
        let Some((id, amount)) = it else {
            break;
        };
        let config = configs.items.get(id)?;
        items_vec.push(Arc::new(Item::new(config.clone(), *amount as u32)));
    }
    Some(items_vec)
}

pub fn on_open_container(
    event: On<OpenContainer>,
    mut commands: Commands,
    configs: Res<ItemConfigs>,
    ui_assets: Res<GameUiAssets>,
    loot_container_q: Query<(&LootContainerUI, &UiWindowRef)>,
    nav_target: Option<Res<ContainerNavTarget>>,
) {
    let nav = nav_target.as_ref().map(|n| n.0);
    let existing = loot_container_q
        .iter()
        .find(|(c, _)| c.container_id == event.container_id)
        .map(|(_, w)| w.window_id);

    let target_window = match nav {
        Some(nav_window) => {
            if let Some(existing_window) = existing
                && existing_window != nav_window
            {
                commands.insert_resource(PreventContainerCloseEvent {
                    container_id: event.container_id,
                });
                commands.trigger(CloseUIWindow {
                    window_id: existing_window,
                });
            }
            Some(nav_window)
        }
        // No navigation in progress. If the container already has a window, leave
        // in-place refreshes to UpdateContainer — rebuilding it here would tear
        // the window down (and spuriously close it server-side). Otherwise this
        // is a fresh open, so spawn a new window.
        None => match existing {
            Some(_) => return,
            None => None,
        },
    };

    let Some(items) = as_item_vec(&event.items, &configs) else {
        commands.trigger(ClientOutdated);
        return;
    };
    let container = LootContainerUI {
        container_id: event.container_id,
        capacity: event.capacity as usize,
        items,
    };
    let grid = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                padding: UiRect {
                    top: Val::Px(2.0),
                    left: Val::Px(4.0),
                    bottom: Val::ZERO,
                    right: Val::ZERO,
                },
                row_gap: Val::Px(3.0),
                column_gap: Val::Px(3.0),
                justify_content: JustifyContent::FlexStart,
                ..default()
            },
            Transform::default(),
        ))
        .id();

    for i in 0..container.capacity {
        let mut slot_cmds = commands.spawn((
            ContainerSlot { index: i },
            Node {
                width: Val::Px(ITEM_SLOT_SIZE),
                height: Val::Px(ITEM_SLOT_SIZE),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            Transform::default(),
        ));

        slot_cmds.with_children(|parent| {
            parent.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    image: ui_assets.background_dark.clone(),
                    ..default()
                },
            ));
        });
        slot_cmds.observe(on_enter_slot);
        slot_cmds.observe(on_leave_slot);

        let slot_id = slot_cmds.id();
        commands.entity(grid).add_child(slot_id);
    }

    commands.entity(grid).insert(container);

    let container_id = event.container_id;
    let custom_buttons = if event.has_parent {
        let button = commands
            .spawn((
                Node {
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    border: UiRect::all(Val::Px(1.0)),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_content: AlignContent::Center,
                    ..default()
                },
                BorderColor {
                    top: ui_colors::LIGHT_BORDER_COLOR.into(),
                    right: ui_colors::DARK_BORDER_COLOR.into(),
                    bottom: ui_colors::DARK_BORDER_COLOR.into(),
                    left: ui_colors::LIGHT_BORDER_COLOR.into(),
                },
            ))
            .with_child((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    image: ui_assets.window.parent_container.clone(),
                    ..default()
                },
            ))
            .observe(move |mut e: On<Pointer<Click>>, mut commands: Commands| {
                e.propagate(false);
                commands.trigger(OpenParentContainer { container_id });
            })
            .observe(|mut e: On<Pointer<DragStart>>| {
                e.propagate(false);
            })
            .id();
        vec![button]
    } else {
        Vec::new()
    };

    match target_window {
        Some(window_id) => commands.trigger(ReplaceUIWindowContent {
            window_id,
            content: grid,
            title: event.title.clone(),
            custom_buttons,
        }),
        None => commands.trigger(AddUIWindow {
            content: grid,
            default_height: LOOT_CONTAINER_DEFAULT_HEIGHT,
            title: event.title.clone(),
            custom_buttons,
        }),
    }

    if nav.is_some() {
        commands.remove_resource::<ContainerNavTarget>();
    }
}

pub fn on_open_parent_container(
    event: On<OpenParentContainer>,
    mut commands: Commands,
    loot_container_q: Query<(&LootContainerUI, &UiWindowRef)>,
) {
    let Some((_, window_ref)) = loot_container_q
        .iter()
        .find(|c| c.0.container_id == event.container_id)
    else {
        return;
    };

    commands.insert_resource(ContainerNavTarget(window_ref.window_id));
    commands.trigger(SendMessage(ClientMessage::OpenParentContainer {
        container_id: event.container_id,
    }));
}

pub fn on_update_container(
    event: On<UpdateContainer>,
    mut commands: Commands,
    configs: Res<ItemConfigs>,
    mut loot_container_q: Query<&mut LootContainerUI>,
) {
    let Some(items) = as_item_vec(&event.items, &configs) else {
        commands.trigger(ClientOutdated);
        return;
    };
    for mut container in loot_container_q.iter_mut() {
        if container.container_id == event.container_id {
            container.items = items;
            break;
        }
    }
}

pub fn on_container_closed_by_server(
    event: On<ContainerClosed>,
    mut commands: Commands,
    loot_container_q: Query<(&LootContainerUI, &UiWindowRef)>,
) {
    for (container_ui, window_ref) in loot_container_q.iter() {
        if container_ui.container_id == event.container_id {
            commands.trigger(CloseUIWindow {
                window_id: window_ref.window_id,
            });
            break;
        }
    }
}

pub fn on_container_ui_closed(
    event: On<Remove, LootContainerUI>,
    mut commands: Commands,
    loot_container_q: Query<&LootContainerUI>,
    prevent_close: Option<Res<PreventContainerCloseEvent>>,
) {
    let Ok(loot_container) = loot_container_q.get(event.entity) else {
        return;
    };

    if let Some(prevent_close) = prevent_close.as_ref()
        && prevent_close.container_id == loot_container.container_id
    {
        commands.remove_resource::<PreventContainerCloseEvent>();
        return;
    }

    commands.trigger(SendMessage(ClientMessage::CloseContainer {
        container_id: loot_container.container_id,
    }));
}

pub fn container_content_changed(
    mut commands: Commands,
    container_q: Query<(&LootContainerUI, &Children), Changed<LootContainerUI>>,
    appearances: Res<Appearances>,
    ui_assets: Res<GameUiAssets>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
) {
    for (container, children) in container_q {
        for (i, child) in children.iter().enumerate() {
            commands.entity(child).despawn_children();
            commands.entity(child).with_child((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
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
            ));
            if let Some(item) = container.items.get(i) {
                commands.entity(child).with_children(|item_container| {
                    item_container
                        .spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                padding: UiRect::all(Val::Px(2.0)),
                                ..default()
                            },
                            Transform::default(),
                        ))
                        .with_child(spawn_ui_item(
                            item,
                            &appearances,
                            &mut texture_atlases,
                            &ui_assets,
                            &Vec2::ZERO,
                        ));
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn configs(ids: &[ItemId]) -> ItemConfigs {
        let mut items = HashMap::new();
        for id in ids {
            items.insert(
                *id,
                Arc::new(crate::items::ItemConfig {
                    id: *id,
                    flags: Vec::new(),
                    friction: None,
                    slot: None,
                    minimap_color: None,
                    elevation: None,
                }),
            );
        }
        ItemConfigs { items }
    }

    #[test]
    fn resolves_the_items_up_to_the_first_empty_slot() {
        let configs = configs(&[ItemId(100), ItemId(200)]);
        let items = as_item_vec(
            &[
                Some((ItemId(100), 1)),
                Some((ItemId(200), 5)),
                None,
                Some((ItemId(100), 1)),
            ],
            &configs,
        )
        .expect("all ids are known");

        assert_eq!(items.len(), 2, "the empty slot ends the container");
        assert_eq!(items[0].config.id, ItemId(100));
        assert_eq!(items[1].config.id, ItemId(200));
        assert_eq!(items[1].amount, 5);
    }

    /// An id this client's assets don't have is a version mismatch, not a
    /// container with one item missing — the caller turns this into
    /// `ClientOutdated`.
    #[test]
    fn rejects_the_whole_container_on_an_unknown_id() {
        let configs = configs(&[ItemId(100)]);
        assert!(as_item_vec(&[Some((ItemId(100), 1)), Some((ItemId(999), 1))], &configs).is_none());
    }
}
