//! The "Move Objects" dialog: how much of a stack to move.
//!
//! Opened by a Ctrl-drop on a countable stack (`gestures::on_drag_end`), which
//! has already despawned the drag ghost and validated the destination. This owns
//! only the amount; the move itself goes back out as an
//! [`InteractionIntent::MoveItem`], so the walk-into-reach deferral and the
//! wire encoding stay where they already live.

use bevy::prelude::*;
use bevy::ui_widgets::{
    Slider, SliderPrecision, SliderRange, SliderStep, SliderThumb, SliderValue, TrackClick,
    ValueChange,
};

use crate::conf::ui::{dialog as conf, ui_colors};
use crate::game_ui::scaling::logical_size;
use crate::game_ui::{
    DialogButton, DialogButtonId, DialogButtonPressed, GameUiAssets, ModalDialog, ModalOrder,
};
use crate::items::{Item, ItemId, ItemPlacement, OpenSplitDialog};
use crate::player::InteractionIntent;

const TRACK_HEIGHT: f32 = 12.0;
const THUMB_WIDTH: f32 = 10.0;

/// Everything the move needs except the amount, which is read off the slider at
/// Ok time.
#[derive(Component)]
pub struct SplitDialog {
    origin: ItemPlacement,
    item_id: ItemId,
    to: ItemPlacement,
    /// The whole stack, capped to what the wire can name.
    max: u8,
}

/// Marks the "1" .. "n" readout so the slider system can rewrite it.
#[derive(Component)]
pub(super) struct SplitAmountText;

/// The slider whose value this dialog will send. A `SliderValue` alone would
/// also match the scrollbars, which are a different widget on the same events.
#[derive(Component)]
pub(super) struct SplitSlider;

/// The amount a slider position means, on the wire's terms.
///
/// The slider carries an `f32` and the wire carries a `u8`, so the conversion
/// gets a name and a test rather than being an `as` cast inside an observer: it
/// rounds rather than truncates, and clamps into `1..=max` so no rounding
/// artifact can send a move of zero.
pub fn slider_amount(value: f32, max: u8) -> u8 {
    if !value.is_finite() {
        return max.max(1);
    }
    (value.round().clamp(1.0, max.max(1) as f32)) as u8
}

/// The largest amount of `item` that fits on the wire.
///
/// `Item::amount` is a `u32` and `ClientMessage::MoveItem::amount` is a `u8`, so
/// a stack the server could not have sent still cannot overflow into a smaller
/// move than the player asked for.
fn wire_amount(item: &Item) -> u8 {
    item.amount.min(u8::MAX as u32) as u8
}

