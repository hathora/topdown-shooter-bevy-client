use bevy::{
    asset::{AssetLoader, LoadedAsset},
    reflect::TypeUuid,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct Player {
    pub id: String,
    pub position: Position,
    pub aimAngle: f32,
}

#[derive(Deserialize, Debug)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

#[derive(Deserialize, Debug)]
pub struct Bullet {
    pub id: i32,
    pub position: Position,
}

#[derive(Deserialize, Debug)]
pub struct GameState {
    pub players: Vec<Player>,
    pub bullets: Vec<Bullet>,
}

#[derive(Deserialize, Debug)]
pub struct UpdateMessage {
    #[serde(rename = "type")]
    serialized_type: u64,
    ts: u64,
    pub state: GameState,
}

#[derive(Serialize)]
pub struct MoveInput {
    #[serde(rename = "type")]
    pub serialized_type: u64,
    pub direction: u64,
}

#[derive(Serialize)]
pub struct AngleInput {
    #[serde(rename = "type")]
    pub serialized_type: u64,
    pub angle: f32,
}

#[derive(Serialize)]
pub struct ClickInput {
    #[serde(rename = "type")]
    pub serialized_type: u64,
}

#[derive(Default)]
pub struct MapLoader;

#[derive(Deserialize, Debug)]
pub struct Wall {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Deserialize, TypeUuid, Debug)]
#[uuid = "39cadc56-aa9c-4543-8640-a018b74b5052"]
pub struct MapAsset {
    pub tileSize: i32,
    pub top: i32,
    pub left: i32,
    pub bottom: i32,
    pub right: i32,
    pub walls: Vec<Wall>,
}

impl AssetLoader for MapLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut bevy::asset::LoadContext,
    ) -> bevy::utils::BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let map = serde_json::from_slice::<MapAsset>(bytes)?;
            load_context.set_default_asset(LoadedAsset::new(map));
            Ok(())
        })
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}
