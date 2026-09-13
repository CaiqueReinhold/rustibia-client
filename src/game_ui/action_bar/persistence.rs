use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::conf::paths::data_dir;
use crate::core::{ActiveCharacter, ItemConfigs, SpellBook};

use super::state::{ActionBar, ActionSlot, SlotAction};

const FILE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct ActionBarFile {
    version: u32,
    slots: BTreeMap<u16, ActionSlot>,
}

#[derive(Error, Debug)]
pub enum LoadError {
    #[error("malformed: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("version {0}, and this client reads version {FILE_VERSION}")]
    Version(u32),
}

pub fn file_path(character_id: u32) -> PathBuf {
    data_dir()
        .join("actionbars")
        .join(format!("{character_id}.json"))
}

pub fn parse(contents: &str) -> Result<BTreeMap<u16, ActionSlot>, LoadError> {
    let file: ActionBarFile = serde_json::from_str(contents)?;
    if file.version != FILE_VERSION {
        return Err(LoadError::Version(file.version));
    }
    Ok(file.slots)
}

pub fn serialize(slots: &BTreeMap<u16, ActionSlot>) -> String {
    serde_json::to_string_pretty(&ActionBarFile {
        version: FILE_VERSION,
        slots: slots.clone(),
    })
    .expect("action bar slots always serialise")
}

/// Never fails: a missing file is an empty bar, and an unreadable one is logged and left on disk.
pub fn read_slots(path: &Path) -> BTreeMap<u16, ActionSlot> {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return BTreeMap::new(),
        Err(error) => {
            warn!("action bar: cannot read {path:?}: {error}");
            return BTreeMap::new();
        }
    };
    parse(&contents).unwrap_or_else(|error| {
        warn!("action bar: ignoring {path:?}: {error}");
        BTreeMap::new()
    })
}

pub(super) fn load_action_bar(
    mut commands: Commands,
    character: Option<Res<ActiveCharacter>>,
    items: Res<ItemConfigs>,
) {
    let slots = character
        .map(|character| read_slots(&file_path(character.id)))
        .unwrap_or_default();
    let mut bar = ActionBar::from_slots(slots);
    let cleared = bar.retain_actions(|action| match action {
        SlotAction::Item { item_id, .. } => items.items.contains_key(item_id),
        SlotAction::Spell { .. } => true,
    });
    for index in cleared {
        info!("action bar: cleared slot {index}, its item is not in the catalogue");
    }
    commands.insert_resource(bar);
}

pub(super) fn prune_unknown_spells(mut bar: ResMut<ActionBar>, book: Res<SpellBook>) {
    if bar.spells_checked() {
        return;
    }
    let Some(spells) = book.spells() else {
        return;
    };
    let unchanged = bar.bypass_change_detection();
    unchanged.mark_spells_checked();
    let cleared = unchanged.retain_actions(|action| match action {
        SlotAction::Spell { id, .. } => spells.iter().any(|spell| spell.id == *id),
        SlotAction::Item { .. } => true,
    });
    if cleared.is_empty() {
        return;
    }
    for index in cleared {
        info!("action bar: cleared slot {index}, its spell is not in the spell list");
    }
    bar.set_changed();
}

/// Writes through a temporary file and renames it over the destination. A write cut short would
/// otherwise leave a file that loads as an empty bar and is overwritten by the next change.
fn write_slots(path: &Path, contents: &str) {
    if let Some(parent) = path.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        warn!("action bar: failed to create {parent:?}: {error}");
        return;
    }
    let staged = path.with_extension("tmp");
    if let Err(error) = std::fs::write(&staged, contents) {
        warn!("action bar: failed to write {staged:?}: {error}");
        return;
    }
    if let Err(error) = std::fs::rename(&staged, path) {
        warn!("action bar: failed to replace {path:?}: {error}");
    }
}

/// Writes the bar if anything changed since the last write. Synchronous: two writes racing on a
/// task pool could land in either order, leaving the older bar on disk.
pub(super) fn flush_action_bar(bar: &mut ActionBar, character: Option<&ActiveCharacter>) {
    let Some(character) = character else {
        return;
    };
    if !bar.take_dirty() {
        return;
    }
    write_slots(&file_path(character.id), &serialize(bar.slots()));
}

