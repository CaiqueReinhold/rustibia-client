use bevy::{ecs::relationship::RelatedSpawnerCommands, prelude::*};

use crate::{
    conf::ui::{ITEM_SLOT_SIZE, SIDE_PANEL_WIDTH, ui_colors, z_index::Z_WINDOW},
    core::Appearances,
    game_ui::{GameUiAssets, Index, RightPanelDock, UIWindow, UIWindowDock, UiWindowRef, WindowId},
    items::{
        item::InventorySlot,
        ui_item::{UiItem, spawn_ui_item},
    },
    player::{InteractionMode, MouseHoverState, components::PlayerInventory},
};

const INVENTORY_HEIGHT: f32 = 170.0;

#[derive(Component, Debug)]
pub struct InventorySlotUi {
    pub slot: InventorySlot,
}

#[derive(Component)]
pub struct CapDisplay;

#[derive(Component)]
pub struct SoulDisplay;

fn spawn_inventory_display(
    col: &mut RelatedSpawnerCommands<'_, ChildOf>,
    marker: impl Component,
    title: &str,
    content: &str,
    ui_assets: &GameUiAssets,
) {
    col.spawn((
        marker,
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(20.0),
            border: UiRect::all(Val::Px(1.0)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BorderColor {
            top: Color::from(ui_colors::DARK_BORDER_COLOR),
            left: Color::from(ui_colors::DARK_BORDER_COLOR),
            bottom: Color::from(ui_colors::LIGHT_BORDER_COLOR),
            right: Color::from(ui_colors::LIGHT_BORDER_COLOR),
        },
    ))
    .with_child((
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
    ))
    .with_child((
        Text::new(title),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: 6.0,
            ..default()
        },
        TextLayout::new_with_justify(Justify::Center),
        TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
    ))
    .with_child((
        Text::new(content),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: 9.0,
            weight: FontWeight::BOLD,
            ..default()
        },
        TextLayout::new_with_justify(Justify::Center),
        TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
    ));
}

fn spawn_inventory_slot(
    col: &mut RelatedSpawnerCommands<'_, ChildOf>,
    slot_type: InventorySlot,
    placeholder: Handle<Image>,
    background: Handle<Image>,
    item: Option<Entity>,
) {
    let mut commands = col.spawn((
        InventorySlotUi { slot: slot_type },
        Node {
            width: Val::Px(ITEM_SLOT_SIZE),
            height: Val::Px(ITEM_SLOT_SIZE),
            border: UiRect::all(Val::Px(1.0)),
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_content: AlignContent::Center,
            ..default()
        },
        BorderColor {
            top: Color::from(ui_colors::DARK_BORDER_COLOR),
            left: Color::from(ui_colors::DARK_BORDER_COLOR),
            bottom: Color::from(ui_colors::LIGHT_BORDER_COLOR),
            right: Color::from(ui_colors::LIGHT_BORDER_COLOR),
        },
    ));
    commands.observe(on_enter_slot).observe(on_leave_slot);
    if let Some(item) = item {
        commands
            .with_child((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ImageNode {
                    image: background,
                    image_mode: NodeImageMode::Tiled {
                        tile_x: true,
                        tile_y: true,
                        stretch_value: 1.0,
                    },
                    ..default()
                },
            ))
            .add_child(item);
    } else {
        commands.with_child((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode {
                image: placeholder,
                ..default()
            },
        ));
    }
}

