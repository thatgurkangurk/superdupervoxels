mod blocks;
mod chunk;
mod debug;
mod player;
mod state;
mod ui;
mod world;

use bevy::{prelude::*, window::PresentMode};
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use blocks::{BlockRegistry, BlockTextures, setup_registry, stitch_textures};
use chunk::Chunk;
use player::{camera_look, camera_movement, setup_crosshair, setup_environment, toggle_mouse_grab};
use state::AppState;

use crate::{
    chunk::remesh_chunks,
    debug::DebugUiPlugin,
    player::break_blocks,
    ui::{MenuState, despawn_menu_camera, main_menu_ui, setup_menu_camera},
    world::{manage_chunks, save_world_on_exit},
};

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
        EguiPlugin::default(),
    ))
    .init_state::<AppState>()
    .init_resource::<BlockRegistry>()
    .init_resource::<MenuState>()
    // --- STARTUP / LOADING PHASE ---
    .add_systems(Startup, (setup_registry, setup_menu_camera))
    .add_systems(Update, stitch_textures.run_if(in_state(AppState::Loading)))
    // --- MAIN MENU PHASE ---
    .add_systems(
        EguiPrimaryContextPass,
        main_menu_ui.run_if(in_state(AppState::MainMenu)),
    )
    // --- GAMEPLAY PHASE ---
    .add_systems(
        OnEnter(AppState::Playing),
        (despawn_menu_camera, setup_environment, setup_crosshair),
    )
    // gameplay loop
    .add_systems(
        Update,
        (
            reload_resources,
            camera_movement,
            camera_look,
            toggle_mouse_grab,
            break_blocks,
            remesh_chunks,
            manage_chunks,
        )
            .run_if(in_state(AppState::Playing)),
    )
    .add_systems(Last, save_world_on_exit.run_if(in_state(AppState::Playing)))
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
