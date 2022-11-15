use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use bevy::{input::mouse::MouseMotion, prelude::*, render::camera::RenderTarget};
use clipboard::{ClipboardContext, ClipboardProvider};
use hathora_client_sdk::{HathoraClient, HathoraTransport};

use crate::{
    components::{BulletId, CurrentPlayer, InterpolationBuffer, MainCamera, UserId},
    serialization::{AngleInput, ClickInput, MapAsset, MoveInput, UpdateMessage},
    ProvidedAppId, ProvidedRoomId,
};

pub struct RoomId(String);

pub fn log_in_and_set_up_transport(
    provided_room_id: Res<ProvidedRoomId>,
    provided_app_id: Res<ProvidedAppId>,
    mut commands: Commands,
) {
    let app_id = provided_app_id
        .0
        .clone()
        .unwrap_or("e2d8571eb89af72f2abbe909def5f19bc4dad0cd475cce5f5b6e9018017d1f1c".to_string());

    let hathora_client = HathoraClient::new(app_id, None);

    let login_result = hathora_client.login_anonymous();
    let token = login_result.expect("Logging in should succeed");

    let room_id = provided_room_id.0.clone().or_else(|| {
        debug!("No room provided, creating one");
        match hathora_client.create(&token, vec![]) {
            Ok(create_response) => Some(create_response),
            Err(e) => {
                error!("Failed to create a room. Error was {}", e);
                None
            }
        }
    });
    let room_id = room_id.expect("Room ID exists");
    commands.insert_resource(RoomId(room_id.to_owned()));

    let user_id = HathoraClient::get_user_from_token(&token).expect("Decoding JWT should succeed");
    commands.insert_resource(UserId(user_id));
    let transport = hathora_client
        .connect(
            &token,
            &room_id,
            hathora_client_sdk::HathoraTransportType::WebSocket,
        )
        .expect("Creating web socket should work.");
    commands.insert_resource(transport);
}

pub fn setup_camera(mut commands: Commands) {
    commands
        .spawn_bundle(Camera2dBundle::default())
        .insert(MainCamera);
}

pub struct ButtonTimer(Timer);
const CLEAR: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
const NORMAL_BUTTON: Color = Color::rgb(0.80, 0.80, 0.80);
const HOVERED_BUTTON: Color = Color::rgb(0.90, 0.90, 0.90);
const PRESSED_BUTTON: Color = Color::WHITE;

pub fn display_room_id(
    asset_server: Res<AssetServer>,
    mut commands: Commands,
    room_id: Res<RoomId>,
) {
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
                        size: Size::new(Val::Px(500.0), Val::Px(100.0)),
                        ..default()
                    },
                    color: CLEAR.into(),
                    ..default()
                })
                .with_children(|parent| {
                    // left vertical fill (content)
                    parent
                        .spawn_bundle(NodeBundle {
                            style: Style {
                                size: Size::new(Val::Px(490.0), Val::Percent(100.0)),
                                ..default()
                            },
                            color: CLEAR.into(),
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
                                    margin: UiRect::all(Val::Auto),
                                    ..default()
                                },
                                image: asset_server.load("icons/content-copy.png").into(),
                                color: NORMAL_BUTTON.into(),
                                ..default()
                            });
                        });
                });
        });

    commands.insert_resource(ButtonTimer(Timer::new(Duration::from_secs(1), false)))
}

pub struct LoadedMap(Handle<MapAsset>, bool);

pub fn load_map(asset_server: Res<AssetServer>, mut commands: Commands) {
    let map_loading = asset_server.load("data/map.json");
    commands.insert_resource(LoadedMap(map_loading, false));
}