pub fn on_open_split_dialog(
    event: On<OpenSplitDialog>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    existing: Query<Entity, With<SplitDialog>>,
) {
    // One at a time. A second Ctrl-drop while the first is open would otherwise
    // stack two dialogs whose Ok buttons both fire.
    if !existing.is_empty() {
        return;
    }

    let max = wire_amount(&event.item);
    let handle = ModalDialog::new("Move Objects")
        .with_buttons([DialogButton::ok(), DialogButton::cancel()])
        .spawn(&mut commands, &ui_assets, &mut order);
    commands.entity(handle.root).insert(SplitDialog {
        origin: event.origin.clone(),
        item_id: event.item.config.id,
        to: event.to.clone(),
        max,
    });

    let caption = commands
        .spawn((
            Text::new(format!("Move how many? (1-{max})")),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
        ))
        .id();

    // The slider defaults to the full stack, so Ok never moves less than the
    // plain drag the player could have done instead.
    let slider = commands
        .spawn((
            SplitSlider,
            // `Snap`: a click on the track is the amount clicked. The default,
            // `Drag`, ignores a track click entirely, which reads as a dead
            // control on a bar this short.
            Slider {
                track_click: TrackClick::Snap,
            },
            SliderValue(max as f32),
            SliderRange::new(1.0, max as f32),
            SliderStep(1.0),
            SliderPrecision(0),
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(TRACK_HEIGHT),
                margin: UiRect::vertical(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(conf::FIELD_BG_COLOR.into()),
            Children::spawn(Spawn((
                SliderThumb,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(THUMB_WIDTH),
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(conf::BUTTON_COLOR.into()),
                BorderColor {
                    top: ui_colors::LIGHT_BORDER_COLOR.into(),
                    right: ui_colors::DARK_BORDER_COLOR.into(),
                    bottom: ui_colors::DARK_BORDER_COLOR.into(),
                    left: ui_colors::LIGHT_BORDER_COLOR.into(),
                },
            ))),
        ))
        // The widget REPORTS the new value and does not apply it: `SliderValue`
        // is immutable and nothing in `SliderPlugin` writes it back. Without
        // this the thumb would not move at all.
        .observe(|change: On<ValueChange<f32>>, mut commands: Commands| {
            commands
                .entity(change.source)
                .insert(SliderValue(change.value));
        })
        .id();

    let readout = commands
        .spawn((
            SplitAmountText,
            Text::new(max.to_string()),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextLayout::new_with_justify(Justify::Center),
            TextColor(Color::WHITE),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
        ))
        .id();

    commands
        .entity(handle.content)
        .add_children(&[caption, slider, readout]);
}

/// The widget is headless: it owns the value and nothing else, so the thumb's
/// position and the readout are ours to write.
///
/// Travel is the track minus the thumb, which is the convention the widget's own
/// click and drag maths assumes — anything else and the thumb stops tracking the
/// mouse.
///
/// **Not gated on `Changed<SliderValue>`.** The value is set on the frame the
/// dialog spawns, when the track still measures `(0, 0)`, so a gate would place
/// the thumb once at the far left and never run again. See the vault's
/// `a-slider-thumb-sits-at-one-end-and-never-moves`.
pub fn sync_split_slider(
    slider_q: Query<(&SliderValue, &SliderRange, &ComputedNode, &Children), With<SplitSlider>>,
    mut thumb_q: Query<(&mut Node, &ComputedNode), With<SliderThumb>>,
    mut text_q: Query<&mut Text, With<SplitAmountText>>,
) {
    for (value, range, slider_node, children) in &slider_q {
        for child in children.iter() {
            let Ok((mut thumb, thumb_node)) = thumb_q.get_mut(child) else {
                continue;
            };
            // `Node.left` is logical and `ComputedNode::size` is physical; see
            // `game_ui::scaling`.
            let travel = (logical_size(slider_node).x - logical_size(thumb_node).x).max(0.0);
            let left = Val::Px(range.thumb_position(value.0) * travel);
            if thumb.left != left {
                thumb.left = left;
            }
        }

        let label = value.0.round().to_string();
        for mut text in &mut text_q {
            if text.0 != label {
                text.0 = label.clone();
            }
        }
    }
}

pub fn on_split_dialog_button(
    event: On<DialogButtonPressed>,
    mut commands: Commands,
    dialog_q: Query<&SplitDialog>,
    slider_q: Query<&SliderValue, With<SplitSlider>>,
) {
    let Ok(dialog) = dialog_q.get(event.dialog) else {
        return;
    };

    if event.button == DialogButtonId::Ok
        && let Ok(value) = slider_q.single()
    {
        commands.trigger(InteractionIntent::MoveItem {
            origin: dialog.origin.clone(),
            item_id: dialog.item_id,
            amount: slider_amount(value.0, dialog.max),
            to: dialog.to.clone(),
        });
    }

    commands.entity(event.dialog).despawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemConfig, ItemFlag};
    use std::sync::Arc;

    fn item(flags: Vec<ItemFlag>, amount: u32) -> Item {
        Item::new(
            Arc::new(ItemConfig {
                id: 2148,
                flags,
                friction: None,
                slot: None,
                minimap_color: None,
                elevation: None,
            }),
            amount,
        )
    }

    /// Rounds to the nearest whole item rather than truncating: a thumb resting
    /// a hair under 7 must send 7, not 6.
    #[test]
    fn a_slider_position_rounds_to_whole_items() {
        assert_eq!(slider_amount(6.9, 50), 7);
        assert_eq!(slider_amount(7.4, 50), 7);
        assert_eq!(slider_amount(50.0, 50), 50);
    }

    /// The clamp is what keeps a move of zero off the wire. The server reads
    /// `amount` as "how many to take", and zero would be a request it can only
    /// deny.
    #[test]
    fn a_slider_position_never_leaves_the_range() {
        assert_eq!(slider_amount(0.0, 50), 1);
        assert_eq!(slider_amount(-3.0, 50), 1);
        assert_eq!(slider_amount(99.0, 50), 50);
        assert_eq!(slider_amount(f32::NAN, 50), 50);
    }

    /// A stack larger than the wire can name is capped, not wrapped: `260 as u8`
    /// is 4, which would silently move four gold coins instead of all of them.
    #[test]
    fn a_stack_wider_than_the_wire_is_capped() {
        assert_eq!(wire_amount(&item(vec![ItemFlag::Cumulative], 260)), 255);
        assert_eq!(wire_amount(&item(vec![ItemFlag::Cumulative], 50)), 50);
    }
}
