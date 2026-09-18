use std::sync::Arc;

use bevy::{camera::visibility::RenderLayers, prelude::*, text::FontSmoothing};
use bevy_text_outline::TextOutline;

use crate::{
    conf::ui::{ITEM_COUNT_FONT_SIZE, UI_ITEM_SIZE, z_index::DRAGGED_ITEM_UI_Z},
    core::{Appearances, SpriteAnimator},
    game_ui::GameUiAssets,
    items::{Item, ItemDragEnded, ItemDragStarted, ItemPlacement, instancing::ItemState},
    player::MouseHoverState,
};

#[derive(Component, Debug)]
#[allow(dead_code)]
pub struct UiItem {
    pub item: Arc<Item>,
}

#[derive(Component)]
pub struct UiItemDragging {
    origin: ItemPlacement,
}

/// The number drawn over a stack, or `None` when the item has no count to show.
/// See [`Item::is_countable_stack`] for why a fluid container has none.
pub fn stack_count_text(item: &Item) -> Option<String> {
    item.is_countable_stack().then(|| item.amount.to_string())
}

/// The count label itself: bottom right of the ITEM node (32 px), not of the
/// slot around it (36 px).
///
/// `Pickable::IGNORE` is load-bearing -- without it the label sits over the
/// sprite and can take a `DragStart`, or a slot's `Over`/`Out`, away from the
/// node beneath it.
fn stack_count_label(item: &Item, ui_assets: &GameUiAssets) -> Option<impl Bundle> {
    let count = stack_count_text(item)?;
    Some((
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            ..default()
        },
        Text::new(count),
        TextFont {
            font: ui_assets.font.clone(),
            font_size: ITEM_COUNT_FONT_SIZE,
            ..default()
        }
        .with_font_smoothing(FontSmoothing::AntiAliased),
        TextColor(Color::WHITE),
        TextOutline {
            width: 1.0,
            ..default()
        },
        Pickable::IGNORE,
    ))
}

pub fn spawn_ui_item(
    item: &Arc<Item>,
    appearances: &Appearances,
    texture_atlases: &mut Assets<TextureAtlasLayout>,
    ui_assets: &GameUiAssets,
    position: &Vec2,
) -> impl Bundle {
    let config = appearances.get_item(item.config.id);
    let sheet = appearances.get_sheet(&config.group);
    let texture_atlas = TextureAtlasLayout::from_grid(
        sheet.sprite_size.as_uvec2(),
        sheet.grid_size.x as u32,
        sheet.grid_size.y as u32,
        None,
        None,
    );
    let texture_atlas_handle = texture_atlases.add(texture_atlas);
    let mut atlas = TextureAtlas::from(texture_atlas_handle);

    // A UI item has an amount but no position, so it takes the intrinsic
    // pattern -- a stack's count tier, a fluid's colour -- and falls back to the
    // first cell for everything else. Hardcoding (0, 0, 0) here is what made a
    // stack of 50 gold draw the "1" sprite in the inventory while drawing
    // correctly on the ground.
    let (pattern_x, pattern_y, pattern_z) = item.intrinsic_patterns(&config).unwrap_or((0, 0, 0));
    let animator = SpriteAnimator::new(Arc::clone(&config), pattern_x, pattern_y, pattern_z);
    atlas.index = animator.current_sprite_ids[0] as usize;

    (
        UiItem { item: item.clone() },
        animator,
        Node {
            width: Val::Px(UI_ITEM_SIZE),
            height: Val::Px(UI_ITEM_SIZE),
            ..default()
        },
        ImageNode::from_atlas_image(sheet.texture().clone(), atlas),
        Transform::from_xyz(position.x, position.y, 0.0),
        RenderLayers::layer(1),
        // An item with no count spawns no child at all; the `Option` is the
        // whole conditional.
        Children::spawn(SpawnIter(stack_count_label(item, ui_assets).into_iter())),
    )
}

