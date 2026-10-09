use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, futures_lite::future},
};
use reqwest::blocking::Client;
use serde::Deserialize;

use crate::config::CONFIG;

pub const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Resource, Debug, Clone, PartialEq, Eq, Default)]
pub enum VersionCheck {
    #[default]
    Pending,
    Current,
    Outdated {
        required: String,
        download_url: String,
    },
    Unreachable,
}

#[derive(Deserialize)]
struct ClientVersionResponse {
    version: String,
    download_url: String,
}

fn classify(status: u16, body: &str, own: &str) -> VersionCheck {
    if status != 200 {
        return VersionCheck::Unreachable;
    }
    match serde_json::from_str::<ClientVersionResponse>(body) {
        Ok(response) if response.version == own => VersionCheck::Current,
        Ok(response) => VersionCheck::Outdated {
            required: response.version,
            download_url: response.download_url,
        },
        Err(_) => VersionCheck::Unreachable,
    }
}

fn fetch() -> VersionCheck {
    let endpoint = CONFIG.client_version_endpoint();
    let response = match Client::new().get(&endpoint).send() {
        Ok(response) => response,
        Err(e) => {
            error!("version check against {} failed: {}", endpoint, e);
            return VersionCheck::Unreachable;
        }
    };
    let status = response.status().as_u16();
    match response.text() {
        Ok(body) => {
            let check = classify(status, &body, CLIENT_VERSION);
            info!(
                "version check: {:?} (this client is {})",
                check, CLIENT_VERSION
            );
            check
        }
        Err(e) => {
            error!(
                "version check against {} returned {}, but the body could not be read: {}",
                endpoint, status, e
            );
            VersionCheck::Unreachable
        }
    }
}

#[derive(Resource)]
pub(super) struct VersionCheckTask(Task<VersionCheck>);

#[derive(Event, Debug)]
pub struct CheckVersion;

pub(super) fn start_version_check(mut commands: Commands) {
    commands.trigger(CheckVersion);
}

pub(super) fn on_check_version(
    _: On<CheckVersion>,
    task: Option<Res<VersionCheckTask>>,
    mut check: ResMut<VersionCheck>,
    mut commands: Commands,
) {
    if task.is_some() {
        return;
    }
    *check = VersionCheck::Pending;
    commands.insert_resource(VersionCheckTask(IoTaskPool::get().spawn(async { fetch() })));
}

pub(super) fn poll_version_check(
    mut commands: Commands,
    mut task: ResMut<VersionCheckTask>,
    mut check: ResMut<VersionCheck>,
) {
    if !task.0.is_finished() {
        return;
    }
    *check = future::block_on(future::poll_once(&mut task.0)).unwrap_or(VersionCheck::Unreachable);
    commands.remove_resource::<VersionCheckTask>();
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"version":"0.2.0","download_url":"https://rustibia.online/download"}"#;

    #[test]
    fn the_supported_version_is_current() {
        assert_eq!(classify(200, BODY, "0.2.0"), VersionCheck::Current);
    }

    #[test]
    fn an_older_client_is_outdated() {
        assert_eq!(
            classify(200, BODY, "0.1.0"),
            VersionCheck::Outdated {
                required: "0.2.0".to_string(),
                download_url: "https://rustibia.online/download".to_string(),
            }
        );
    }

    #[test]
    fn a_newer_client_is_outdated_too() {
        assert!(matches!(
            classify(200, BODY, "0.3.0"),
            VersionCheck::Outdated { .. }
        ));
    }

    #[test]
    fn an_unreadable_body_is_unreachable() {
        assert_eq!(classify(200, "<html>", "0.2.0"), VersionCheck::Unreachable);
    }

    #[test]
    fn an_error_status_is_unreachable() {
        assert_eq!(classify(503, BODY, "0.2.0"), VersionCheck::Unreachable);
    }
}
