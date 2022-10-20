use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;

use bevy::render::camera::RenderTarget;
use bevy::tasks::AsyncComputeTaskPool;
use bevy::ui::update;
use futures::{stream::StreamExt, Future};
use futures::{FutureExt, SinkExt};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::f32::consts::PI;
use std::iter::Iterator;
use std::net::TcpStream;
use std::pin::Pin;
use std::task::Context;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};
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

fn login(app_id: &str) -> Result<LoginResponse, Box<dyn std::error::Error>> {
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";

    let login_url = format!("https://coordinator.hathora.dev/{app_id}/login/anonymous");
    let client = reqwest::blocking::Client::new();

    let resp: LoginResponse = client.post(login_url).send()?.json()?;
    Ok(resp)
}

#[derive(Component, Eq, PartialEq, Hash, Clone, Debug)]
struct UserId(String);

#[derive(Component)]
struct BulletComponent(i32);

#[derive(Component)]
struct MainCamera;

fn setup(mut commands: Commands) {
    commands
        .spawn_bundle(Camera2dBundle::default())
        .insert(MainCamera);
}

fn main() {
    let room_id = "374jv73032a1i";
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    let login_result = login(app_id);
    let login_response = login_result.expect("Logging in should succeed");

    let user_id = decode_user_id_without_validating_jwt(&login_response.token)
        .expect("Decoding JWT should succeed");
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    let websocket_url = format!("wss://coordinator.hathora.dev/connect/{app_id}");

    let (mut socket, _response) =
        connect(Url::parse(&websocket_url).unwrap()).expect("Can't connect");

    let initial_state = InitialState {
        token: login_response.token,
        stateId: room_id.to_owned(),
    };
    let message = serde_json::to_vec(&initial_state).expect("Serialization should work");
    let message = WsMessage::Binary(message);

    match socket.write_message(Message::binary(message)) {
        Ok(_) => {
            dbg!("Successfully connected to websocket.");
        }
        Err(e) => {
            dbg!("Failed to connect to websocket. Error was {}", e);
        }
    }

    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(socket)
        .insert_resource(UserId(user_id))
        .insert_resource(MouseLocation(Vec2::ZERO))
        .add_startup_system(setup)
        .add_system(bevy::window::close_on_esc)
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

fn read_from_server(
    mut socket: ResMut<WebSocket<MaybeTlsStream<TcpStream>>>,
    client_user_id: Res<UserId>,
    mut camera_query: Query<(&Camera, &mut Transform), (Without<UserId>, Without<BulletComponent>)>,
    mut player_query: Query<
        (Entity, &UserId, &mut Transform),
        (Without<Camera>, Without<BulletComponent>),
    >,
    mut bullet_query: Query<
        (Entity, &BulletComponent, &mut Transform),
        (Without<Camera>, Without<UserId>),
    >,

    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let msg = socket.read_message().expect("Error reading message");

    match msg {
        Message::Text(_) => {
            debug!("Got text");
        }
        Message::Binary(data) => {
            if !data.is_empty() {
                let update: UpdateMessage =
                    serde_json::from_slice(&data).expect("Deserialize should work");

                let mut spawned_players: HashSet<String> = HashSet::new();

                for (entity, user_id, mut player_transform) in &mut player_query {
                    let mut found = false;
                    spawned_players.insert(user_id.0.clone());
                    for player_update in update.state.players.iter() {
                        if player_update.id == user_id.0 {
                            debug!("Updating {:?}", &player_update);
                            found = true;
                            player_transform.translation.x = player_update.position.x;
                            player_transform.translation.y = -player_update.position.y;
                            player_transform.rotation = Quat::from_rotation_z(-player_update.aimAngle)
                        }
                    }

                    if &user_id.0 == &client_user_id.0 {
                        for (_camera, mut camera_transform) in &mut camera_query {
                            debug!("Player transform is {}", player_transform.translation);
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

                    if !found {
                        debug!("Despawning {:?}", user_id);
                        commands.entity(entity).despawn();
                    }
                }

                for player_update in update.state.players.iter() {
                    if !spawned_players.contains(&player_update.id) {
                        debug!("Spawning {}", &player_update.id);
                        let mut entity = commands.spawn();
                        entity
                            .insert(UserId(player_update.id.clone()))
                            .insert_bundle(SpriteBundle {
                                texture: asset_server.load("sprites/player.png"),
                                // TODO: update angle
                                transform: Transform {
                                    translation: Vec3::new(
                                        player_update.position.x,
                                        -player_update.position.y,
                                        0.,
                                    ),
                                    rotation: Quat::from_rotation_z(-player_update.aimAngle),
                                    ..default()
                                },
                                ..default()
                            });

                        if &player_update.id == &client_user_id.0 {
                            entity.insert(CurrentPlayer);
                        }
                    }
                }

                let mut spawned_bullets: HashSet<i32> = HashSet::new();

                for (bullet_entity, bullet, mut bullet_transform) in &mut bullet_query {
                    let mut found = false;
                    spawned_bullets.insert(bullet.0);

                    for bullet_update in update.state.bullets.iter() {
                        if bullet_update.id == bullet.0 {
                            debug!("Updating {}", bullet.0);
                            found = true;
                            bullet_transform.translation.x = bullet_update.position.x;
                            bullet_transform.translation.y = -bullet_update.position.y;
                            debug!("Bullet transform is {}", bullet_transform.translation);
                        }
                    }

                    if !found {
                        debug!("Despawning bullet {}", bullet.0);
                        commands.entity(bullet_entity).despawn();
                    }
                }

                for bullet_update in update.state.bullets.iter() {
                    if !spawned_bullets.contains(&bullet_update.id) {
                        debug!("Spawning bullet {}", bullet_update.id);
                        commands
                            .spawn()
                            .insert(BulletComponent(bullet_update.id))
                            .insert_bundle(SpriteBundle {
                                texture: asset_server.load("sprites/bullet.png"),
                                // TODO: update angle
                                transform: Transform {
                                    translation: Vec3::new(
                                        bullet_update.position.x,
                                        -bullet_update.position.y,
                                        0.,
                                    ),
                                    ..default()
                                },
                                ..default()
                            });
                    }
                }
            }
        }
        Message::Ping(_) => {
            debug!("Got ping");
        }
        Message::Pong(_) => {
            debug!("Got pong");
        }
        Message::Close(_) => {
            debug!("Got close");
        }
        Message::Frame(_) => {
            debug!("Got frame");
        }
    }
}

#[derive(Component)]
struct CurrentPlayer;

#[derive(Serialize)]
struct MoveInput {
    #[serde(rename = "type")]
    serialized_type: u64,
    direction: u64,
}

#[derive(Serialize)]
struct AngleInput {
    #[serde(rename = "type")]
    serialized_type: u64,
    angle: f32,
}

#[derive(Serialize)]
struct ClickInput {
    #[serde(rename = "type")]
    serialized_type: u64,
}

struct MouseLocation(Vec2);

fn write_inputs(
    input: Res<Input<KeyCode>>,
    mut query: Query<(&CurrentPlayer, &Transform)>,
    // need to get window dimensions
    wnds: Res<Windows>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut mouse_location: ResMut<MouseLocation>,
    mut mouse_motion_events: EventReader<MouseMotion>,
    mouse_button_input: Res<Input<MouseButton>>,

    mut socket: ResMut<WebSocket<MaybeTlsStream<TcpStream>>>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        debug!("Mouse clicked.");
        let mouse_input = ClickInput { serialized_type: 2 };
        let message = serde_json::to_vec(&mouse_input).expect("Serialization should work");
        socket.write_message(Message::Binary(message));
    }

    debug!("Processing keyboard input");

    let mut keyboard_update_necessary = false;
    let mut direction = 0;

    if input.any_just_released([KeyCode::W, KeyCode::A, KeyCode::S, KeyCode::D]) {
        keyboard_update_necessary = true;
    }

    if input.just_pressed(KeyCode::W) {
        keyboard_update_necessary = true;
        direction = 1
    } else if input.just_pressed(KeyCode::S) {
        keyboard_update_necessary = true;
        direction = 2;
    } else if input.just_pressed(KeyCode::A) {
        keyboard_update_necessary = true;
        direction = 3;
    } else if input.just_pressed(KeyCode::D) {
        keyboard_update_necessary = true;
        direction = 4;
    }

    if keyboard_update_necessary {
        debug!("Writing input");
        let input = MoveInput {
            serialized_type: 0,
            direction,
        };

        let message = serde_json::to_vec(&input).expect("Serialization should work");
        socket.write_message(Message::Binary(message));
    }

    if !mouse_motion_events.is_empty() {
        debug!("Processing mouse input");

        // get the camera info and transform
        // assuming there is exactly one main camera entity, so query::single() is OK
        let (camera, camera_transform) = q_camera.single();

        // get the window that the camera is displaying to (or the primary window)
        let wnd = if let RenderTarget::Window(id) = camera.target {
            wnds.get(id).unwrap()
        } else {
            wnds.get_primary().unwrap()
        };

        // check if the cursor is inside the window and get its position
        if let Some(screen_pos) = wnd.cursor_position() {
            // get the size of the window
            let window_size = Vec2::new(wnd.width() as f32, wnd.height() as f32);

            // convert screen position [0..resolution] to ndc [-1..1] (gpu coordinates)
            let ndc = (screen_pos / window_size) * 2.0 - Vec2::ONE;

            // matrix for undoing the projection and camera transform
            let ndc_to_world =
                camera_transform.compute_matrix() * camera.projection_matrix().inverse();

            // use it to convert ndc to world-space coordinates
            let world_pos = ndc_to_world.project_point3(ndc.extend(-1.0));

            // reduce it to a 2D value
            let world_pos: Vec2 = world_pos.truncate();

            debug!("Mouse coords: {}/{}", world_pos.x, world_pos.y);

            for (_, player_transform) in query.iter() {
                let angle =
                    (world_pos - player_transform.translation.truncate()).angle_between(Vec2::X);
                debug!("Angle {}", angle);

                let mouse_input = AngleInput {
                    serialized_type: 1,
                    angle,
                };

                let message = serde_json::to_vec(&mouse_input).expect("Serialization should work");
                socket.write_message(Message::Binary(message));
            }

            // todo: remove this
            *mouse_location = MouseLocation(world_pos);
        }
    }
}
