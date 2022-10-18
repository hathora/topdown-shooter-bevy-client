use bevy::{prelude::*, render::camera, tasks::AsyncComputeTaskPool};
use jsonwebtoken::{decode, Algorithm, DecodingKey, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use wasm_bindgen::prelude::*;
// use tungstenite::{connect, stream::MaybeTlsStream, Message, WebSocket};
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

async fn login(app_id: &str) -> Result<LoginResponse, Box<dyn std::error::Error>> {
    let login_url = format!("https://coordinator.hathora.dev/{app_id}/login/anonymous");
    let client = reqwest::Client::new();

    let resp: LoginResponse = client.post(login_url).send().await?.json().await?;
    Ok(resp)
}

#[derive(Component, Eq, PartialEq, Hash, Clone, Debug)]
struct UserId(String);

fn setup_websocket(mut commands: Commands) {
    web_sys::console::log_1(&"asdf".into());
    commands.spawn_bundle(Camera2dBundle::default());

    // // TODO: room should be dynamic
    let room_id = "2g80ygbukgn65";
    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";

    let thread_pool = AsyncComputeTaskPool::get();

    let x = thread_pool.spawn_local(async move {

        web_sys::console::log_1(&"inside task".into());


        let login_result = login(app_id).await;
        let login_response = login_result.expect("Logging in should succeed");
        // web_sys::console::log_1(&login_response.token.into());

        let user_id = decode_user_id_without_validating_jwt(&login_response.token)
            .expect("Decoding JWT should succeed");

        // commands.insert_resource(UserId(user_id.claims.id));

        // let websocket_url = format!("wss://coordinator.hathora.dev/connect/{app_id}");

        // let (mut socket, _response) =
        //     connect(Url::parse(&websocket_url).unwrap()).expect("Can't connect");

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
    });


    
}

#[wasm_bindgen]
pub fn run() {
    console_error_panic_hook::set_once();

    // web_sys::console::log_1(&"aasdfasdfasdfsdf".into());

    App::new()
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup_websocket)
        // .add_system(bevy::window::close_on_esc)
        // .add_system(update_state)
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

fn update_state(
    // mut socket: ResMut<WebSocket<MaybeTlsStream<TcpStream>>>,
    client_user_id: Res<UserId>,
    mut camera_query: Query<(&Camera, &mut Transform), Without<UserId>>,
    mut query: Query<(Entity, &UserId, &mut Transform), Without<Camera>>,
    mut commands: Commands,
) {
    // let msg = socket.read_message().expect("Error reading message");

    // dbg!(client_user_id);

    // TODO: update camera to point at client_user_id

    // match msg {
    //     Message::Text(_) => todo!(),
    //     Message::Binary(data) => {
    //         if !data.is_empty() {
    //             let update: UpdateMessage =
    //                 serde_json::from_slice(&data).expect("Deserialize should work");

    //             let mut spawned: HashSet<String> = HashSet::new();

    //             for (entity, user_id, mut player_transform) in &mut query {
    //                 if &user_id.0 == &client_user_id.0 {
    //                     for (_camera, mut camera_transform) in &mut camera_query {
    //                         *camera_transform = Transform {
    //                             translation: Vec3::new(
    //                                 player_transform.translation.x,
    //                                 player_transform.translation.y,
    //                                 camera_transform.translation.z,
    //                             ),
    //                             ..*camera_transform
    //                         };
    //                     }
    //                 }

    //                 let mut found = false;
    //                 spawned.insert(user_id.0.clone());
    //                 for player in update.state.players.iter() {
    //                     if player.id == user_id.0 {
    //                         // dbg!("Updating {}", &player);
    //                         found = true;
    //                         player_transform.translation.x = player.position.x;
    //                         player_transform.translation.y = player.position.y;
    //                     }
    //                 }
    //                 if !found {
    //                     dbg!("Despawning {}", user_id);
    //                     commands.entity(entity).despawn();
    //                 }
    //             }

    //             for player in update.state.players.iter() {
    //                 if !spawned.contains(&player.id) {
    //                     dbg!("Spawning {}", &player.id);
    //                     commands
    //                         .spawn()
    //                         .insert(UserId(player.id.clone()))
    //                         .insert_bundle(SpriteBundle {
    //                             // TODO: update angle
    //                             transform: Transform {
    //                                 translation: Vec3::new(
    //                                     player.position.x,
    //                                     player.position.y,
    //                                     0.,
    //                                 ),
    //                                 ..default()
    //                             },
    //                             ..default()
    //                         });
    //                 }
    //             }
    //         }
    //     }
    //     Message::Ping(_) => {
    //         dbg!("Got ping");
    //     }
    //     Message::Pong(_) => todo!(),
    //     Message::Close(_) => todo!(),
    //     Message::Frame(_) => todo!(),
    // }
}
