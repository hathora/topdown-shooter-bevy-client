use std::{collections::HashSet, net::TcpStream};

use bevy::prelude::*;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, TokenData, Validation};
use serde::{Deserialize, Serialize};
use tungstenite::{connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

#[derive(Deserialize, Debug, Clone)]
struct LoginResponse {
    token: String,
}

#[derive(Serialize, Debug)]
struct InitialState {
    token: String,
    stateId: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Token {
    id: String,
}

fn decode_user_id_without_validating_jwt(
    token: &str,
) -> Result<TokenData<Token>, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.required_spec_claims = HashSet::new();

    decode::<Token>(
        token,
        // Not verifying, we don't need a secret
        &DecodingKey::from_secret("".as_ref()),
        &validation,
    )
}

fn login(app_id: &str) -> Result<LoginResponse, Box<dyn std::error::Error>> {
    let login_url = format!("https://coordinator.hathora.dev/{app_id}/login/anonymous");
    let client = reqwest::blocking::Client::new();
    let resp: LoginResponse = client.post(login_url).send()?.json()?;
    Ok(resp)
}

struct UserId(String);

fn setup_websocket(mut commands: Commands) {
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    // TODO: room should be dynamic
    let room_id = "2g80ygbukgn65";

    let login_result = login(app_id);
    let login_response = login_result.expect("Logging in should succeed");

    let user_id = decode_user_id_without_validating_jwt(&login_response.token)
        .expect("Decoding JWT should succeed");

    commands.insert_resource(UserId(user_id.claims.id));

    let websocket_url = format!("wss://coordinator.hathora.dev/connect/{app_id}");

    let (mut socket, _response) =
        connect(Url::parse(&websocket_url).unwrap()).expect("Can't connect");

    let initial_state = InitialState {
        token: login_response.token,
        stateId: room_id.to_owned(),
    };
    let message = serde_json::to_vec(&initial_state).expect("Serialization should work");
    match socket.write_message(Message::binary(message)) {
        Ok(_) => {
            dbg!("Successfully connected to websocket.");
        }
        Err(e) => {
            dbg!("Failed to connect to websocket. Error was {}", e);
        }
    }

    commands.insert_resource(socket);
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_websocket)
        .add_system(bevy::window::close_on_esc)
        .add_system(update_state)
        .run();
}

#[derive(Deserialize, Debug)]
struct Player {
    id: String,
    position: Position,
    aimAngle: f32,
}

#[derive(Deserialize, Debug)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Deserialize, Debug)]
struct Bullet {
    id: i32,
    position: Position,
}

#[derive(Deserialize, Debug)]
struct GameState {
    players: Vec<Player>,
    bullets: Vec<Bullet>,
}

#[derive(Deserialize, Debug)]
struct UpdateMessage {
    #[serde(rename = "type")]
    serialized_type: u64,
    ts: u64,
    state: GameState,
}

fn update_state(mut socket: ResMut<WebSocket<MaybeTlsStream<TcpStream>>>) {
    let msg = socket.read_message().expect("Error reading message");

    match msg {
        Message::Text(_) => todo!(),
        Message::Binary(data) => {
            dbg!(String::from_utf8(data.clone()));

            if (!data.is_empty()) {
                let update: UpdateMessage =
                    serde_json::from_slice(&data).expect("Deserialize should work");

                dbg!(update);
            }
        }
        Message::Ping(_) => todo!(),
        Message::Pong(_) => todo!(),
        Message::Close(_) => todo!(),
        Message::Frame(_) => todo!(),
    }
}
