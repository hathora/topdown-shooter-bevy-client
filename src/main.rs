use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;

use bevy::reflect::TypeUuid;
use bevy::render::camera::RenderTarget;

use clap::Parser;
use clipboard::{ClipboardContext, ClipboardProvider};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use std::iter::Iterator;
use std::net::TcpStream;

use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};

#[derive(Deserialize, Debug, Clone)]
struct LoginResponse {
    token: String,
}

#[derive(Serialize, Debug)]
struct InitialState {
    token: String,
    stateId: String,
}

#[derive(Deserialize)]
struct CreateRoomResponse {
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
    let _id = segments[1];

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

fn setup_camera(mut commands: Commands) {
    commands
        .spawn_bundle(Camera2dBundle::default())
        .insert(MainCamera);
}

#[derive(Parser)]
struct Args {
    room_id: Option<String>,
}

struct RoomId(String);

fn create_room(app_id: &str, token: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::new();
    let create_url = format!("https://coordinator.hathora.dev/{app_id}/create");

    let response: CreateRoomResponse = client
        .post(create_url)
        .header(AUTHORIZATION, token)
        .header(CONTENT_TYPE, "application/octet-stream")
        .body(vec![])
        .send()?
        .json()?;

    info!("Created room {}", response.stateId);

    Ok(response.stateId)
}

fn main() {
    let args = Args::parse();

    let app_id = "e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c";
    // let app_id = "app-e623314c-c28b-4c9c-a623-c02c7efc25c6";
    let login_result = login(app_id);
    let login_response = login_result.expect("Logging in should succeed");

    let room_id = args.room_id.or_else(|| {
        dbg!("No room provided, creating one");
        match create_room(app_id, &login_response.token) {
            Ok(create_response) => Some(create_response),
            Err(e) => {
                dbg!("Failed to create a room. Error was {}", e);
                None
            }
        }
    });

    let room_id = room_id.expect("Room ID exists");

    let user_id = decode_user_id_without_validating_jwt(&login_response.token)
        .expect("Decoding JWT should succeed");

    let websocket_url = format!("wss://coordinator.hathora.dev/connect/{app_id}");
    let (mut socket, _response) =
        connect(Url::parse(&websocket_url).unwrap()).expect("Can't connect to websockets");

    let initial_state = InitialState {
        token: login_response.token,
        stateId: room_id.to_owned(),
    };
    let message = serde_json::to_string(&initial_state).expect("Serialization should work");
    use encoding::Encoding;
    let bytes = encoding::all::UTF_8
        .encode(&message, encoding::EncoderTrap::Strict)
        .expect("utf8 encoding");

    match socket.write_message(Message::binary(bytes)) {
        Ok(_) => {}
        Err(e) => {
            dbg!("Failed to connect to websocket. Error was {}", e);
        }
    }

    App::new()
        .insert_resource(WindowDescriptor {
            title: "bevy-topdown-shooter".to_string(),
            width: 800.,
            height: 600.,
            ..default()
        })
        .add_plugins_with(DefaultPlugins, |group| group)
        .add_asset::<Map>()
        .init_asset_loader::<MapLoader>()
        .insert_resource(socket)
        .insert_resource(UserId(user_id))
        .insert_resource(RoomId(room_id))
        .insert_resource(MouseLocation(Vec2::ZERO))
        .add_startup_system(setup_camera)
        .add_startup_system(display_room_id)
        .add_startup_system(load_map)
        .add_system(draw_map)
        .add_system(bevy::window::close_on_esc)
        .add_system(copy_room_id)
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

    loaded_map: ResMut<LoadedMap>,
    map_assets: ResMut<Assets<Map>>,
) {
    match socket.read_message() {
        Ok(msg) => {
            debug!("got some data!");

            match msg {
                Message::Text(_) => {
                    info!("Got text");
                }
                Message::Binary(data) => {
                    debug!("Got binary");

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
                                    player_transform.rotation =
                                        Quat::from_rotation_z(-player_update.aimAngle)
                                }
                            }

                            if &user_id.0 == &client_user_id.0 {
                                for (camera, mut camera_transform) in &mut camera_query {
                                    *camera_transform = Transform {
                                        translation: Vec3::new(
                                            player_transform.translation.x,
                                            player_transform.translation.y,
                                            camera_transform.translation.z,
                                        ),
                                        ..*camera_transform
                                    };

                                    if let Some(map) = map_assets.get(&loaded_map.0) {
                                        let min_gpu = Vec3::splat(-1.);
                                        let to_world = camera_transform.compute_matrix()
                                            * camera.projection_matrix().inverse();
                                        let camera_min = to_world.project_point3(min_gpu);
                                        let max_gpu = Vec3::splat(1.);
                                        let camera_max = to_world.project_point3(max_gpu);

                                        let map_min_x = (map.tileSize * map.left) as f32;
                                        if (camera_min.x) < map_min_x {
                                            camera_transform.translation.x +=
                                                map_min_x - camera_min.x;
                                        }
                                        let map_max_x = (map.tileSize * map.right) as f32;
                                        if (camera_max.x) > map_max_x {
                                            camera_transform.translation.x -=
                                                (camera_max.x) - map_max_x;
                                        }
                                        let map_min_y = -(map.tileSize * map.bottom) as f32;
                                        if (camera_min.y) < map_min_y {
                                            camera_transform.translation.y +=
                                                map_min_y - camera_min.y;
                                        }
                                        let map_max_y = -(map.tileSize * map.top) as f32;
                                        if (camera_max.y) > map_max_y {
                                            camera_transform.translation.y +=
                                                map_max_y - camera_max.y;
                                        }
                                    }
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
                                            rotation: Quat::from_rotation_z(
                                                -player_update.aimAngle,
                                            ),
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
                    info!("Got ping");
                }
                Message::Pong(_) => {
                    info!("Got pong");
                }
                Message::Close(_) => {
                    info!("Got close");
                }
                Message::Frame(_) => {
                    info!("Got frame");
                }
            }
        }
        Err(e) => {
            debug!("Error in stream: {}", e);
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
    query: Query<(&CurrentPlayer, &Transform)>,
    // need to get window dimensions
    wnds: Res<Windows>,
    q_camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut mouse_location: ResMut<MouseLocation>,
    mouse_motion_events: EventReader<MouseMotion>,
    mouse_button_input: Res<Input<MouseButton>>,

    mut socket: ResMut<WebSocket<MaybeTlsStream<TcpStream>>>,
) {
    if mouse_button_input.just_pressed(MouseButton::Left) {
        debug!("Mouse clicked.");
        let mouse_input = ClickInput { serialized_type: 2 };
        let message = serde_json::to_vec(&mouse_input).expect("Serialization should work");
        if let Err(e) = socket.write_message(Message::Binary(message)) {
            warn!("Socket failed to write, error was {}", e);
        }
    }

    debug!("Processing keyboard input");
    if input.any_just_released([KeyCode::W, KeyCode::A, KeyCode::S, KeyCode::D])
        || input.any_just_pressed([KeyCode::W, KeyCode::A, KeyCode::S, KeyCode::D])
    {
        let mut direction = 0;

        if input.pressed(KeyCode::W) {
            direction = 1
        } else if input.pressed(KeyCode::S) {
            direction = 2;
        } else if input.pressed(KeyCode::A) {
            direction = 3;
        } else if input.pressed(KeyCode::D) {
            direction = 4;
        }

        if input.just_pressed(KeyCode::W) {
            direction = 1
        } else if input.just_pressed(KeyCode::S) {
            direction = 2;
        } else if input.just_pressed(KeyCode::A) {
            direction = 3;
        } else if input.just_pressed(KeyCode::D) {
            direction = 4;
        }

        let input = MoveInput {
            serialized_type: 0,
            direction,
        };

        let message = serde_json::to_vec(&input).expect("Serialization should work");
        if let Err(e) = socket.write_message(Message::Binary(message)) {
            warn!("Socket failed to write, error was {}", e);
        }
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

            info!("Mouse coords: {}/{}", world_pos.x, world_pos.y);

            for (_, player_transform) in query.iter() {
                let angle =
                    (world_pos - player_transform.translation.truncate()).angle_between(Vec2::X);
                debug!("Angle {}", angle);

                let mouse_input = AngleInput {
                    serialized_type: 1,
                    angle,
                };

                let message = serde_json::to_vec(&mouse_input).expect("Serialization should work");
                if let Err(e) = socket.write_message(Message::Binary(message)) {
                    warn!("Socket failed to write, error was {}", e);
                }
            }

            // todo: remove this
            *mouse_location = MouseLocation(world_pos);
        }
    }
}

use bevy::asset::{AssetLoader, LoadedAsset};

#[derive(Default)]
struct MapLoader {}

#[derive(Deserialize, Debug)]
struct Wall {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Deserialize, TypeUuid, Debug)]
#[uuid = "39cadc56-aa9c-4543-8640-a018b74b5052"]
struct Map {
    tileSize: i32,
    top: i32,
    left: i32,
    bottom: i32,
    right: i32,
    walls: Vec<Wall>,
}

impl AssetLoader for MapLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let map = serde_json::from_slice::<Map>(bytes)?;
            load_context.set_default_asset(LoadedAsset::new(map));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

struct LoadedMap(Handle<Map>, bool);

fn load_map(asset_server: Res<AssetServer>, mut commands: Commands) {
    let map_loading = asset_server.load("data/map.json");
    commands.insert_resource(LoadedMap(map_loading, false));
}

fn draw_map(
    asset_server: Res<AssetServer>,
    mut loaded_map: ResMut<LoadedMap>,
    mut commands: Commands,
    map_assets: ResMut<Assets<Map>>,
) {
    let map_asset = map_assets.get(&loaded_map.0);

    if map_asset.is_none() || loaded_map.1 {
        return;
    }

    let map = map_asset.expect("Verified that map isn't None");

    debug!("Custom asset loaded: {:?}", map);
    loaded_map.1 = true;

    for wall in &map.walls {
        for x in 0..wall.width {
            for y in 0..wall.height {
                let dx = 0.5 + x as f32;
                let dy = 0.5 + y as f32;

                commands.spawn().insert_bundle({
                    SpriteBundle {
                        texture: asset_server.load("sprites/wall.png"),
                        transform: Transform {
                            translation: Vec3::new(
                                map.tileSize as f32 * (wall.x as f32 + dx),
                                -map.tileSize as f32 * (wall.y as f32 + dy),
                                0.,
                            ),
                            ..default()
                        },
                        ..default()
                    }
                });
            }
        }
    }
}

fn display_room_id(asset_server: Res<AssetServer>, mut commands: Commands, room_id: Res<RoomId>) {
    commands
        .spawn_bundle(NodeBundle {
            style: Style {
                size: Size::new(Val::Percent(100.0), Val::Percent(100.0)),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            color: Color::NONE.into(),
            ..default()
        })
        .with_children(|parent| {
            // left vertical fill (border)
            parent
                .spawn_bundle(NodeBundle {
                    style: Style {
                        size: Size::new(Val::Px(600.0), Val::Px(100.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        ..default()
                    },
                    color: Color::rgb(0.65, 0.65, 0.65).into(),
                    ..default()
                })
                .with_children(|parent| {
                    // left vertical fill (content)
                    parent
                        .spawn_bundle(NodeBundle {
                            style: Style {
                                size: Size::new(Val::Percent(80.0), Val::Percent(100.0)),
                                align_items: AlignItems::FlexEnd,
                                ..default()
                            },
                            color: Color::rgb(0.15, 0.15, 0.15).into(),
                            ..default()
                        })
                        .with_children(|parent| {
                            // text
                            parent.spawn_bundle(
                                TextBundle::from_section(
                                    format!("Room ID: {}", room_id.0),
                                    TextStyle {
                                        font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                                        font_size: 30.0,
                                        color: Color::WHITE,
                                    },
                                )
                                .with_style(Style {
                                    margin: UiRect::all(Val::Px(5.0)),
                                    align_self: AlignSelf::Center,
                                    ..default()
                                }),
                            );

                            parent.spawn_bundle(ButtonBundle {
                                style: Style {
                                    size: Size::new(Val::Px(50.0), Val::Px(50.0)),
                                    // center button
                                    margin: UiRect::all(Val::Auto),
                                    // horizontally center child text
                                    // justify_content: JustifyContent::Center,
                                    // vertically center child text
                                    // align_items: AlignItems::Center,
                                    ..default()
                                },
                                image: asset_server.load("icons/content-copy.png").into(),
                                color: NORMAL_BUTTON.into(),
                                ..default()
                            });
                        });
                });
        });
}

const NORMAL_BUTTON: Color = Color::rgb(0.80, 0.80, 0.80);
const HOVERED_BUTTON: Color = Color::rgb(0.90, 0.90, 0.90);
const PRESSED_BUTTON: Color = Color::WHITE;

fn copy_room_id(mut interaction_query: Query<(&Interaction, &mut UiColor)>, room_id: Res<RoomId>) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Clicked => {
                debug!("Button clicked");
                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                ctx.set_contents(room_id.0.to_owned()).unwrap();
                *color = PRESSED_BUTTON.into();
            }
            Interaction::Hovered => {
                debug!("Button clicked");
                *color = HOVERED_BUTTON.into();
            }
            Interaction::None => {
                debug!("No interaction");
                *color = NORMAL_BUTTON.into();
            }
        }
    }
}
