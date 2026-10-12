use std::sync::Arc;

use bevy::input::mouse::MouseScrollUnit;
use bevy::picking::events::Scroll;
use bevy::prelude::*;
use bevy::text::LineBreak;
use bevy::ui_widgets::{
    ControlOrientation, CoreScrollbarThumb, Scrollbar, Slider, SliderPrecision, SliderRange,
    SliderStep, SliderThumb, SliderValue, TrackClick, ValueChange,
};
use bevy_ui_text_input::actions::{TextInputAction, TextInputEdit};
use bevy_ui_text_input::{
    TextInputContents, TextInputMode, TextInputNode, TextInputPrompt, TextInputQueue,
};

use crate::conf::ui::{UI_ITEM_SIZE, dialog as conf, ui_colors};
use crate::core::{Appearances, ItemConfigs};
use crate::game_ui::button::spawn_panel_button;
use crate::game_ui::shop::state::{
    OfferRow, ShopState, ShopTab, Ware, grouped, kept_selection, ounces, rows,
};
use crate::game_ui::{FloatingDialog, FloatingDialogClosed, GameUiAssets};
use crate::items::{Item, ItemFlag, ItemId, fluid_name, slider_amount, spawn_ui_item, thumb_left};
use crate::network::{ClientMessage, SendMessage};
use crate::player::components::PlayerInventory;

#[derive(Component, Debug)]
pub(super) struct ShopWindow {
    npc_name: String,
    tab: ShopTab,
    selected: Option<Ware>,
    search: String,
    shown: Vec<(Ware, bool)>,
    amount_for: Option<Ware>,
    icon_for: Option<Ware>,
}

impl ShopWindow {
    fn new(npc_name: &str) -> Self {
        Self {
            npc_name: npc_name.to_string(),
            tab: ShopTab::Buy,
            selected: None,
            search: String::new(),
            shown: Vec::new(),
            amount_for: None,
            icon_for: None,
        }
    }
}

#[derive(Component)]
pub(super) struct ShopList;

#[derive(Component)]
pub(super) struct OfferLine(Ware);

#[derive(Component)]
pub(super) struct TabButton(ShopTab);

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub(super) enum Readout {
    Amount,
    Price,
    Gold,
}

#[derive(Component)]
pub(super) struct ShopSlider;

#[derive(Component)]
pub(super) struct ShopSearch;

#[derive(Component)]
pub(super) struct SelectedIcon;

fn trade_request(window: &ShopWindow, value: f32, max: u16) -> Option<ClientMessage> {
    let ware = window.selected?;
    let amount = slider_amount(value, max.min(u8::MAX as u16) as u8);
    let (item_id, subtype) = (ware.item_id, ware.subtype);
    Some(match window.tab {
        ShopTab::Buy => ClientMessage::ShopBuy {
            item_id,
            subtype,
            amount,
        },
        ShopTab::Sell => ClientMessage::ShopSell {
            item_id,
            subtype,
            amount,
        },
    })
}

pub(super) fn on_close_clicked(
    event: On<FloatingDialogClosed>,
    windows: Query<(), With<ShopWindow>>,
    mut state: ResMut<ShopState>,
    mut commands: Commands,
) {
    if windows.get(event.dialog).is_err() {
        return;
    }
    commands.trigger(SendMessage(ClientMessage::CloseShop));
    state.0 = None;
    commands.entity(event.dialog).despawn();
}

#[allow(clippy::too_many_arguments)]
pub(super) fn open_or_close_window(
    mut commands: Commands,
    state: Res<ShopState>,
    windows: Query<(Entity, &ShopWindow)>,
    ui_assets: Res<GameUiAssets>,
    configs: Res<ItemConfigs>,
    appearances: Res<Appearances>,
    mut atlases: ResMut<Assets<TextureAtlasLayout>>,
    window: Single<&Window>,
) {
    let open = windows.single().ok();
    match (&state.0, open) {
        (Some(shop), Some((_, view))) if view.npc_name == shop.npc_name => {}
        (Some(shop), open) => {
            if let Some((entity, _)) = open {
                commands.entity(entity).despawn();
            }
            let at = Vec2::new(
                ((window.width() - conf::SHOP_WIDTH) / 2.0).max(0.0),
                conf::SHOP_TOP,
            );
            let icons = Icons {
                ui_assets: &ui_assets,
                configs: &configs,
                appearances: &appearances,
            };
            spawn_window(&mut commands, &icons, &mut atlases, &shop.npc_name, at);
        }
        (None, Some((entity, _))) => commands.entity(entity).despawn(),
        (None, None) => {}
    }
}

