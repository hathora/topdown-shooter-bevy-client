use std::{net::TcpStream, collections::HashSet};

use bevy::prelude::*;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
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

fn login(app_id: &str) -> Result<LoginResponse, Box<dyn std::error::Error>> {
    let login_url = format!("https://coordinator.hathora.dev/{app_id}/login/anonymous");
    let client = reqwest::blocking::Client::new();
    let resp: LoginResponse = client.post(login_url).send()?.json()?;
    Ok(resp)
}

fn setup_websocket(mut commands: Commands) {
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    // TODO: room should be dynamic
    let room_id = "2g80ygbukgn65";

    let login_result = login(app_id);
    let login_response = login_result.expect("Logging in should succeed");

    let websocket_url = format!("wss://coordinator.hathora.dev/connect/{app_id}");

    dbg!("Attempting to connect to {}", websocket_url.clone());
    let (mut socket, response) =
        connect(Url::parse(&websocket_url).unwrap()).expect("Can't connect");

    dbg!("Connected to the server");
    dbg!("Response HTTP code: {}", response.status());
    dbg!("Response contains the following headers:");
    for (ref header, _value) in response.headers() {
        println!("* {}", header);
    }

    match decode_header(&login_response.token) {
        Ok(ok) => {
            dbg!("{}", ok);
        }
        Err(e) => {
            dbg!("{}", e);
        }
    }

    let mut validation = Validation::new(Algorithm::HS256);
    validation.insecure_disable_signature_validation();
    validation.validate_exp = false;
    validation.required_spec_claims = HashSet::new();

    match decode::<Token>(
        &login_response.token,
        &DecodingKey::from_base64_secret("".as_ref()).expect("asdf"),
        &validation,
    ) {
        Ok(ok) => {
            dbg!("{}", ok);
        }
        Err(e) => {
            dbg!("{}", e);
        }
    }

    dbg!("{}", login_response.to_owned());

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

fn update_state(mut socket: ResMut<WebSocket<MaybeTlsStream<TcpStream>>>) {
    let msg = socket.read_message().expect("Error reading message");
    println!("Receved: {}", msg);
}
