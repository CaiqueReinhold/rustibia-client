use std::{fs, path::PathBuf, sync::LazyLock};

use bevy::prelude::*;
use serde::Deserialize;

pub const CONFIG_FILE_NAME: &str = "client_conf.yaml";

pub static CONFIG: LazyLock<ClientConfig> = LazyLock::new(read_from_file);

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct ClientConfig {
    pub server_address: String,
    pub site_url: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        ClientConfig {
            server_address: "rustibia.online:5555".to_string(),
            site_url: "https://rustibia.online".to_string(),
        }
    }
}

impl ClientConfig {
    pub fn auth_endpoint(&self) -> String {
        format!("{}/api/auth", self.site())
    }

    pub fn char_list_endpoint(&self) -> String {
        format!("{}/api/characters", self.site())
    }

    pub fn game_token_endpoint(&self, character_id: u32) -> String {
        format!("{}/api/characters/{}/token", self.site(), character_id)
    }

    fn site(&self) -> &str {
        self.site_url.trim_end_matches('/')
    }
}

/// Next to the executable first — that is the install directory for a packaged
/// build — then the working directory, which is where it lands when running from
/// the repo with `cargo run`.
fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(2);
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        paths.push(dir.join(CONFIG_FILE_NAME));
    }
    paths.push(PathBuf::from(CONFIG_FILE_NAME));
    paths
}

fn read_from_file() -> ClientConfig {
    read_from(&config_paths())
}

fn read_from(paths: &[PathBuf]) -> ClientConfig {
    for path in paths {
        let Ok(contents) = fs::read_to_string(path) else {
            continue;
        };
        match serde_yaml::from_str::<ClientConfig>(&contents) {
            Ok(config) => {
                info!("loaded {}", path.display());
                return config;
            }
            // A typo in the file must not silently connect the player somewhere
            // else, so the parse error is loud even though it is not fatal.
            Err(e) => error!("failed to parse {}: {e}", path.display()),
        }
        break;
    }

    // Reached when no file exists, and also when one existed but did not parse.
    let config = ClientConfig::default();
    info!(
        "no usable {CONFIG_FILE_NAME}, using built-in defaults ({})",
        config.server_address
    );
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the file's shape. The field names are what a player edits, so renaming
    /// one silently falls back to a default that points at localhost.
    #[test]
    fn parses_a_full_config_file() {
        let config: ClientConfig = serde_yaml::from_str(
            r#"
server_address: "play.example.com:5555"
site_url: "https://play.example.com"
"#,
        )
        .expect("the shipped file layout must parse");

        assert_eq!(config.server_address, "play.example.com:5555");
        assert_eq!(config.auth_endpoint(), "https://play.example.com/api/auth");
    }

    /// Every field is optional so an old file keeps working after a new one is
    /// added, rather than dropping the player back onto localhost.
    #[test]
    fn missing_fields_keep_their_defaults() {
        let config: ClientConfig = serde_yaml::from_str("site_url: \"http://example.com\"")
            .expect("a partial file must parse");

        assert_eq!(
            config.server_address,
            ClientConfig::default().server_address
        );
        assert_eq!(
            config.char_list_endpoint(),
            "http://example.com/api/characters"
        );
    }

    /// The shipped template has to be a file the client can actually read back,
    /// not just a valid-looking one -- it is what the installer puts on disk.
    #[test]
    fn the_packaged_template_loads() {
        let template = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("packaging")
            .join(CONFIG_FILE_NAME);

        let config = read_from(std::slice::from_ref(&template));

        assert_eq!(
            config.server_address,
            ClientConfig::default().server_address
        );
    }

    /// The first readable file wins, and a later one is not consulted -- that is
    /// what makes the install directory beat the working directory.
    #[test]
    fn the_first_readable_path_wins() {
        let dir = std::env::temp_dir().join("rustibia-config-precedence");
        fs::create_dir_all(&dir).expect("temp dir");
        let first = dir.join("first.yaml");
        let second = dir.join("second.yaml");
        fs::write(&first, "server_address: \"first:1\"\n").expect("write");
        fs::write(&second, "server_address: \"second:2\"\n").expect("write");

        let config = read_from(&[dir.join("missing.yaml"), first, second]);

        assert_eq!(config.server_address, "first:1");
        let _ = fs::remove_dir_all(&dir);
    }

    /// A file a player broke while editing must not take the client down, and
    /// must not silently inherit half of it either.
    #[test]
    fn an_unparsable_file_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join("rustibia-config-broken");
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(CONFIG_FILE_NAME);
        fs::write(&path, "server_address: [not, a, string]\n").expect("write");

        let config = read_from(&[path]);

        assert_eq!(
            config.server_address,
            ClientConfig::default().server_address
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_trailing_slash_does_not_double_up() {
        let config = ClientConfig {
            site_url: "http://example.com/".to_string(),
            ..Default::default()
        };

        assert_eq!(
            config.game_token_endpoint(7),
            "http://example.com/api/characters/7/token"
        );
    }
}
