use bevy::prelude::*;

use bevy::tasks::AsyncComputeTaskPool;
use bevy::ui::update;
use futures::{stream::StreamExt, Future};
use futures::{FutureExt, SinkExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::iter::Iterator;
use std::pin::Pin;
use std::task::Context;
use wasm_bindgen::prelude::*;
use ws_stream_wasm::{WsErr, WsMessage, WsMeta, WsStream};

#[derive(Deserialize, Debug, Clone)]
struct LoginResponse {
    token: String,
}

#[derive(Serialize, Debug)]
struct InitialState {
    token: String,
    stateId: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Token {
    id: String,
}

#[derive(Debug)]
struct TokenError;

fn decode_user_id_without_validating_jwt(token: &str) -> Result<String, TokenError> {
    let segments: Vec<&str> = token.split('.').collect();
    let id = segments[1];

    match base64::decode_config(segments[1], base64::URL_SAFE_NO_PAD) {
        Ok(data) => {
            let string = String::from_utf8(data).expect("base64 output is valid utf8");
            let token: Token = serde_json::from_str(&string).expect("token JSON is valid");
            Ok(token.id)
        }
        Err(_) => Err(TokenError),
    }
}

async fn login(app_id: &str) -> Result<LoginResponse, Box<dyn std::error::Error>> {
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";

    let login_url = format!("https://coordinator.hathora.dev/{app_id}/login/anonymous");
    let client = reqwest::Client::new();

    let resp: LoginResponse = client.post(login_url).send().await?.json().await?;
    Ok(resp)
}

#[derive(Component, Eq, PartialEq, Hash, Clone, Debug)]
struct UserId(String);

fn setup(mut commands: Commands) {
    commands.spawn_bundle(Camera2dBundle::default());
}

#[wasm_bindgen]
pub async fn run() {
    let room_id = "374jv73032a1i";
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    let login_result = login(app_id).await;
    let login_response = login_result.expect("Logging in should succeed");

    let user_id = decode_user_id_without_validating_jwt(&login_response.token)
        .expect("Decoding JWT should succeed");
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    let websocket_url = format!("wss://coordinator.hathora.dev/connect/{app_id}");

    let (_ws, mut ws_stream) = WsMeta::connect(websocket_url, None)
        .await
        .expect_throw("assume the connection succeeds");

    let initial_state = InitialState {
        token: login_response.token,
        stateId: room_id.to_owned(),
    };
    let message = serde_json::to_vec(&initial_state).expect("Serialization should work");
    let message = WsMessage::Binary(message);
    let send_result = ws_stream.send(message).await;

    match send_result {
        Ok(_) => debug!("Successfully wrote to websocket."),
        Err(e) => error!("Failed to write to websocket; {}", e),
    }

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(ws_stream)
        .insert_resource(UserId(user_id))
        .add_startup_system(setup)
        .add_system(read_from_server)
        .add_system(write_inputs)
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

fn poll_for_update(socket: &mut WsStream) -> Option<UpdateMessage> {
    let waker = noop_waker::noop_waker();
    let mut ctx = Context::from_waker(&waker);

    match socket.poll_next_unpin(&mut ctx) {
        std::task::Poll::Ready(message) => match message {
            Some(message) => match message {
                WsMessage::Text(t) => {
                    if !t.is_empty() {
                        return Some(
                            serde_json::from_str::<UpdateMessage>(&t)
                                .expect("Successfully deserialized update"),
                        );
                    }
                    None
                }
                WsMessage::Binary(b) => {
                    if !b.is_empty() {
                        return Some(
                            serde_json::from_slice::<UpdateMessage>(&b)
                                .expect("Successfully deserialized update"),
                        );
                    }
                    return None;
                }
            },
            None => {
                return None;
            }
        },
        std::task::Poll::Pending => {
            return None;
        }
    }
}

fn read_from_server(
    mut socket: ResMut<WsStream>,
    client_user_id: Res<UserId>,
    mut camera_query: Query<(&Camera, &mut Transform), Without<UserId>>,
    mut query: Query<(Entity, &UserId, &mut Transform), Without<Camera>>,
    mut commands: Commands,
) {
    if let Some(update) = poll_for_update(&mut socket) {
        let mut spawned: HashSet<String> = HashSet::new();

        for (entity, user_id, mut player_transform) in &mut query {
            if &user_id.0 == &client_user_id.0 {
                for (_camera, mut camera_transform) in &mut camera_query {
                    *camera_transform = Transform {
                        translation: Vec3::new(
                            player_transform.translation.x,
                            player_transform.translation.y,
                            camera_transform.translation.z,
                        ),
                        ..*camera_transform
                    };
                }
            }

            let mut found = false;
            spawned.insert(user_id.0.clone());
            for player in update.state.players.iter() {
                if player.id == user_id.0 {
                    debug!("Updating {:?}", player.id);
                    found = true;
                    player_transform.translation.x = player.position.x;
                    player_transform.translation.y = player.position.y;
                }
            }
            if !found {
                info!("Despawning {:?}", entity);
                commands.entity(entity).despawn();
            }
        }

        for player in update.state.players.iter() {
            if !spawned.contains(&player.id) {
                info!("Spawning {}", &player.id);
                commands
                    .spawn()
                    .insert(UserId(player.id.clone()))
                    .insert_bundle(SpriteBundle {
                        // TODO: update angle
                        transform: Transform {
                            translation: Vec3::new(player.position.x, player.position.y, 0.),
                            ..default()
                        },
                        ..default()
                    });
            }
        }
    }
}

#[derive(Serialize)]
struct MoveInput {
    #[serde(rename = "type")]
    serialized_type: u64,
    direction: u64,
}

fn write_inputs(
    input: Res<Input<KeyCode>>,
    mut socket: ResMut<WsStream>,
    // input_future: ResMut<>>,
) {
    let waker = noop_waker::noop_waker();
    let mut ctx = Context::from_waker(&waker);
    match socket.poll_flush_unpin(&mut ctx) {
        std::task::Poll::Ready(_) => {
            debug!("Write buffer is empty");
        }
        std::task::Poll::Pending => {
            debug!("Write buffer is still writing");
            return;
        }
    }

    debug!("Processing keyboard input");

    let mut update_necessary = false;
    let mut direction = 0;

    if input.any_just_released([KeyCode::W, KeyCode::A, KeyCode::S, KeyCode::D]) {
        update_necessary = true;
    }

    if input.just_pressed(KeyCode::W) {
        update_necessary = true;
        direction = 1
    } else if input.just_pressed(KeyCode::S) {
        update_necessary = true;
        direction = 2;
    } else if input.just_pressed(KeyCode::A) {
        update_necessary = true;
        direction = 3;
    } else if input.just_pressed(KeyCode::D) {
        update_necessary = true;
        direction = 4;
    }

    if update_necessary {
        debug!("Writing input");
        let input = MoveInput {
            serialized_type: 0,
            direction,
        };

        let message = serde_json::to_vec(&input).expect("Serialization should work");
        let message = WsMessage::Binary(message);
        let mut task = socket.send(message);

        debug!("{:?}", task.poll_unpin(&mut ctx));
    }
}