pub fn spawn_inventory_ui(
    mut commands: Commands,
    dock_q: Query<(Entity, &UIWindowDock), With<RightPanelDock>>,
    ui_assets: Res<GameUiAssets>,
    inventory: Res<PlayerInventory>,
    appearances: Res<Appearances>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
) {
    let Ok((dock_entity, dock)) = dock_q.single() else {
        return;
    };

    let window_id = WindowId::new();

    let mut spawn_slot_item = |slot: InventorySlot| -> Option<Entity> {
        inventory.items.get(&slot).map(|item| {
            commands
                .spawn(spawn_ui_item(
                    item,
                    &appearances,
                    &mut texture_atlases,
                    &ui_assets,
                    &Vec2::ZERO,
                ))
                .id()
        })
    };

    let amulet_item = spawn_slot_item(InventorySlot::Amulet);
    let left_item = spawn_slot_item(InventorySlot::LeftHand);
    let ring_item = spawn_slot_item(InventorySlot::Ring);
    let head_item = spawn_slot_item(InventorySlot::Head);
    let chest_item = spawn_slot_item(InventorySlot::Chest);
    let legs_item = spawn_slot_item(InventorySlot::Legs);
    let feet_item = spawn_slot_item(InventorySlot::Feet);
    let backpack_item = spawn_slot_item(InventorySlot::Backpack);
    let right_item = spawn_slot_item(InventorySlot::RightHand);
    let trinket_item = spawn_slot_item(InventorySlot::Trinket);

    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            padding: UiRect::all(Val::Px(5.0)),
            justify_content: JustifyContent::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|content| {
            content
                .spawn(Node {
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|col1| {
                    spawn_inventory_slot(
                        col1,
                        InventorySlot::Amulet,
                        ui_assets.inventory.no_amulet.clone(),
                        ui_assets.background_dark.clone(),
                        amulet_item,
                    );
                    spawn_inventory_slot(
                        col1,
                        InventorySlot::LeftHand,
                        ui_assets.inventory.no_left_hand.clone(),
                        ui_assets.background_dark.clone(),
                        left_item,
                    );
                    spawn_inventory_slot(
                        col1,
                        InventorySlot::Ring,
                        ui_assets.inventory.no_ring.clone(),
                        ui_assets.background_dark.clone(),
                        ring_item,
                    );
                    spawn_inventory_display(col1, SoulDisplay, "Soul:", "0", &ui_assets);
                });

            content
                .spawn(Node {
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|col2| {
                    spawn_inventory_slot(
                        col2,
                        InventorySlot::Head,
                        ui_assets.inventory.no_helmet.clone(),
                        ui_assets.background_dark.clone(),
                        head_item,
                    );
                    spawn_inventory_slot(
                        col2,
                        InventorySlot::Chest,
                        ui_assets.inventory.no_armor.clone(),
                        ui_assets.background_dark.clone(),
                        chest_item,
                    );
                    spawn_inventory_slot(
                        col2,
                        InventorySlot::Legs,
                        ui_assets.inventory.no_legs.clone(),
                        ui_assets.background_dark.clone(),
                        legs_item,
                    );
                    spawn_inventory_slot(
                        col2,
                        InventorySlot::Feet,
                        ui_assets.inventory.no_feet.clone(),
                        ui_assets.background_dark.clone(),
                        feet_item,
                    );
                });

            content
                .spawn(Node {
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexEnd,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|col3| {
                    spawn_inventory_slot(
                        col3,
                        InventorySlot::Backpack,
                        ui_assets.inventory.no_backpack.clone(),
                        ui_assets.background_dark.clone(),
                        backpack_item,
                    );
                    spawn_inventory_slot(
                        col3,
                        InventorySlot::RightHand,
                        ui_assets.inventory.no_right_hand.clone(),
                        ui_assets.background_dark.clone(),
                        right_item,
                    );
                    spawn_inventory_slot(
                        col3,
                        InventorySlot::Trinket,
                        ui_assets.background_dark.clone(),
                        ui_assets.background_dark.clone(),
                        trinket_item,
                    );
                    spawn_inventory_display(
                        col3,
                        CapDisplay,
                        "Cap:",
                        &inventory.get_capacity_display(),
                        &ui_assets,
                    );
                });
        })
        .id();

    commands.entity(content).insert(UiWindowRef { window_id });

    let window = commands
        .spawn((
            UIWindow {
                id: window_id,
                dock_id: dock.id,
            },
            Index(0),
            Node {
                left: Val::Px(-2.0),
                width: Val::Px(SIDE_PANEL_WIDTH),
                height: Val::Px(INVENTORY_HEIGHT),
                min_height: Val::Px(INVENTORY_HEIGHT),
                border: UiRect::all(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::hidden(),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
            },
            ZIndex(Z_WINDOW),
        ))
        .add_child(content)
        .id();

    commands.entity(dock_entity).add_child(window);
}

fn on_enter_slot(
    event: On<Pointer<Over>>,
    mut commands: Commands,
    mut hover_state: ResMut<MouseHoverState>,
    slot_q: Query<&InventorySlotUi>,
    mode: Res<InteractionMode>,
) {
    if mode.drag_crossed_threshold() {
        commands.entity(event.entity).insert(Outline {
            width: Val::Px(1.0),
            offset: Val::Px(0.0),
            color: Color::from(ui_colors::ITEM_SLOT_OUTLINE_HOVERED),
        });
    }

    let Ok(slot_ui) = slot_q.get(event.entity) else {
        return;
    };

    hover_state.inventory_slot = Some(slot_ui.slot);
}

fn on_leave_slot(
    event: On<Pointer<Out>>,
    mut commands: Commands,
    mut hover_state: ResMut<MouseHoverState>,
) {
    commands.entity(event.entity).remove::<Outline>();
    hover_state.inventory_slot = None;
}

fn slot_placeholder(slot: &InventorySlot, ui_assets: &GameUiAssets) -> Handle<Image> {
    match slot {
        InventorySlot::Head => ui_assets.inventory.no_helmet.clone(),
        InventorySlot::Amulet => ui_assets.inventory.no_amulet.clone(),
        InventorySlot::Chest => ui_assets.inventory.no_armor.clone(),
        InventorySlot::Backpack => ui_assets.inventory.no_backpack.clone(),
        InventorySlot::LeftHand => ui_assets.inventory.no_left_hand.clone(),
        InventorySlot::RightHand => ui_assets.inventory.no_right_hand.clone(),
        InventorySlot::Ring => ui_assets.inventory.no_ring.clone(),
        InventorySlot::Legs => ui_assets.inventory.no_legs.clone(),
        InventorySlot::Feet => ui_assets.inventory.no_feet.clone(),
        InventorySlot::Trinket | InventorySlot::BothHands => ui_assets.background_dark.clone(),
    }
}

pub fn update_inventory_ui(
    mut commands: Commands,
    inventory: Res<PlayerInventory>,
    slot_q: Query<(Entity, &InventorySlotUi, Option<&Children>)>,
    ui_item_q: Query<&UiItem>,
    ui_assets: Res<GameUiAssets>,
    appearances: Res<Appearances>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
) {
    if !inventory.is_changed() {
        return;
    }

    for (slot_entity, slot_ui, children) in &slot_q {
        let inventory_item = inventory.items.get(&slot_ui.slot);

        let existing_ui_item =
            children.and_then(|ch| ch.iter().find_map(|child| ui_item_q.get(child).ok()));

        let changed = match (inventory_item, existing_ui_item) {
            (Some(inv_item), Some(ui_item)) => inv_item.as_ref() != ui_item.item.as_ref(),
            (None, None) => false,
            _ => true,
        };

        if !changed {
            continue;
        }

        commands.entity(slot_entity).despawn_children();

        if let Some(item) = inventory_item {
            let item_entity = commands
                .spawn(spawn_ui_item(
                    item,
                    &appearances,
                    &mut texture_atlases,
                    &ui_assets,
                    &Vec2::ZERO,
                ))
                .id();
            commands
                .entity(slot_entity)
                .with_child((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        position_type: PositionType::Absolute,
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
                .add_child(item_entity);
        } else {
            commands.entity(slot_entity).with_child((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode {
                    image: slot_placeholder(&slot_ui.slot, &ui_assets),
                    ..default()
                },
            ));
        }
    }
}

pub fn update_capacity(
    inventory: Res<PlayerInventory>,
    cap_q: Query<&Children, With<CapDisplay>>,
    mut text_q: Query<&mut Text>,
) {
    if !inventory.is_changed() {
        return;
    }

    let Ok(children) = cap_q.single() else {
        return;
    };
    let Some(content_entity) = children.get(2) else {
        return;
    };
    let Ok(mut text) = text_q.get_mut(*content_entity) else {
        return;
    };
    text.0 = inventory.get_capacity_display();
}
