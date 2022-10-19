use bevy::{prelude::*, render::camera, tasks::AsyncComputeTaskPool};
use futures::future::Ready;
use futures::{stream::StreamExt, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use futures::{FutureExt, SinkExt};
use pharos::*;
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::iter::Iterator;
use std::pin::Pin;
use std::task::Context;
use url::Url;
use wasm_bindgen::prelude::*;
use web_sys::WebSocket;
use ws_stream_wasm::{WsMessage, WsMeta, WsStream};

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

fn setup_websocket(mut commands: Commands) {
    commands.spawn_bundle(Camera2dBundle::default());

    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    let websocket_url = format!("wss://coordinator.hathora.dev/connect/{app_id}");
    // let (mut socket, _response) =
    //     connect(Url::parse(&websocket_url).unwrap()).expect("Can't connect");

    // // // TODO: room should be dynamic
    // let room_id = "2g80ygbukgn65";
    // let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";

    // let thread_pool = AsyncComputeTaskPool::get();

    // let x = thread_pool.spawn_local(async move {
    //     web_sys::console::log_1(&"inside task".into());

    // let login_result = login(app_id).await;
    // let login_response = login_result.expect("Logging in should succeed");
    // web_sys::console::log_1(&login_response.token.into());

    // let user_id = decode_user_id_without_validating_jwt(&login_response.token)
    // .expect("Decoding JWT should succeed");

    // commands.insert_resource(UserId(user_id.claims.id));

    // let initial_state = InitialState {
    //     token: login_response.token,
    //     stateId: room_id.to_owned(),
    // };
    // let message = serde_json::to_vec(&initial_state).expect("Serialization should work");
    // match socket.write_message(Message::binary(message)) {
    //     Ok(_) => {
    //         dbg!("Successfully connected to websocket.");
    //     }
    //     Err(e) => {
    //         dbg!("Failed to connect to websocket. Error was {}", e);
    //     }
    // }

    // commands.insert_resource(socket);
}

#[wasm_bindgen]
pub async fn run() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    let room_id = "374jv73032a1i";
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    let login_result = login(app_id).await;
    let login_response = login_result.expect("Logging in should succeed");
    web_sys::console::log_1(&"decoding user_id...".into());

    let user_id = decode_user_id_without_validating_jwt(&login_response.token)
        .expect("Decoding JWT should succeed");

    web_sys::console::log_1(&"done decoding user_id".into());
    web_sys::console::log_1(&user_id.clone().into());

    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    let websocket_url = format!("wss://coordinator.hathora.dev/connect/{app_id}");

    log::info!("Connecting to websocket");

    let (mut ws, mut ws_stream) = WsMeta::connect(websocket_url, None)
        .await
        .expect_throw("assume the connection succeeds");

    log::info!("Connected to websocket!");

    // let mut io_stream = ws_stream.into_io();

    // let (mut read, mut write) = io_stream.split();

    let initial_state = InitialState {
        token: login_response.token,
        stateId: room_id.to_owned(),
    };
    let message = serde_json::to_vec(&initial_state).expect("Serialization should work");

    log::info!("Writing to websocket.");
    let message = WsMessage::Binary(message);

    let send_result = ws_stream.send(message).await;

    match send_result {
        Ok(_) => log::info!("Successfully wrote to websocket."),
        Err(e) => log::error!("Failed to write to websocket; {}", e),
    }

    // let mut output_1 = Vec::with_capacity(1024);

    // let x = serde_json::from_reader(io_stream);

    // todo: hook this into bevy lifecycle somehow

    // let a = futures::executor::block_on(ws_stream.next());

    log::info!("Reading from websocket.");
    // while let Some(message) = ws_stream.next().await {
    //     match message {
    //         WsMessage::Text(t) => {
    //             log::info!("Got text: {:#?}", t);
    //             if !t.is_empty() {
    //                 let x: UpdateMessage =
    //                     serde_json::from_str(&t).expect("Successfully deserialized update");
    //             }
    //         }
    //         WsMessage::Binary(b) => {
    //             log::info!(
    //                 "Got binary: {:#?}",
    //                 String::from_utf8(b.clone()).expect("asdf")
    //             );
    //             if !b.is_empty() {
    //                 let x: UpdateMessage =
    //                     serde_json::from_slice(&b).expect("Successfully deserialized update");
    //             }
    //         }
    //     }
    // }

    // let mut output_2 = Vec::with_capacity(1024);

    // while let Ok(bytes) = io_stream.read_until(b'}', &mut output_2).await {
    //     if bytes == 0 {
    //         log::info!("Zero byte update");
    //         // log::info!("Read from websocket: {:#?}", output_2);
    //     } else {
    //         log::info!("Read {} bytes from websocket.", bytes);
    //         // log::info!("Read from websocket: {:#?}", output_2);
    //     }
    // }

    // let mut evts = ws
    //     .observe(ObserveConfig::default())
    //     .await
    //     .expect_throw("Observe");
    // let x = evts.next().await.expect("First message");

    App::new()
        // .add_plugins(DefaultPlugins)
        // .insert_resource(ws_stream)
        // .insert_resource(UserId(user_id))
        // .add_startup_system(setup_websocket)
        // .add_system(bevy::window::close_on_esc)
        // .add_system(update_state)
        .add_system(hello_world)
        .run();
}

