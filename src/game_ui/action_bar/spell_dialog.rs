use bevy::input::mouse::MouseScrollUnit;
use bevy::picking::events::Scroll;
use bevy::prelude::*;

use crate::conf::ui::{UI_ITEM_SIZE, action_bar as bar_conf, dialog, ui_colors};
use crate::core::{SpellBook, SpellId, SpellInfo};
use crate::game_ui::{
    DialogButton, DialogButtonId, DialogButtonPressed, GameUiAssets, ModalDialog, ModalOrder,
};

use super::bar::{ActionBarAssets, icon_bundle, spell_image};
use super::state::{ActionBar, SlotAction};
use super::target_dialog::{OpenTargetDialog, PendingAssignment};

#[derive(Event, Debug, Clone, Copy)]
pub struct OpenAssignSpellDialog {
    pub slot: u16,
}

#[derive(Component, Debug)]
pub(super) struct AssignSpellDialog {
    slot: u16,
    selected: Option<SpellId>,
    last_click: Option<(SpellId, f32)>,
}

#[derive(Component)]
pub(super) struct SpellRow(SpellId);

#[derive(Debug, PartialEq, Eq)]
pub enum SpellChoice {
    Assign(SlotAction),
    ChooseAim(PendingAssignment),
    Nothing,
}

pub fn choice_for(selected: Option<SpellId>, book: &SpellBook) -> SpellChoice {
    let Some(spell) = selected.and_then(|id| book.get(id)) else {
        return SpellChoice::Nothing;
    };
    if spell.aimable {
        SpellChoice::ChooseAim(PendingAssignment::Spell(spell.id))
    } else {
        SpellChoice::Assign(SlotAction::Spell {
            id: spell.id,
            aim: None,
        })
    }
}

pub(super) fn on_open_assign_spell_dialog(
    event: On<OpenAssignSpellDialog>,
    mut commands: Commands,
    ui_assets: Res<GameUiAssets>,
    mut order: ResMut<ModalOrder>,
    existing: Query<(), With<AssignSpellDialog>>,
    book: Res<SpellBook>,
    assets: Res<ActionBarAssets>,
) {
    if !existing.is_empty() {
        return;
    }

    let handle = ModalDialog::new(format!("Assign Spell to Action Button {}", event.slot + 1))
        .with_buttons([DialogButton::ok(), DialogButton::cancel()])
        .spawn(&mut commands, &ui_assets, &mut order);
    let dialog = handle.root;
    commands.entity(dialog).insert(AssignSpellDialog {
        slot: event.slot,
        selected: None,
        last_click: None,
    });

    let spells = book.spells().unwrap_or_default();
    if spells.is_empty() {
        let empty = commands
            .spawn((
                Text::new("No spells available."),
                TextFont {
                    font: ui_assets.font.clone(),
                    font_size: 11.0,
                    ..default()
                },
                TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            ))
            .id();
        commands.entity(handle.content).add_child(empty);
        return;
    }

    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                max_height: Val::Px(bar_conf::SPELL_LIST_MAX_HEIGHT),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor {
                top: ui_colors::DARK_BORDER_COLOR.into(),
                right: ui_colors::LIGHT_BORDER_COLOR.into(),
                bottom: ui_colors::LIGHT_BORDER_COLOR.into(),
                left: ui_colors::DARK_BORDER_COLOR.into(),
            },
            BackgroundColor(dialog::FIELD_BG_COLOR.into()),
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
                MouseScrollUnit::Line => scroll.y * bar_conf::SPELL_ROW_HEIGHT,
                MouseScrollUnit::Pixel => scroll.y,
            };
            position.y = (position.y - delta).clamp(0.0, max);
        },
    );

    for spell in spells {
        let row = spawn_spell_row(&mut commands, &ui_assets, &assets, dialog, spell);
        commands.entity(list).add_child(row);
    }
    commands.entity(handle.content).add_child(list);
}

