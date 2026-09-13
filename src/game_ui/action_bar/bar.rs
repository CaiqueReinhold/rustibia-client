use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::text::FontSmoothing;
use bevy_text_outline::TextOutline;

use crate::conf::ui::{UI_ITEM_SIZE, action_bar as conf, dialog, ui_colors};
use crate::core::{Appearances, ItemConfigs, SpellBook, SpriteAnimator, SpriteSheet};
use crate::game_ui::GameUiAssets;
use crate::game_ui::cooldown::{CooldownOverlay, spawn_cooldown_overlay};
use crate::game_ui::scaling::logical_size;
use crate::items::{Item, ItemId};
use crate::player::Hotkey;

use super::activation::ActionSlotActivated;
use super::menu::OpenSlotMenu;
use super::state::{ActionBar, SlotAction};

#[derive(Resource)]
pub struct ActionBarAssets {
    pub spells: Handle<Image>,
    pub spell_layout: Handle<TextureAtlasLayout>,
    /// One layout per sprite sheet, so a redraw reuses layouts instead of adding one per slot.
    item_layouts: HashMap<String, Handle<TextureAtlasLayout>>,
}

impl ActionBarAssets {
    fn item_layout(
        &mut self,
        sheet: &SpriteSheet,
        layouts: &mut Assets<TextureAtlasLayout>,
    ) -> Handle<TextureAtlasLayout> {
        self.item_layouts
            .entry(sheet.sheet_name.clone())
            .or_insert_with(|| {
                layouts.add(TextureAtlasLayout::from_grid(
                    sheet.sprite_size.as_uvec2(),
                    sheet.grid_size.x as u32,
                    sheet.grid_size.y as u32,
                    None,
                    None,
                ))
            })
            .clone()
    }
}

#[derive(Component)]
pub(super) struct ActionBarRoot;

#[derive(Component)]
pub(super) struct ActionSlotCell(pub u16);

#[derive(Resource, Debug, Default)]
pub(super) struct VisibleSlots(pub u16);

pub fn visible_slot_count(content_width: f32) -> u16 {
    ((content_width + conf::SLOT_GAP) / (conf::SLOT_SIZE + conf::SLOT_GAP)).floor() as u16
}

/// The atlas index of a 1-based icon cell, if the sheet has that cell.
pub fn spell_icon_index(icon: u16) -> Option<usize> {
    let index = usize::from(icon.checked_sub(1)?);
    (index < (conf::SPELL_ICON_COLUMNS * conf::SPELL_ICON_ROWS) as usize).then_some(index)
}

pub(super) fn setup_action_bar_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let spell_layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(conf::SPELL_ICON_SIZE),
        conf::SPELL_ICON_COLUMNS,
        conf::SPELL_ICON_ROWS,
        None,
        None,
    ));
    commands.insert_resource(ActionBarAssets {
        spells: asset_server.load("ui/spells.png"),
        spell_layout,
        item_layouts: HashMap::new(),
    });
}