fn hello_world() {
    log::info!("Hello world");
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

fn update_state(
    mut socket: ResMut<WsStream>,
    client_user_id: Res<UserId>,
    mut camera_query: Query<(&Camera, &mut Transform), Without<UserId>>,
    mut query: Query<(Entity, &UserId, &mut Transform), Without<Camera>>,
    mut commands: Commands,
) {
    log::info!("Inside system");

    // let waker = noop_waker::noop_waker();
    // let mut ctx = Context::from_waker(&waker);

    // let mut message = socket.next();
    // use futures_lite::future::FutureExt;
    // match message.poll(&mut ctx) {
    //     std::task::Poll::Ready(_) => {
    //         log::info!("ready");
    //     }
    //     std::task::Poll::Pending => {
    //         log::info!("pending");
    //     }
    // }

    // match socket.poll_next_unpin(&mut ctx) {
    //     std::task::Poll::Ready(message) => {
    //         log::info!("Message is ready");

    //         match message {
    //             Some(message) => match message {
    //                 WsMessage::Text(t) => {
    //                     log::info!("Got text: {:#?}", t);
    //                     if !t.is_empty() {
    //                         let x: UpdateMessage =
    //                             serde_json::from_str(&t).expect("Successfully deserialized update");
    //                     }
    //                 }
    //                 WsMessage::Binary(b) => {
    //                     log::info!(
    //                         "Got binary: {:#?}",
    //                         String::from_utf8(b.clone()).expect("asdf")
    //                     );
    //                     if !b.is_empty() {
    //                         let x: UpdateMessage = serde_json::from_slice(&b)
    //                             .expect("Successfully deserialized update");
    //                     }
    //                 }
    //             },
    //             None => {
    //                 log::info!("Message was None");
    //             }
    //         }
    //     }
    //     std::task::Poll::Pending => {
    //         log::info!("Still pending");
    //     }
    // }
    // futures::ready!(message);

    // match z.await {
    //     Some(_) => todo!(),
    //     None => todo!(),
    // }

    //

    // let message = futures::executor::block_on(socket.next());

    // if let Some(message) = message {
    //     match message {
    //         WsMessage::Text(t) => {
    //             log::info!("Got text: {:#?}", t);
    //             if !t.is_empty() {
    //                 let x: UpdateMessage =
    //                     serde_json::from_str(&t).expect("Successfully deserialized update");
    //             }
    //         }
    //         WsMessage::Binary(b) => {
    //             log::info!(
    //                 "Got binary: {:#?}",
    //                 String::from_utf8(b.clone()).expect("asdf")
    //             );
    //             if !b.is_empty() {
    //                 let x: UpdateMessage =
    //                     serde_json::from_slice(&b).expect("Successfully deserialized update");
    //             }
    //         }
    //     }
    // } else {
    //     log::info!("Got a None message.");
    // }

    //     // let msg = socket.read_message().expect("Error reading message");

    //     // dbg!(client_user_id);

    //     // TODO: update camera to point at client_user_id

    //     // match msg {
    //     //     Message::Text(_) => todo!(),
    //     //     Message::Binary(data) => {
    //     //         if !data.is_empty() {
    //     //             let update: UpdateMessage =
    //     //                 serde_json::from_slice(&data).expect("Deserialize should work");

    //     //             let mut spawned: HashSet<String> = HashSet::new();

    //     //             for (entity, user_id, mut player_transform) in &mut query {
    //     //                 if &user_id.0 == &client_user_id.0 {
    //     //                     for (_camera, mut camera_transform) in &mut camera_query {
    //     //                         *camera_transform = Transform {
    //     //                             translation: Vec3::new(
    //     //                                 player_transform.translation.x,
    //     //                                 player_transform.translation.y,
    //     //                                 camera_transform.translation.z,
    //     //                             ),
    //     //                             ..*camera_transform
    //     //                         };
    //     //                     }
    //     //                 }

    //     //                 let mut found = false;
    //     //                 spawned.insert(user_id.0.clone());
    //     //                 for player in update.state.players.iter() {
    //     //                     if player.id == user_id.0 {
    //     //                         // dbg!("Updating {}", &player);
    //     //                         found = true;
    //     //                         player_transform.translation.x = player.position.x;
    //     //                         player_transform.translation.y = player.position.y;
    //     //                     }
    //     //                 }
    //     //                 if !found {
    //     //                     dbg!("Despawning {}", user_id);
    //     //                     commands.entity(entity).despawn();
    //     //                 }
    //     //             }

    //     //             for player in update.state.players.iter() {
    //     //                 if !spawned.contains(&player.id) {
    //     //                     dbg!("Spawning {}", &player.id);
    //     //                     commands
    //     //                         .spawn()
    //     //                         .insert(UserId(player.id.clone()))
    //     //                         .insert_bundle(SpriteBundle {
    //     //                             // TODO: update angle
    //     //                             transform: Transform {
    //     //                                 translation: Vec3::new(
    //     //                                     player.position.x,
    //     //                                     player.position.y,
    //     //                                     0.,
    //     //                                 ),
    //     //                                 ..default()
    //     //                             },
    //     //                             ..default()
    //     //                         });
    //     //                 }
    //     //             }
    //     //         }
    //     //     }
    //     //     Message::Ping(_) => {
    //     //         dbg!("Got ping");
    //     //     }
    //     //     Message::Pong(_) => todo!(),
    //     //     Message::Close(_) => todo!(),
    //     //     Message::Frame(_) => todo!(),
    //     // }
}