/// What drawing an item's sprite in the UI needs.
struct Icons<'a> {
    ui_assets: &'a GameUiAssets,
    configs: &'a ItemConfigs,
    appearances: &'a Appearances,
}

impl Icons<'_> {
    fn spawn(
        &self,
        commands: &mut Commands,
        atlases: &mut Assets<TextureAtlasLayout>,
        id: ItemId,
        amount: u32,
    ) -> Entity {
        match self.configs.items.get(&id) {
            Some(config) => {
                let item = Arc::new(Item::new(Arc::clone(config), amount));
                commands
                    .spawn(spawn_ui_item(
                        &item,
                        self.appearances,
                        atlases,
                        self.ui_assets,
                        &Vec2::ZERO,
                    ))
                    .insert(Pickable::IGNORE)
                    .id()
            }
            None => commands
                .spawn((
                    Node {
                        width: Val::Px(UI_ITEM_SIZE),
                        height: Val::Px(UI_ITEM_SIZE),
                        ..default()
                    },
                    Pickable::IGNORE,
                ))
                .id(),
        }
    }

    fn spawn_ware(
        &self,
        commands: &mut Commands,
        atlases: &mut Assets<TextureAtlasLayout>,
        ware: Ware,
    ) -> Entity {
        let holds_fluid = self
            .configs
            .items
            .get(&ware.item_id)
            .is_some_and(|config| config.has_flag(ItemFlag::LiquidContainer));
        let amount = if holds_fluid { ware.subtype as u32 } else { 1 };
        self.spawn(commands, atlases, ware.item_id, amount)
    }

    fn name(&self, ware: Ware) -> String {
        let name = self
            .configs
            .items
            .get(&ware.item_id)
            .and_then(|config| config.name.clone())
            .unwrap_or_else(|| format!("item {}", ware.item_id.0));
        match fluid_name(ware.subtype) {
            Some(fluid) => format!("{name} of {fluid}"),
            None => name,
        }
    }
}

