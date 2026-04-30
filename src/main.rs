mod blocks;
mod chunk;
mod debug;
mod player;
mod state;

use bevy::{prelude::*, window::PresentMode};
use blocks::{BlockRegistry, BlockTextures, setup_registry, stitch_textures};
use chunk::{Chunk, spawn_chunk};
use player::{camera_look, camera_movement, setup_crosshair, setup_environment, toggle_mouse_grab};
use state::AppState;

use crate::{chunk::remesh_chunks, debug::DebugUiPlugin, player::break_blocks};

fn main() {
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(AssetPlugin {
                watch_for_changes_override: Some(true),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    present_mode: PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            }),
        DebugUiPlugin,
    ))
    .init_state::<AppState>()
    .init_resource::<BlockRegistry>()
    .add_systems(
        Startup,
        (setup_registry, setup_environment, setup_crosshair),
    )
    .add_systems(Update, stitch_textures.run_if(in_state(AppState::Loading)))
    .add_systems(
        Update,
        (
            reload_resources,
            camera_movement,
            camera_look,
            toggle_mouse_grab,
            break_blocks,
            remesh_chunks,
        )
            .run_if(in_state(AppState::Playing)),
    )
    .add_systems(OnEnter(AppState::Playing), spawn_chunk)
    .run();
}

fn reload_resources(
    keys: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut commands: Commands,
    chunk_query: Query<Entity, With<Chunk>>,
    registry: Res<BlockRegistry>,
    asset_server: Res<AssetServer>,
) {
    if keys.just_pressed(KeyCode::F5) {
        info!("reloading textures");

        // ignore cached textures
        for block in registry.internal_to_data.values() {
            match &block.textures {
                BlockTextures::All(handle) => {
                    asset_server.reload(handle.path().expect("no texture"));
                }
                BlockTextures::Sided { top, bottom, side } => {
                    asset_server.reload(top.path().expect("no texture"));
                    asset_server.reload(bottom.path().expect("no texture"));
                    asset_server.reload(side.path().expect("no texture"));
                }
                BlockTextures::None => {}
            }
        }

        // delete all chunks
        for entity in chunk_query.iter() {
            commands.entity(entity).despawn_children();
            commands.entity(entity).despawn();
        }

        // send the game to the loading state so that it generates the new atlas
        next_state.set(AppState::Loading);
    }
}
