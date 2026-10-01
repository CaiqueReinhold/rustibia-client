use std::collections::HashMap;

use bevy::{
    prelude::*,
    tasks::{IoTaskPool, Task, futures_lite::future},
};

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::{
    config::CONFIG,
    core::GameState,
    game_ui::{LoginPhase, PendingLoginError},
    network::LoginCredentials,
};

#[derive(Event, Debug)]
pub struct FetchCharacters {
    pub email: String,
    pub password: String,
}

#[derive(Event, Debug)]
pub struct GenerateLoginToken {
    pub character_id: u32,
}

#[derive(Deserialize, Debug)]
struct AuthResponse {
    pub session_token: String,
}

#[derive(Deserialize, Debug)]
struct GameTokenResponse {
    pub auth_token: String,
}

#[derive(Deserialize, Debug)]
pub struct CharacterSummary {
    pub id: u32,
    pub name: String,
    pub level: u16,
    pub vocation: String,
}

#[derive(Resource)]
pub struct CharacterList {
    pub characters: Vec<CharacterSummary>,
    pub session_token: String,
}

/// Leaves out `session_token`: this is logged.
impl std::fmt::Debug for CharacterList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CharacterList")
            .field("characters", &self.characters)
            .finish_non_exhaustive()
    }
}

#[derive(Resource, Debug)]
pub(super) struct LoginTask(Task<Result<CharacterList, String>>);

#[derive(Resource, Debug)]
pub(super) struct GameTokenTask(Task<Result<String, String>>);

fn get_session_token(email: String, password: String, client: &Client) -> Result<String, String> {
    let mut body = HashMap::new();
    body.insert("email", email);
    body.insert("password", password);
    let endpoint = CONFIG.auth_endpoint();
    let response = client.post(&endpoint).json(&body).send();
    match response {
        Ok(response) => {
            let status = response.status();
            match response.json::<AuthResponse>() {
                Ok(session) => return Ok(session.session_token),
                Err(e) => error!(
                    "auth request to {} returned {}, but the body could not be read: {}",
                    endpoint, status, e
                ),
            }
        }
        Err(e) => error!("auth request to {} failed: {}", endpoint, e),
    }
    Err("Failed to authenticate. Try again later or contact support.".to_owned())
}

fn get_character_list(session_token: String, client: &Client) -> Result<CharacterList, String> {
    let endpoint = CONFIG.char_list_endpoint();
    let response = client
        .get(&endpoint)
        .header("Authorization", format!("Bearer {}", session_token))
        .send();

    match response {
        Ok(response) => {
            let status = response.status();
            match response.json::<Vec<CharacterSummary>>() {
                Ok(characters) => {
                    return Ok(CharacterList {
                        characters,
                        session_token,
                    });
                }
                Err(e) => error!(
                    "character list request to {} returned {}, but the body could not be read: {}",
                    endpoint, status, e
                ),
            }
        }
        Err(e) => error!("character list request to {} failed: {}", endpoint, e),
    }
    Err("Failed to authenticate. Try again later or contact support.".to_owned())
}

fn auth_and_get_characters(email: String, password: String) -> Result<CharacterList, String> {
    let client = Client::new();
    info!("auth_and_get_characters");
    let session_token = get_session_token(email, password, &client)?;
    let characters = get_character_list(session_token, &client)?;
    info!("characters: {:?}", characters);
    Ok(characters)
}

pub(super) fn on_fetch_characters(event: On<FetchCharacters>, mut commands: Commands) {
    let email = event.email.clone();
    let password = event.password.clone();
    let task = IoTaskPool::get().spawn(async move { auth_and_get_characters(email, password) });
    commands.insert_resource(LoginTask(task));
}

pub(super) fn pool_login_task(
    mut commands: Commands,
    task: Option<ResMut<LoginTask>>,
    mut login_error: ResMut<PendingLoginError>,
) {
    let Some(mut task) = task else {
        return;
    };

    if !task.0.is_finished() {
        return;
    }

    let result = future::block_on(future::poll_once(&mut task.0));
    let result = result.unwrap_or(Err(
        "Failed to authenticate. Try again later or contact support.".to_owned(),
    ));

    match result {
        Ok(characters) => {
            commands.insert_resource(characters);
            commands.set_state(LoginPhase::CharacterList);
        }
        Err(e) => {
            login_error.0 = Some(("Failed to authenticate".to_owned(), e));
            commands.set_state(LoginPhase::EnterGame);
        }
    }

    commands.remove_resource::<LoginTask>();
}

fn fetch_game_token(session_token: String, character_id: u32) -> Result<String, String> {
    let client = Client::new();
    let response = client
        .post(CONFIG.game_token_endpoint(character_id))
        .header("Authorization", format!("Bearer {}", session_token))
        .send();

    match response {
        Ok(response) => {
            let status = response.status();
            match response.json::<GameTokenResponse>() {
                Ok(game_token) => {
                    return Ok(game_token.auth_token);
                }
                Err(e) => error!(
                    "Request game token returned {}, but the body could not be read: {}",
                    status, e
                ),
            }
        }
        Err(e) => error!("Game token request failed: {}", e),
    }
    Err("Failed to connect. Try again later or contact support.".to_owned())
}

pub(super) fn on_generate_game_token(
    event: On<GenerateLoginToken>,
    mut commands: Commands,
    characters: Res<CharacterList>,
) {
    let character_id = event.character_id;
    let session_token = characters.session_token.clone();
    let task =
        IoTaskPool::get().spawn(async move { fetch_game_token(session_token, character_id) });
    commands.insert_resource(GameTokenTask(task));
}

pub(super) fn pool_generate_game_token(
    mut commands: Commands,
    task: Option<ResMut<GameTokenTask>>,
    mut login_error: ResMut<PendingLoginError>,
) {
    let Some(mut task) = task else {
        return;
    };

    if !task.0.is_finished() {
        return;
    }

    let result = future::block_on(future::poll_once(&mut task.0));
    let result = result.unwrap_or(Err(
        "Failed to connect. Try again later or contact support.".to_owned(),
    ));

    match result {
        Ok(auth_token) => {
            commands.insert_resource(LoginCredentials { auth_token });
            commands.set_state(GameState::Connecting);
        }
        Err(e) => {
            login_error.0 = Some(("Failed to connect".to_owned(), e));
            commands.set_state(LoginPhase::EnterGame);
        }
    }

    commands.remove_resource::<GameTokenTask>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_character_list_never_prints_its_session_token() {
        let list = CharacterList {
            characters: Vec::new(),
            session_token: "secret-session".to_string(),
        };

        assert!(!format!("{list:?}").contains("secret-session"));
    }
}