fn spawn_window(
    commands: &mut Commands,
    icons: &Icons,
    atlases: &mut Assets<TextureAtlasLayout>,
    npc_name: &str,
    at: Vec2,
) {
    let ui_assets = icons.ui_assets;
    let handle = FloatingDialog::new("NPC Trade", conf::SHOP_WIDTH, at).spawn(commands, ui_assets);
    let root = handle.root;
    commands.entity(root).insert(ShopWindow::new(npc_name));

    let currency_icon = icons.spawn(commands, atlases, ItemId(conf::SHOP_CURRENCY_ITEM), 100);
    let currency = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            flex_grow: 1.0,
            ..default()
        })
        .with_child(label(ui_assets, "Currency:", ui_colors::FONT_COLOR_CONTENT))
        .with_child(label(ui_assets, "Gold Coin", ui_colors::FONT_COLOR_CONTENT))
        .id();
    let tabs = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            margin: UiRect::left(Val::Px(4.0)),
            ..default()
        })
        .id();
    for (tab, text) in [(ShopTab::Buy, "Buy"), (ShopTab::Sell, "Sell")] {
        let button = spawn_tab(commands, ui_assets, tab, text, root);
        commands.entity(tabs).add_child(button);
    }
    let header = commands
        .spawn(Node {
            align_items: AlignItems::Center,
            margin: UiRect::bottom(Val::Px(4.0)),
            ..default()
        })
        .add_children(&[currency, currency_icon, tabs])
        .id();

    let list = commands
        .spawn((
            ShopList,
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
        ))
        .id();
    commands.entity(list).observe(
        move |mut scroll: On<Pointer<Scroll>>,
              mut lists: Query<(&mut ScrollPosition, &ComputedNode)>| {
            scroll.propagate(false);
            let Ok((mut position, computed)) = lists.get_mut(list) else {
                return;
            };
            let max = ((computed.content_size() - computed.size())
                * computed.inverse_scale_factor())
            .y
            .max(0.0);
            let delta = match scroll.unit {
                MouseScrollUnit::Line => scroll.y * conf::SHOP_ROW_HEIGHT,
                MouseScrollUnit::Pixel => scroll.y,
            };
            position.y = (position.y - delta).clamp(0.0, max);
        },
    );
    let scrollbar = commands
        .spawn((
            Node {
                min_width: Val::Px(10.0),
                height: Val::Percent(100.0),
                ..default()
            },
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
            Scrollbar::new(list, ControlOrientation::Vertical, 10.0),
            Children::spawn(Spawn((
                Node {
                    position_type: PositionType::Absolute,
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                raised(),
                BackgroundColor(conf::BUTTON_COLOR.into()),
                CoreScrollbarThumb,
            ))),
        ))
        .id();
    let list_frame = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(conf::SHOP_ROW_HEIGHT * conf::SHOP_VISIBLE_ROWS + 2.0),
                border: UiRect::all(Val::Px(1.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            sunken(),
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
        ))
        .add_children(&[list, scrollbar])
        .id();

    let field = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(conf::FIELD_HEIGHT - 4.0),
                border: UiRect::all(Val::Px(1.0)),
                padding: UiRect::axes(Val::Px(3.0), Val::Px(2.0)),
                ..default()
            },
            sunken(),
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
        ))
        .with_child((
            ShopSearch,
            TextInputNode {
                mode: TextInputMode::SingleLine,
                clear_on_submit: false,
                ..default()
            },
            TextInputContents::default(),
            TextInputQueue::default(),
            TextInputPrompt {
                text: "Type to search".to_string(),
                ..default()
            },
            font(ui_assets, 11.0),
            TextColor(Color::WHITE),
        ))
        .id();
    let clear = commands
        .spawn((
            Node {
                width: Val::Px(conf::FIELD_HEIGHT - 4.0),
                height: Val::Px(conf::FIELD_HEIGHT - 4.0),
                margin: UiRect::left(Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            raised(),
        ))
        .with_child((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::new(ui_assets.window.close_button.clone()),
            Pickable::IGNORE,
        ))
        .observe(
            |_: On<Pointer<Click>>, mut fields: Query<&mut TextInputQueue, With<ShopSearch>>| {
                if let Ok(mut queue) = fields.single_mut() {
                    queue.add(TextInputAction::Edit(TextInputEdit::SelectAll));
                    queue.add(TextInputAction::Edit(TextInputEdit::Delete));
                }
            },
        )
        .id();
    let search = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            margin: UiRect::vertical(Val::Px(4.0)),
            align_items: AlignItems::Center,
            ..default()
        })
        .add_children(&[field, clear])
        .id();

    let slider = commands
        .spawn((
            ShopSlider,
            Slider {
                track_click: TrackClick::Snap,
            },
            SliderValue(1.0),
            SliderRange::new(1.0, 1.0),
            SliderStep(1.0),
            SliderPrecision(0),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(12.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            sunken(),
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
            Children::spawn(Spawn((
                SliderThumb,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(10.0),
                    height: Val::Percent(100.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                raised(),
                BackgroundColor(conf::BUTTON_COLOR.into()),
            ))),
        ))
        .observe(|change: On<ValueChange<f32>>, mut commands: Commands| {
            commands
                .entity(change.source)
                .insert(SliderValue(change.value));
        })
        .id();

    let readouts = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            flex_grow: 1.0,
            ..default()
        })
        .id();
    for (readout, text) in [
        (Readout::Amount, "Amount:"),
        (Readout::Price, "Price:"),
        (Readout::Gold, "Gold:"),
    ] {
        let line = commands
            .spawn(Node::default())
            .with_child((
                label(ui_assets, text, ui_colors::FONT_COLOR_CONTENT),
                Node {
                    width: Val::Px(52.0),
                    ..default()
                },
            ))
            .with_child((readout, label(ui_assets, "", Srgba::WHITE)))
            .id();
        commands.entity(readouts).add_child(line);
    }
    let selected_icon = commands
        .spawn((
            SelectedIcon,
            Node {
                width: Val::Px(UI_ITEM_SIZE + 2.0),
                height: Val::Px(UI_ITEM_SIZE + 2.0),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            sunken(),
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
        ))
        .id();
    let ok = spawn_panel_button(commands, "Ok", None, ui_assets, Some((40.0, 20.0)));
    commands.entity(ok).observe(
        move |_: On<Pointer<Click>>,
              windows: Query<&ShopWindow>,
              sliders: Query<(&SliderValue, &SliderRange), With<ShopSlider>>,
              mut commands: Commands| {
            let (Ok(window), Ok((value, range))) = (windows.get(root), sliders.single()) else {
                return;
            };
            if let Some(message) = trade_request(window, value.0, range.end() as u16) {
                commands.trigger(SendMessage(message));
            }
        },
    );
    let actions = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .add_children(&[selected_icon, ok])
        .id();
    let footer = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            ..default()
        })
        .add_children(&[readouts, actions])
        .id();

    commands
        .entity(handle.content)
        .add_children(&[header, list_frame, search, slider, footer]);
}

