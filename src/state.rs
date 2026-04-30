use bevy::prelude::*;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Loading, // loading textures, block registries, etc.
    MainMenu, // the egui world selector
    Playing,  // in the game
}
