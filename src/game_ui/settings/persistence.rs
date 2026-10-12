use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::conf::paths::data_dir;
use crate::game_ui::files::write_atomically;

use super::state::Settings;

const FILE_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct SettingsFile {
    version: u32,
    #[serde(flatten)]
    settings: Settings,
}

/// The file's contents as last read or written, so a save writes only what differs.
#[derive(Resource, Debug, Default)]
pub(super) struct WrittenSettings(pub(super) Option<String>);

#[derive(Error, Debug)]
pub enum LoadError {
    #[error("malformed: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("version {0}, and this client reads version {FILE_VERSION}")]
    Version(u32),
}

pub fn file_path() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn parse(contents: &str) -> Result<Settings, LoadError> {
    let file: SettingsFile = serde_json::from_str(contents)?;
    if file.version != FILE_VERSION {
        return Err(LoadError::Version(file.version));
    }
    Ok(file.settings.clamped())
}

pub fn serialize(settings: &Settings) -> String {
    serde_json::to_string_pretty(&SettingsFile {
        version: FILE_VERSION,
        settings: settings.clone(),
    })
    .expect("settings always serialise")
}

/// Never fails: a missing file is the defaults, and an unreadable one is logged and left on disk.
pub fn read_settings(path: &Path) -> Settings {
    let contents = match std::fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == ErrorKind::NotFound => return Settings::default(),
        Err(error) => {
            warn!("settings: cannot read {path:?}: {error}");
            return Settings::default();
        }
    };
    parse(&contents).unwrap_or_else(|error| {
        warn!("settings: ignoring {path:?}: {error}");
        Settings::default()
    })
}

pub(super) fn flush_settings(settings: &Settings, written: &mut WrittenSettings, path: &Path) {
    let contents = serialize(settings);
    if written.0.as_ref() == Some(&contents) {
        return;
    }
    if let Err(error) = write_atomically(path, &contents) {
        warn!("settings: failed to write {path:?}: {error}");
        return;
    }
    written.0 = Some(contents);
}

pub(super) fn save_settings(settings: Res<Settings>, mut written: ResMut<WrittenSettings>) {
    flush_settings(&settings, &mut written, &file_path());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rustibia-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn changed() -> Settings {
        Settings {
            show_fps: true,
            show_ping: false,
            vsync: true,
            show_names: false,
            show_health_bars: false,
            show_damage_numbers: false,
            show_speech: false,
            key_repeat_ms: 120,
        }
    }

    #[test]
    fn settings_round_trip_through_the_file_format() {
        assert_eq!(parse(&serialize(&changed())).unwrap(), changed());
    }

    #[test]
    fn a_field_missing_from_the_file_keeps_the_others() {
        let settings = parse(r#"{ "version": 1, "show_fps": true }"#).unwrap();

        assert_eq!(
            settings,
            Settings {
                show_fps: true,
                ..Settings::default()
            }
        );
    }

    #[test]
    fn another_version_is_refused() {
        assert!(matches!(
            parse(r#"{ "version": 2 }"#),
            Err(LoadError::Version(2))
        ));
    }

    #[test]
    fn a_repeat_interval_outside_the_range_is_clamped() {
        let low = parse(r#"{ "version": 1, "key_repeat_ms": 5 }"#).unwrap();
        let high = parse(r#"{ "version": 1, "key_repeat_ms": 9000 }"#).unwrap();

        assert_eq!(low.key_repeat_ms, 50);
        assert_eq!(high.key_repeat_ms, 500);
    }

    #[test]
    fn an_unusable_file_reads_as_the_defaults_and_stays_on_disk() {
        let dir = scratch_dir("unusable");
        for (name, contents) in [
            ("malformed", "{ not json"),
            ("future", r#"{ "version": 9 }"#),
        ] {
            let path = dir.join(format!("{name}.json"));
            std::fs::write(&path, contents).unwrap();

            assert_eq!(read_settings(&path), Settings::default(), "{name}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), contents, "{name}");
        }
    }

    #[test]
    fn a_missing_file_reads_as_the_defaults() {
        let path = scratch_dir("missing").join("settings.json");

        assert_eq!(read_settings(&path), Settings::default());
    }

    #[test]
    fn an_unchanged_save_does_not_write() {
        let path = scratch_dir("unchanged").join("settings.json");
        let mut written = WrittenSettings::default();
        flush_settings(&changed(), &mut written, &path);
        std::fs::remove_file(&path).unwrap();

        flush_settings(&changed(), &mut written, &path);
        assert!(!path.exists(), "nothing changed, so nothing was written");

        flush_settings(&Settings::default(), &mut written, &path);
        assert_eq!(read_settings(&path), Settings::default());
    }

    #[test]
    fn a_failed_write_is_retried_on_the_next_save() {
        let dir = scratch_dir("failed");
        let blocker = dir.join("not-a-directory");
        std::fs::write(&blocker, "").unwrap();
        let mut written = WrittenSettings::default();

        flush_settings(&changed(), &mut written, &blocker.join("settings.json"));

        assert_eq!(written.0, None);
    }
}