pub(super) fn save_action_bar(mut bar: ResMut<ActionBar>, character: Option<Res<ActiveCharacter>>) {
    flush_action_bar(bar.bypass_change_detection(), character.as_deref());
}

#[cfg(test)]
mod tests {
    use super::super::state::Aim;
    use super::*;
    use crate::core::{SpellGroup, SpellId, SpellInfo};
    use crate::items::ItemId;
    use crate::player::Hotkey;
    use bevy::ecs::system::RunSystemOnce;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rustibia-action-bar-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn slots_survive_a_round_trip() {
        let slots = BTreeMap::from([
            (
                0,
                ActionSlot {
                    action: Some(SlotAction::Spell {
                        id: SpellId(1),
                        aim: None,
                    }),
                    hotkey: Some(Hotkey::plain(KeyCode::F1)),
                },
            ),
            (
                7,
                ActionSlot {
                    action: Some(SlotAction::Item {
                        item_id: ItemId(266),
                        aim: Some(Aim::Target),
                    }),
                    hotkey: None,
                },
            ),
            (
                9,
                ActionSlot {
                    action: None,
                    hotkey: Some(Hotkey::new(KeyCode::Digit2, true, false, false)),
                },
            ),
        ]);

        assert_eq!(parse(&serialize(&slots)).unwrap(), slots);
    }

    #[test]
    fn malformed_contents_and_an_unknown_version_are_refused() {
        assert!(matches!(parse("{ not json"), Err(LoadError::Parse(_))));
        assert!(matches!(
            parse(r#"{"version":2,"slots":{}}"#),
            Err(LoadError::Version(2))
        ));
    }

    #[test]
    fn a_missing_or_corrupt_file_loads_empty_and_is_left_alone() {
        let dir = scratch_dir("read");
        assert!(read_slots(&dir.join("missing.json")).is_empty());

        let corrupt = dir.join("corrupt.json");
        std::fs::write(&corrupt, "{ not json").unwrap();
        assert!(read_slots(&corrupt).is_empty());
        assert_eq!(std::fs::read_to_string(&corrupt).unwrap(), "{ not json");
    }

    fn a_bar_with_spell(id: u16) -> ActionBar {
        let mut bar = ActionBar::default();
        bar.set_action(
            0,
            SlotAction::Spell {
                id: SpellId(id),
                aim: None,
            },
        );
        bar.take_dirty();
        bar
    }

    #[test]
    fn a_spell_is_not_cleared_before_the_list_arrives() {
        let mut world = World::new();
        world.insert_resource(a_bar_with_spell(9));
        world.init_resource::<SpellBook>();

        world.run_system_once(prune_unknown_spells).unwrap();

        assert!(!world.resource::<ActionBar>().slot(0).is_empty());
    }

    #[test]
    fn a_spell_missing_from_the_list_is_cleared_once_it_arrives() {
        let mut world = World::new();
        world.insert_resource(a_bar_with_spell(9));
        world.insert_resource(SpellBook::new(vec![SpellInfo {
            id: SpellId(1),
            name: "Light Healing".to_owned(),
            words: "exura".to_owned(),
            level: 8,
            icon: 6,
            aimable: false,
            group: SpellGroup::Healing,
        }]));

        world.run_system_once(prune_unknown_spells).unwrap();

        let bar = world.resource::<ActionBar>();
        assert!(bar.slot(0).is_empty());
        assert!(bar.spells_checked());
    }

    #[test]
    fn a_write_lands_whole_and_leaves_no_staging_file() {
        let dir = scratch_dir("write");
        let path = dir.join("7.json");
        let mut bar = a_bar_with_spell(1);
        bar.set_action(
            1,
            SlotAction::Item {
                item_id: ItemId(266),
                aim: None,
            },
        );

        write_slots(&path, &serialize(bar.slots()));

        assert_eq!(read_slots(&path), *bar.slots());
        assert!(!path.with_extension("tmp").exists());
    }

    #[test]
    fn a_flush_without_a_character_keeps_the_change_pending() {
        let mut bar = a_bar_with_spell(1);
        bar.set_action(
            2,
            SlotAction::Spell {
                id: SpellId(4),
                aim: None,
            },
        );

        flush_action_bar(&mut bar, None);

        assert!(
            bar.take_dirty(),
            "a change with nowhere to go is not a saved change"
        );
    }
}