fn spawn_spell_row(
    commands: &mut Commands,
    ui_assets: &GameUiAssets,
    assets: &ActionBarAssets,
    dialog: Entity,
    spell: &SpellInfo,
) -> Entity {
    let icon = match spell_image(spell.icon, assets) {
        Some(image) => commands.spawn(icon_bundle(image)).id(),
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
    };
    let name = commands
        .spawn((
            Text::new(spell.name.clone()),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 11.0,
                ..default()
            },
            TextColor(Color::WHITE),
            Pickable::IGNORE,
        ))
        .id();
    let detail = commands
        .spawn((
            Text::new(format!("{} · level {}", spell.words, spell.level)),
            TextFont {
                font: ui_assets.font.clone(),
                font_size: 10.0,
                ..default()
            },
            TextColor(ui_colors::FONT_COLOR_CONTENT.into()),
            Pickable::IGNORE,
        ))
        .id();
    let text = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .add_children(&[name, detail])
        .id();

    let spell_id = spell.id;
    commands
        .spawn((
            SpellRow(spell_id),
            Node {
                height: Val::Px(bar_conf::SPELL_ROW_HEIGHT),
                flex_shrink: 0.0,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .add_children(&[icon, text])
        .observe(
            move |click: On<Pointer<Click>>,
                  mut dialogs: Query<&mut AssignSpellDialog>,
                  time: Res<Time>,
                  mut commands: Commands| {
                if click.button != PointerButton::Primary {
                    return;
                }
                let Ok(mut state) = dialogs.get_mut(dialog) else {
                    return;
                };
                let now = time.elapsed_secs();
                let is_double = matches!(
                    state.last_click,
                    Some((id, at)) if id == spell_id && now - at < dialog::DOUBLE_CLICK_SECS
                );
                state.selected = Some(spell_id);
                if is_double {
                    state.last_click = None;
                    commands.trigger(DialogButtonPressed {
                        dialog,
                        button: DialogButtonId::Ok,
                    });
                } else {
                    state.last_click = Some((spell_id, now));
                }
            },
        )
        .id()
}

pub(super) fn update_spell_row_highlight(
    dialogs: Query<&AssignSpellDialog, Changed<AssignSpellDialog>>,
    mut rows: Query<(&SpellRow, &mut BackgroundColor)>,
) {
    let Ok(dialog) = dialogs.single() else {
        return;
    };
    for (row, mut color) in &mut rows {
        *color = if Some(row.0) == dialog.selected {
            BackgroundColor(dialog::ROW_SELECTED_COLOR.into())
        } else {
            BackgroundColor(Color::NONE)
        };
    }
}

pub(super) fn on_assign_spell_button(
    event: On<DialogButtonPressed>,
    mut commands: Commands,
    dialogs: Query<&AssignSpellDialog>,
    book: Res<SpellBook>,
    mut bar: ResMut<ActionBar>,
) {
    let Ok(dialog) = dialogs.get(event.dialog) else {
        return;
    };
    if event.button == DialogButtonId::Ok {
        match choice_for(dialog.selected, &book) {
            SpellChoice::Nothing => return,
            SpellChoice::Assign(action) => bar.set_action(dialog.slot, action),
            SpellChoice::ChooseAim(pending) => commands.trigger(OpenTargetDialog {
                slot: dialog.slot,
                pending,
            }),
        }
    }
    commands.entity(event.dialog).despawn();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_spell(id: u16, aimable: bool) -> SpellInfo {
        SpellInfo {
            id: SpellId(id),
            name: format!("Spell {id}"),
            words: format!("words {id}"),
            level: 8,
            icon: 6,
            aimable,
        }
    }

    fn a_book() -> SpellBook {
        SpellBook::new(vec![a_spell(1, false), a_spell(7, true)])
    }

    #[test]
    fn a_spell_cast_without_a_target_is_assigned_straight_away() {
        assert_eq!(
            choice_for(Some(SpellId(1)), &a_book()),
            SpellChoice::Assign(SlotAction::Spell {
                id: SpellId(1),
                aim: None
            })
        );
    }

    #[test]
    fn an_aimable_spell_asks_for_an_aim() {
        assert_eq!(
            choice_for(Some(SpellId(7)), &a_book()),
            SpellChoice::ChooseAim(PendingAssignment::Spell(SpellId(7)))
        );
    }

    #[test]
    fn nothing_selected_or_nothing_known_chooses_nothing() {
        assert_eq!(choice_for(None, &a_book()), SpellChoice::Nothing);
        assert_eq!(
            choice_for(Some(SpellId(99)), &a_book()),
            SpellChoice::Nothing
        );
        assert_eq!(
            choice_for(Some(SpellId(1)), &SpellBook::default()),
            SpellChoice::Nothing
        );
    }

    fn a_world_with_an_open_dialog(selected: Option<SpellId>) -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<ActionBar>();
        world.insert_resource(a_book());
        world.add_observer(on_assign_spell_button);
        let dialog = world
            .spawn(AssignSpellDialog {
                slot: 2,
                selected,
                last_click: None,
            })
            .id();
        (world, dialog)
    }

    #[test]
    fn ok_with_nothing_selected_keeps_the_dialog_open() {
        let (mut world, dialog) = a_world_with_an_open_dialog(None);

        world.trigger(DialogButtonPressed {
            dialog,
            button: DialogButtonId::Ok,
        });
        world.flush();

        assert!(world.get_entity(dialog).is_ok());
    }

    #[test]
    fn ok_assigns_the_selected_spell_and_closes() {
        let (mut world, dialog) = a_world_with_an_open_dialog(Some(SpellId(1)));

        world.trigger(DialogButtonPressed {
            dialog,
            button: DialogButtonId::Ok,
        });
        world.flush();

        assert!(world.resource::<ActionBar>().slot(2).action.is_some());
        assert!(world.get_entity(dialog).is_err());
    }

    /// An aimable spell assigns nothing here: it hands the choice to the aim modal, which assigns
    /// once an aim is picked.
    #[test]
    fn ok_on_an_aimable_spell_hands_off_to_the_aim_modal() {
        #[derive(Resource, Default)]
        struct Seen(Vec<PendingAssignment>);

        let (mut world, dialog) = a_world_with_an_open_dialog(Some(SpellId(7)));
        world.init_resource::<Seen>();
        world.add_observer(|event: On<OpenTargetDialog>, mut seen: ResMut<Seen>| {
            seen.0.push(event.pending)
        });

        world.trigger(DialogButtonPressed {
            dialog,
            button: DialogButtonId::Ok,
        });
        world.flush();

        assert_eq!(
            world.resource::<Seen>().0,
            vec![PendingAssignment::Spell(SpellId(7))]
        );
        assert!(world.resource::<ActionBar>().slot(2).action.is_none());
        assert!(world.get_entity(dialog).is_err());
    }
}