pub fn animate_ui_items(
    mut query: Query<(&SpriteAnimator, &mut ImageNode), Changed<SpriteAnimator>>,
) {
    for (animator, mut image_node) in &mut query {
        if let Some(atlas) = &mut image_node.texture_atlas {
            atlas.index = animator.current_sprite_ids[0] as usize;
        }
    }
}

pub fn item_drag_started(
    event: On<ItemDragStarted>,
    mut commands: Commands,
    appearances: Res<Appearances>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    ui_assets: Res<GameUiAssets>,
    hover_state: Res<MouseHoverState>,
    state: Res<ItemState>,
    stack_item_q: Query<&Children>,
    drag_item_q: Query<Entity, With<UiItemDragging>>,
) {
    for e in drag_item_q {
        commands.entity(e).despawn();
    }

    if let ItemPlacement::Map { position, index } = &event.origin {
        let Some(stack_entity) = state.occupied_tiles.get(position) else {
            return;
        };
        let Ok(stack_items) = stack_item_q.get(*stack_entity) else {
            return;
        };
        let Some(item_entity) = stack_items.get(*index) else {
            return;
        };

        commands.entity(*item_entity).insert(Visibility::Hidden);
    }

    commands
        .spawn(spawn_ui_item(
            &event.item,
            &appearances,
            &mut texture_atlases,
            &ui_assets,
            &hover_state.screen_position,
        ))
        .insert((
            UiItemDragging {
                origin: event.origin.clone(),
            },
            ZIndex(DRAGGED_ITEM_UI_Z),
        ));
}

pub fn item_drag_ended(
    _: On<ItemDragEnded>,
    mut commands: Commands,
    state: Res<ItemState>,
    stack_item_q: Query<&Children>,
    drag_item_q: Query<(Entity, &UiItemDragging)>,
) {
    let Ok((entity, drag_item)) = drag_item_q.single() else {
        return;
    };
    if let ItemPlacement::Map { position, index } = &drag_item.origin {
        let Some(stack_entity) = state.occupied_tiles.get(position) else {
            return;
        };
        let Ok(stack_items) = stack_item_q.get(*stack_entity) else {
            return;
        };
        let Some(item) = stack_items.get(*index) else {
            return;
        };
        commands.entity(*item).insert(Visibility::Visible);
    };
    commands.entity(entity).despawn();
}

pub fn move_dragged_item(
    ui_item_q: Query<&mut UiTransform, With<UiItemDragging>>,
    hover_state: Res<MouseHoverState>,
) {
    for mut item_transform in ui_item_q {
        item_transform.translation = Val2::new(
            Val::Px(hover_state.screen_position.x),
            Val::Px(hover_state.screen_position.y),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::ItemId;
    use crate::items::{ItemConfig, ItemFlag};

    fn item(flags: Vec<ItemFlag>, amount: u32) -> Item {
        Item::new(
            Arc::new(ItemConfig {
                id: ItemId(2148),
                flags,
                friction: None,
                slot: None,
                minimap_color: None,
                elevation: None,
            }),
            amount,
        )
    }

    /// The label renders the amount, not the count *tier* the sprite shows: a
    /// stack of 50 and a stack of 99 draw the same sprite and must not draw the
    /// same number.
    #[test]
    fn a_stack_is_labelled_with_its_amount() {
        assert_eq!(
            stack_count_text(&item(vec![ItemFlag::Cumulative], 50)),
            Some("50".to_string())
        );
        assert_eq!(
            stack_count_text(&item(vec![ItemFlag::Cumulative], 99)),
            Some("99".to_string())
        );
    }

    /// Delegated to `Item::is_countable_stack`, which is tested against every
    /// flag in `item.rs`. Pinned here too because this is the caller that turns
    /// a `false` into "draw nothing".
    #[test]
    fn a_single_item_and_a_fluid_carry_no_label() {
        assert_eq!(stack_count_text(&item(vec![ItemFlag::Cumulative], 1)), None);
        assert_eq!(
            stack_count_text(&item(vec![ItemFlag::LiquidContainer], 5)),
            None
        );
    }
}