pub fn draw_map(
    asset_server: Res<AssetServer>,
    mut loaded_map: ResMut<LoadedMap>,
    mut commands: Commands,
    map_assets: ResMut<Assets<MapAsset>>,
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

pub fn copy_room_id_button(
    mut interaction_query: Query<(&Interaction, &mut UiColor)>,
    mut text_query: Query<&mut Text>,
    mut mouse_button_input: ResMut<Input<MouseButton>>,
    room_id: Res<RoomId>,

    mut button_timer: ResMut<ButtonTimer>,
    time: Res<Time>,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Clicked => {
                debug!("Button clicked");
                mouse_button_input.clear_just_pressed(MouseButton::Left);
                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                ctx.set_contents(room_id.0.to_owned()).unwrap();
                *color = PRESSED_BUTTON.into();

                text_query.single_mut().sections[0].value = "Copied to clipboard".to_string();
                button_timer.0.reset();
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

    button_timer.0.tick(time.delta());

    if button_timer.0.finished() {
        text_query.single_mut().sections[0].value = format!("Room ID: {}", room_id.0);
    }
}

pub fn read_from_server(
    mut connection: ResMut<Box<dyn HathoraTransport>>,
    client_user_id: Res<UserId>,
    mut player_query: Query<
        (Entity, &UserId, &mut InterpolationBuffer),
        (Without<Camera>, Without<BulletId>),
    >,
    mut bullet_query: Query<
        (Entity, &BulletId, &mut Transform),
        (Without<Camera>, Without<UserId>),
    >,

    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    match connection.read_message() {
        Ok(data) => {
            debug!("got some data!");
            if !data.is_empty() {
                let update: UpdateMessage =
                    serde_json::from_slice(&data).expect("Deserialize should work");

                let mut spawned_players: HashSet<String> = HashSet::new();

                for (entity, user_id, mut interpolation_buffer) in &mut player_query {
                    let mut found = false;
                    spawned_players.insert(user_id.0.clone());
                    for player_update in update.state.players.iter() {
                        if player_update.id == user_id.0 {
                            debug!("Updating {:?}", &player_update);
                            found = true;

                            interpolation_buffer.0.push_back(Transform {
                                translation: Vec3::new(
                                    player_update.position.x,
                                    -player_update.position.y,
                                    0.,
                                ),
                                rotation: Quat::from_rotation_z(-player_update.aimAngle),
                                ..default()
                            });
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
                            })
                            .insert(InterpolationBuffer(VecDeque::new()));

                        if player_update.id == client_user_id.0 {
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
                            .insert(BulletId(bullet_update.id))
                            .insert_bundle(SpriteBundle {
                                texture: asset_server.load("sprites/bullet.png"),
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
        Err(e) => {
            debug!("Error in stream: {}", e);
        }
    }
}

// determines how quickly our interpolation converges
const LAMBDA: f32 = 1.;

pub fn update_position_from_interpolation_buffer(
    mut buffer_query: Query<(&mut InterpolationBuffer, &mut Transform), Without<BulletId>>,

    time: Res<Time>,
) {
    let delta = (LAMBDA * time.delta_seconds()).max(1.0);

    for (mut buffer, mut player_transform) in &mut buffer_query {
        if let Some(updated_position) = buffer.0.get(0) {
            debug!("Updating position by {}", delta);
            player_transform.translation = player_transform
                .translation
                .lerp(updated_position.translation, delta);

            player_transform.rotation = player_transform
                .rotation
                .lerp(updated_position.rotation, delta);

            if player_transform
                .translation
                .distance(updated_position.translation)
                < f32::EPSILON
            {
                debug!("Done processing update");
                buffer.0.pop_front();
            }
        }
    }
}

pub fn write_inputs(
    input: Res<Input<KeyCode>>,
    query: Query<(&CurrentPlayer, &Transform)>,
    windows: Res<Windows>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mouse_motion_events: EventReader<MouseMotion>,
    mouse_button_input: Res<Input<MouseButton>>,

    mut transport: ResMut<Box<dyn HathoraTransport>>,
) {
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
        if let Err(e) = transport.write_message(message) {
            warn!("Transport failed to write, error was {}", e);
        }
    }

    if mouse_button_input.just_pressed(MouseButton::Left) {
        debug!("Mouse clicked.");
        let mouse_input = ClickInput { serialized_type: 2 };
        let message = serde_json::to_vec(&mouse_input).expect("Serialization should work");
        if let Err(e) = transport.write_message(message) {
            warn!("Transport failed to write, error was {}", e);
        }
    }

    if !mouse_motion_events.is_empty() {
        debug!("Processing mouse input");
        let (camera, camera_transform) = camera_query.single();
        let window = if let RenderTarget::Window(id) = camera.target {
            windows.get(id).unwrap()
        } else {
            windows.get_primary().unwrap()
        };
        if let Some(cursor_screen_position) = window.cursor_position() {
            let window_size = Vec2::new(window.width() as f32, window.height() as f32);
            // convert screen position [0..resolution] to ndc [-1..1] (gpu coordinates)
            let ndc = (cursor_screen_position / window_size) * 2.0 - Vec2::ONE;
            // matrix for undoing the projection and camera transform
            let ndc_to_world =
                camera_transform.compute_matrix() * camera.projection_matrix().inverse();
            // use it to convert ndc to world-space coordinates
            let world_pos = ndc_to_world.project_point3(ndc.extend(-1.0));

            // reduce it to a 2D value
            let cursor_world_position: Vec2 = world_pos.truncate();

            for (_, player_transform) in query.iter() {
                let angle = (cursor_world_position - player_transform.translation.truncate())
                    .angle_between(Vec2::X);
                debug!("Angle {}", angle);

                let mouse_input = AngleInput {
                    serialized_type: 1,
                    angle,
                };

                let message = serde_json::to_vec(&mouse_input).expect("Serialization should work");
                if let Err(e) = transport.write_message(message) {
                    warn!("Transport failed to write, error was {}", e);
                }
            }
        }
    }
}

pub fn update_camera(
    current_player_query: Query<&Transform, (With<CurrentPlayer>, Without<Camera>)>,
    mut camera_query: Query<(&Camera, &mut Transform), (With<Camera>, Without<CurrentPlayer>)>,

    map_assets: ResMut<Assets<MapAsset>>,
    loaded_map: ResMut<LoadedMap>,
) {
    let (camera, mut camera_transform) = camera_query.single_mut();

    // can't use single here; CurrentPlayer might not have spawned yet
    for player_transform in &current_player_query {
        camera_transform.translation = player_transform.translation;
    }

    if let Some(map) = map_assets.get(&loaded_map.0) {
        let min_gpu = Vec3::splat(-1.);
        let to_world = camera_transform.compute_matrix() * camera.projection_matrix().inverse();
        let camera_min = to_world.project_point3(min_gpu);
        let max_gpu = Vec3::splat(1.);
        let camera_max = to_world.project_point3(max_gpu);

        let map_min_x = (map.tileSize * map.left) as f32;
        if (camera_min.x) < map_min_x {
            camera_transform.translation.x += map_min_x - camera_min.x;
        }
        let map_max_x = (map.tileSize * map.right) as f32;
        if (camera_max.x) > map_max_x {
            camera_transform.translation.x -= (camera_max.x) - map_max_x;
        }
        let map_min_y = -(map.tileSize * map.bottom) as f32;
        if (camera_min.y) < map_min_y {
            camera_transform.translation.y += map_min_y - camera_min.y;
        }
        let map_max_y = -(map.tileSize * map.top) as f32;
        if (camera_max.y) > map_max_y {
            camera_transform.translation.y += map_max_y - camera_max.y;
        }
    }
}