pub fn spawn_action_bar(commands: &mut Commands, ui_assets: &GameUiAssets) -> Entity {
    commands
        .spawn((
            ActionBarRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(conf::HEIGHT),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(conf::SLOT_GAP),
                padding: UiRect::all(Val::Px(conf::PADDING)),
                border: UiRect::all(Val::Px(conf::BORDER)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor {
                top: ui_colors::LIGHT_BORDER_COLOR.into(),
                right: ui_colors::DARK_BORDER_COLOR.into(),
                bottom: ui_colors::DARK_BORDER_COLOR.into(),
                left: ui_colors::LIGHT_BORDER_COLOR.into(),
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
        .id()
}

fn spawn_cell(commands: &mut Commands, index: u16) -> Entity {
    commands
        .spawn((
            ActionSlotCell(index),
            Node {
                width: Val::Px(conf::SLOT_SIZE),
                height: Val::Px(conf::SLOT_SIZE),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(dialog::FIELD_BG_COLOR.into()),
        ))
        .observe(
            move |mut click: On<Pointer<Click>>, mut commands: Commands| {
                click.propagate(false);
                match click.button {
                    PointerButton::Primary => commands.trigger(ActionSlotActivated { slot: index }),
                    PointerButton::Secondary => commands.trigger(OpenSlotMenu {
                        slot: index,
                        at: click.pointer_location.position,
                    }),
                    PointerButton::Middle => {}
                }
            },
        )
        .id()
}

pub(super) fn update_slot_count(
    mut commands: Commands,
    root_q: Query<(Entity, &ComputedNode), (With<ActionBarRoot>, Changed<ComputedNode>)>,
    cells: Query<Entity, With<ActionSlotCell>>,
    mut visible: ResMut<VisibleSlots>,
) {
    let Ok((root, computed)) = root_q.single() else {
        return;
    };
    let content_width = logical_size(computed).x - 2.0 * (conf::PADDING + conf::BORDER);
    let count = visible_slot_count(content_width);
    if count == visible.0 {
        return;
    }
    for cell in &cells {
        commands.entity(cell).despawn();
    }
    let new_cells: Vec<Entity> = (0..count)
        .map(|index| spawn_cell(&mut commands, index))
        .collect();
    commands.entity(root).add_children(&new_cells);
    visible.0 = count;
}

pub(super) fn redraw_slots(
    mut commands: Commands,
    bar: Res<ActionBar>,
    book: Res<SpellBook>,
    items: Res<ItemConfigs>,
    appearances: Res<Appearances>,
    mut assets: ResMut<ActionBarAssets>,
    ui_assets: Res<GameUiAssets>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    cells: Query<(Entity, &ActionSlotCell)>,
) {
    for (entity, cell) in &cells {
        let slot = bar.slot(cell.0);
        commands.entity(entity).despawn_children();

        let mut children = Vec::new();
        if let Some(image) = slot.action.and_then(|action| {
            action_image(
                &action,
                &book,
                &items,
                &appearances,
                &mut assets,
                &mut layouts,
            )
        }) {
            children.push(commands.spawn(icon_bundle(image)).id());
        }
        if let Some(tracks) = slot
            .action
            .and_then(|action| cooldown_overlay_for(action, &book))
        {
            children.push(spawn_cooldown_overlay(
                &mut commands,
                tracks,
                Some(&ui_assets.font),
            ));
        }
        if let Some(hotkey) = slot.hotkey {
            children.push(commands.spawn(hotkey_label(&hotkey, &ui_assets)).id());
        }
        commands.entity(entity).add_children(&children);
    }
}

pub(super) fn icon_bundle(image: ImageNode) -> impl Bundle {
    (
        image,
        Node {
            width: Val::Px(UI_ITEM_SIZE),
            height: Val::Px(UI_ITEM_SIZE),
            ..default()
        },
        Pickable::IGNORE,
    )
}

fn hotkey_label(hotkey: &Hotkey, ui_assets: &GameUiAssets) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(0.0),
            right: Val::Px(1.0),
            ..default()
        },
        Text::new(hotkey.short_label()),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: conf::HOTKEY_FONT_SIZE,
            ..default()
        }
        .with_font_smoothing(FontSmoothing::None),
        TextColor(Color::WHITE),
        TextOutline {
            width: 1.0,
            ..default()
        },
        Pickable::IGNORE,
    )
}

pub(super) fn action_image(
    action: &SlotAction,
    book: &SpellBook,
    items: &ItemConfigs,
    appearances: &Appearances,
    assets: &mut ActionBarAssets,
    layouts: &mut Assets<TextureAtlasLayout>,
) -> Option<ImageNode> {
    match action {
        SlotAction::Spell { id, .. } => spell_image(book.get(*id)?.icon, assets),
        SlotAction::Item { item_id, .. } => {
            static_item_image(*item_id, items, appearances, assets, layouts)
        }
    }
}

fn cooldown_overlay_for(action: SlotAction, book: &SpellBook) -> Option<CooldownOverlay> {
    match action {
        SlotAction::Spell { id, .. } => Some(CooldownOverlay::Spell {
            id,
            group: book.get(id)?.group,
        }),
        SlotAction::Item { .. } => None,
    }
}

pub(super) fn spell_image(icon: u16, assets: &ActionBarAssets) -> Option<ImageNode> {
    Some(ImageNode::from_atlas_image(
        assets.spells.clone(),
        TextureAtlas {
            layout: assets.spell_layout.clone(),
            index: spell_icon_index(icon)?,
        },
    ))
}

/// The item's first frame, in the pattern its amount of one asks for; never animated.
fn static_item_image(
    item_id: ItemId,
    items: &ItemConfigs,
    appearances: &Appearances,
    assets: &mut ActionBarAssets,
    layouts: &mut Assets<TextureAtlasLayout>,
) -> Option<ImageNode> {
    let config = items.items.get(&item_id)?;
    let sprite = appearances.get_item(item_id);
    let sheet = appearances.get_sheet(&sprite.group);
    let layout = assets.item_layout(sheet, layouts);
    let (pattern_x, pattern_y, pattern_z) = Item::new(Arc::clone(config), 1)
        .intrinsic_patterns(&sprite)
        .unwrap_or((0, 0, 0));
    let first_frame = SpriteAnimator::new(Arc::clone(&sprite), pattern_x, pattern_y, pattern_z)
        .current_sprite_ids[0];
    Some(ImageNode::from_atlas_image(
        sheet.texture().clone(),
        TextureAtlas {
            layout,
            index: first_frame as usize,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache is what keeps a redraw from adding a layout per slot, and nothing else says so.
    #[test]
    fn a_sheet_is_laid_out_once_and_reused() {
        let mut layouts = Assets::<TextureAtlasLayout>::default();
        let mut assets = ActionBarAssets {
            spells: Handle::default(),
            spell_layout: Handle::default(),
            item_layouts: HashMap::new(),
        };
        let items =
            SpriteSheet::for_test("item-32-32-0.png", Vec2::new(12.0, 12.0), Vec2::splat(32.0));
        let outfits = SpriteSheet::for_test(
            "outfit-4-3-2-2-2-1.png",
            Vec2::new(8.0, 8.0),
            Vec2::splat(64.0),
        );

        let first = assets.item_layout(&items, &mut layouts);
        let again = assets.item_layout(&items, &mut layouts);
        let other = assets.item_layout(&outfits, &mut layouts);

        assert_eq!(first, again, "the same sheet must reuse its layout");
        assert_ne!(first, other, "a different sheet must get its own");
        assert_eq!(layouts.len(), 2);
    }

    #[test]
    fn as_many_slots_as_fit_are_shown() {
        assert_eq!(visible_slot_count(358.0), 10);
        assert_eq!(visible_slot_count(357.0), 9);
        assert_eq!(visible_slot_count(34.0), 1);
        assert_eq!(visible_slot_count(0.0), 0);
        assert_eq!(visible_slot_count(-20.0), 0);
    }

    #[test]
    fn an_icon_cell_is_one_based_and_bounded_by_the_sheet() {
        assert_eq!(spell_icon_index(1), Some(0));
        assert_eq!(spell_icon_index(44), Some(43));
        assert_eq!(spell_icon_index(132), Some(131));
        assert_eq!(spell_icon_index(133), None);
        assert_eq!(spell_icon_index(0), None);
    }

    #[test]
    fn only_a_spell_the_book_knows_is_overlaid() {
        use crate::core::{SpellGroup, SpellId, SpellInfo};

        let book = SpellBook::new(vec![SpellInfo {
            id: SpellId(2),
            name: "Fire Wave".to_owned(),
            words: "exevo flam hur".to_owned(),
            level: 18,
            icon: 44,
            aimable: false,
            group: SpellGroup::Attack,
        }]);

        assert_eq!(
            cooldown_overlay_for(
                SlotAction::Spell {
                    id: SpellId(2),
                    aim: None
                },
                &book
            ),
            Some(CooldownOverlay::Spell {
                id: SpellId(2),
                group: SpellGroup::Attack
            })
        );
        assert_eq!(
            cooldown_overlay_for(
                SlotAction::Spell {
                    id: SpellId(9),
                    aim: None
                },
                &book
            ),
            None
        );
        assert_eq!(
            cooldown_overlay_for(
                SlotAction::Item {
                    item_id: ItemId(3031),
                    aim: None
                },
                &book
            ),
            None
        );
    }
}