fn spawn_tab(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    tab: ShopTab,
    text: &str,
    root: Entity,
) -> Entity {
    commands
        .spawn((
            TabButton(tab),
            Node {
                width: Val::Px(44.0),
                height: Val::Px(18.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            raised(),
            ImageNode::new(ui_assets.background_light.clone()).with_mode(NodeImageMode::Tiled {
                tile_x: true,
                tile_y: true,
                stretch_value: 1.0,
            }),
        ))
        .with_child((
            Text::new(text),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                weight: FontWeight::BOLD,
                ..default()
            },
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            Pickable::IGNORE,
        ))
        .observe(
            move |_: On<Pointer<Click>>, mut windows: Query<&mut ShopWindow>| {
                if let Ok(mut window) = windows.get_mut(root)
                    && window.tab != tab
                {
                    window.tab = tab;
                    window.selected = None;
                }
            },
        )
        .id()
}

fn spawn_line(
    commands: &mut Commands,
    icons: &Icons,
    atlases: &mut Assets<TextureAtlasLayout>,
    row: &OfferRow,
    root: Entity,
) -> Entity {
    let color: Color = if row.max > 0 {
        Color::WHITE
    } else {
        conf::SHOP_DIM_TEXT_COLOR.into()
    };
    let icon = icons.spawn_ware(commands, atlases, row.ware);
    let text = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                flex_grow: 1.0,
                overflow: Overflow::clip(),
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_child((
            Text::new(row.name.clone()),
            font(icons.ui_assets, 11.0),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            TextColor(color),
            Pickable::IGNORE,
        ))
        .with_child((
            Text::new(format!(
                "Price {}, {}",
                grouped(row.price as u64),
                ounces(row.weight)
            )),
            font(icons.ui_assets, 10.0),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            TextColor(color),
            Pickable::IGNORE,
        ))
        .id();
    let ware = row.ware;
    let tradeable = row.max > 0;
    commands
        .spawn((
            OfferLine(ware),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(conf::SHOP_ROW_HEIGHT),
                flex_shrink: 0.0,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(2.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .add_children(&[icon, text])
        .observe(
            move |click: On<Pointer<Click>>, mut windows: Query<&mut ShopWindow>| {
                if click.button != PointerButton::Primary || !tradeable {
                    return;
                }
                if let Ok(mut window) = windows.get_mut(root)
                    && window.selected != Some(ware)
                {
                    window.selected = Some(ware);
                }
            },
        )
        .id()
}

pub(super) fn read_search(
    fields: Query<&TextInputContents, (With<ShopSearch>, Changed<TextInputContents>)>,
    mut windows: Query<&mut ShopWindow>,
) {
    let (Ok(contents), Ok(mut window)) = (fields.single(), windows.single_mut()) else {
        return;
    };
    if window.search != contents.get() {
        window.search = contents.get().to_string();
    }
}

/// Re-derives the whole window from `ShopState` every frame; only what differs is written.
#[allow(clippy::too_many_arguments)]
pub(super) fn refresh_window(
    mut commands: Commands,
    state: Res<ShopState>,
    inventory: Res<PlayerInventory>,
    configs: Res<ItemConfigs>,
    appearances: Res<Appearances>,
    mut atlases: ResMut<Assets<TextureAtlasLayout>>,
    ui_assets: Res<GameUiAssets>,
    mut windows: Query<(Entity, &mut ShopWindow)>,
    list: Query<Entity, With<ShopList>>,
    selected_icon: Query<Entity, With<SelectedIcon>>,
    mut lines: Query<(&OfferLine, &mut BackgroundColor)>,
    mut tabs: Query<(&TabButton, &mut BorderColor)>,
    mut readouts: Query<(&Readout, &mut Text)>,
    sliders: Query<(Entity, &SliderValue, &SliderRange), With<ShopSlider>>,
) {
    let (Some(shop), Ok((root, mut window))) = (&state.0, windows.single_mut()) else {
        return;
    };
    let icons = Icons {
        ui_assets: &ui_assets,
        configs: &configs,
        appearances: &appearances,
    };
    let rows = rows(
        shop,
        window.tab,
        &window.search,
        inventory.capacity,
        |ware| icons.name(ware),
    );
    let selected = kept_selection(&rows, window.selected);
    if window.selected != selected {
        window.selected = selected;
    }

    let shown: Vec<(Ware, bool)> = rows.iter().map(|row| (row.ware, row.max > 0)).collect();
    if window.shown != shown
        && let Ok(list) = list.single()
    {
        commands.entity(list).despawn_children();
        for row in &rows {
            let line = spawn_line(&mut commands, &icons, &mut atlases, row, root);
            commands.entity(list).add_child(line);
        }
        window.shown = shown;
    }
    if window.icon_for != selected
        && let Ok(slot) = selected_icon.single()
    {
        commands.entity(slot).despawn_children();
        if let Some(ware) = selected {
            let icon = icons.spawn_ware(&mut commands, &mut atlases, ware);
            commands.entity(slot).add_child(icon);
        }
        window.icon_for = selected;
    }

    for (line, mut background) in &mut lines {
        let color = if Some(line.0) == selected {
            conf::ROW_SELECTED_COLOR.into()
        } else {
            Color::NONE
        };
        if background.0 != color {
            background.0 = color;
        }
    }
    for (tab, mut border) in &mut tabs {
        let wanted = if tab.0 == window.tab {
            sunken()
        } else {
            raised()
        };
        if *border != wanted {
            *border = wanted;
        }
    }

    let row = selected.and_then(|ware| rows.iter().find(|row| row.ware == ware));
    let fresh = window.amount_for != selected;
    if fresh {
        window.amount_for = selected;
    }
    let mut amount = 1.0;
    if let Ok((slider, value, range)) = sliders.single() {
        let max = row.map_or(1, |row| row.max.max(1)) as f32;
        let start = match (fresh, window.tab) {
            (true, ShopTab::Buy) => 1.0,
            (true, ShopTab::Sell) => max,
            (false, _) => value.0,
        };
        amount = start.clamp(1.0, max);
        if range.end() != max || value.0 != amount {
            commands
                .entity(slider)
                .insert((SliderRange::new(1.0, max), SliderValue(amount)));
        }
    }
    let amount = amount.round() as u64;
    for (readout, mut text) in &mut readouts {
        let value = match (readout, row) {
            (Readout::Amount, Some(_)) => amount.to_string(),
            (Readout::Price, Some(row)) => grouped(row.price as u64 * amount),
            (Readout::Gold, _) => grouped(shop.money),
            (_, None) => String::new(),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// The widget owns the value and nothing else; the thumb is placed here every frame for the
/// reason `sync_split_slider` gives.
pub(super) fn sync_shop_slider(
    sliders: Query<(&SliderValue, &SliderRange, &ComputedNode, &Children), With<ShopSlider>>,
    mut thumbs: Query<(&mut Node, &ComputedNode), With<SliderThumb>>,
) {
    for (value, range, slider_node, children) in &sliders {
        for child in children.iter() {
            let Ok((mut thumb, thumb_node)) = thumbs.get_mut(child) else {
                continue;
            };
            let left = thumb_left(range, value.0, slider_node, thumb_node);
            if thumb.left != left {
                thumb.left = left;
            }
        }
    }
}

fn label(ui_assets: &GameUiAssets, text: &str, color: Srgba) -> impl Bundle {
    (
        Text::new(text),
        font(ui_assets, 11.0),
        TextColor(color.into()),
    )
}

fn font(ui_assets: &GameUiAssets, size: f32) -> TextFont {
    TextFont {
        font: ui_assets.font.clone(),
        font_size: size,
        ..default()
    }
}

fn raised() -> BorderColor {
    BorderColor {
        top: ui_colors::LIGHT_BORDER_COLOR.into(),
        right: ui_colors::DARK_BORDER_COLOR.into(),
        bottom: ui_colors::DARK_BORDER_COLOR.into(),
        left: ui_colors::LIGHT_BORDER_COLOR.into(),
    }
}

fn sunken() -> BorderColor {
    BorderColor {
        top: ui_colors::DARK_BORDER_COLOR.into(),
        right: ui_colors::LIGHT_BORDER_COLOR.into(),
        bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
        left: ui_colors::DARK_BORDER_COLOR.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_ui::shop::state::{Shop, ShopOffer, Ware};

    fn a_shop() -> Shop {
        Shop {
            npc_name: "Xodet".to_string(),
            money: 100,
            offers: vec![ShopOffer {
                item_id: ItemId(3003),
                subtype: 0,
                weight: 0,
                buy: 50,
                sell: 0,
            }],
            goods: Default::default(),
        }
    }

    #[derive(Resource, Default)]
    struct Sent(Vec<String>);

    fn a_world() -> World {
        let mut world = World::new();
        world.init_resource::<Sent>();
        world.add_observer(|event: On<SendMessage>, mut sent: ResMut<Sent>| {
            sent.0.push(format!("{:?}", event.0));
        });
        world.add_observer(on_close_clicked);
        world
    }

    #[test]
    fn the_close_button_tells_the_server_and_forgets_the_shop() {
        let mut world = a_world();
        world.insert_resource(ShopState(Some(a_shop())));
        let window = world.spawn(ShopWindow::new("Xodet")).id();

        world.trigger(FloatingDialogClosed { dialog: window });
        world.flush();

        assert_eq!(world.resource::<Sent>().0, ["CloseShop"]);
        assert!(world.resource::<ShopState>().0.is_none());
        assert!(world.get_entity(window).is_err());
    }

    #[test]
    fn another_dialogs_close_button_is_ignored() {
        let mut world = a_world();
        world.insert_resource(ShopState(Some(a_shop())));
        let other = world.spawn_empty().id();

        world.trigger(FloatingDialogClosed { dialog: other });
        world.flush();

        assert!(world.resource::<Sent>().0.is_empty());
        assert!(world.resource::<ShopState>().0.is_some());
    }

    #[test]
    fn a_trade_request_names_the_selection_and_the_slider_amount() {
        let mut window = ShopWindow::new("Xodet");
        window.selected = Some(Ware {
            item_id: ItemId(3003),
            subtype: 0,
        });

        assert_eq!(
            format!("{:?}", trade_request(&window, 6.6, 50)),
            "Some(ShopBuy { item_id: ItemId(3003), subtype: 0, amount: 7 })"
        );
        window.tab = ShopTab::Sell;
        assert_eq!(
            format!("{:?}", trade_request(&window, 99.0, 3)),
            "Some(ShopSell { item_id: ItemId(3003), subtype: 0, amount: 3 })"
        );
        window.selected = None;
        assert!(trade_request(&window, 1.0, 3).is_none());
    }
}
