use crate::state::AppState;
use crate::world::ChunkManager;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use std::fs;
use crate::consts::VERSION;

#[derive(Resource, Default)]
pub struct MenuState {
    pub new_world_name: String,
}

#[derive(Component)]
pub struct MenuCamera;

pub fn setup_menu_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MenuCamera,
    ));
}

pub fn despawn_menu_camera(mut commands: Commands, query: Query<Entity, With<MenuCamera>>) {
    for entity in query.iter() {
        commands.entity(entity).despawn_children();
        commands.entity(entity).despawn();
    }
}

pub fn main_menu_ui(
    mut contexts: EguiContexts,
    mut next_state: ResMut<NextState<AppState>>,
    mut commands: Commands,
    mut menu_state: ResMut<MenuState>,
) {
    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    egui::Window::new(format!("superdupervoxels {VERSION}"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.heading("select a world");
            ui.separator();

            // read the 'worlds' folder and create a button for each world
            // note: this doesn't check if they actually are valid worlds but MEH.
            if let Ok(entries) = fs::read_dir("worlds") {
                for entry in entries.flatten() {
                    // only look at directories
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            let world_name = entry.file_name().into_string().unwrap_or_default();

                            // if the button is clicked, load this world!
                            if ui.button(format!("load '{}'", world_name)).clicked() {
                                commands.insert_resource(ChunkManager::new(&world_name));
                                next_state.set(AppState::Playing);
                            }
                        }
                    }
                }
            }

            ui.add_space(20.0);
            ui.heading("or create a new one");
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("World Name:");
                ui.text_edit_singleline(&mut menu_state.new_world_name);
            });

            ui.add_enabled_ui(!menu_state.new_world_name.trim().is_empty(), |ui| {
                if ui.button("create & play").clicked() {
                    commands.insert_resource(ChunkManager::new(menu_state.new_world_name.trim()));
                    next_state.set(AppState::Playing);
                }
            });
        });
}
