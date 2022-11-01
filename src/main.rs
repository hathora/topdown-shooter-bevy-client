use bevy::prelude::*;

use clap::Parser;

use serialization::{MapAsset, MapLoader};
use systems::*;

mod components;
mod serialization;
mod systems;

#[derive(Parser)]
struct Args {
    room_id: Option<String>,

    #[arg(short, long)]
    app_id: Option<String>,
}

pub struct ProvidedRoomId(Option<String>);
pub struct ProvidedAppId(Option<String>);

fn main() {
    let args = Args::parse();

    App::new()
        .insert_resource(WindowDescriptor {
            width: 800.,
            height: 600.,
            title: "bevy-topdown-shooter".to_string(),
            resizable: false,
            ..default()
        })
        .add_plugins(DefaultPlugins)
        .add_asset::<MapAsset>()
        .init_asset_loader::<MapLoader>()
        .insert_resource(ProvidedRoomId(args.room_id))
        .insert_resource(ProvidedAppId(args.app_id))
        // This is exclusive so we can guarantee that the room is created before
        // we render the room ID
        .add_startup_system(log_in_and_set_up_websocket.exclusive_system())
        .add_startup_system(setup_camera)
        .add_startup_system(display_room_id)
        .add_startup_system(load_map)
        // general systems
        .add_system(bevy::window::close_on_esc)
        .add_system(draw_map)
        .add_system(copy_room_id_button)
        // game state systems
        .add_system(read_from_server)
        .add_system(update_position_from_interpolation_buffer.after(read_from_server))
        .add_system(
            write_inputs
                .after(read_from_server)
                .after(copy_room_id_button),
        )
        .add_system(update_camera.after(update_position_from_interpolation_buffer))
        .run();
}
