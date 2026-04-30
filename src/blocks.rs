use crate::state::AppState;
use bevy::{asset::LoadState, image::TextureAtlasSources, prelude::*};
use std::collections::HashMap;

#[derive(Resource)]
pub struct BlockAtlas {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
    pub sources: TextureAtlasSources,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct NamespacedId {
    pub namespace: String,
    pub name: String,
}

impl NamespacedId {
    pub fn new(namespace: &str, name: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            name: name.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum BlockTextures {
    All(Handle<Image>),
    Sided {
        top: Handle<Image>,
        bottom: Handle<Image>,
        side: Handle<Image>,
    },
    None,
}

#[derive(Clone, Debug)]
pub struct BlockData {
    pub id: NamespacedId,
    pub textures: BlockTextures,
    pub is_solid: bool,
}

#[derive(Resource, Default)]
pub struct BlockRegistry {
    pub string_to_internal: HashMap<NamespacedId, u16>,
    pub internal_to_data: HashMap<u16, BlockData>,
    pub next_internal_id: u16,
}

impl BlockRegistry {
    pub fn register_block(&mut self, data: BlockData) -> u16 {
        let internal_id = self.next_internal_id;
        self.next_internal_id += 1;

        self.string_to_internal.insert(data.id.clone(), internal_id);
        self.internal_to_data.insert(internal_id, data.clone());

        info!(
            "Registered block: {}:{} as ID {}",
            data.id.namespace, data.id.name, internal_id
        );
        internal_id
    }

    pub fn get_internal_id(&self, id: &NamespacedId) -> Option<u16> {
        self.string_to_internal.get(id).copied()
    }

    pub fn get_data_by_internal(&self, internal_id: u16) -> Option<&BlockData> {
        self.internal_to_data.get(&internal_id)
    }
}

pub fn load_block_textures(namespace: &str, name: &str, asset_server: &AssetServer) -> BlockTextures {
    let asset_path = format!("{}/textures/block/{}", namespace, name);
    let physical_path = format!("assets/{}", asset_path);

    if std::path::Path::new(&format!("{}/all.png", physical_path)).exists() {
        BlockTextures::All(asset_server.load(format!("{}/all.png", asset_path)))
    } else if std::path::Path::new(&format!("{}/top.png", physical_path)).exists() {
        BlockTextures::Sided {
            top: asset_server.load(format!("{}/top.png", asset_path)),
            bottom: asset_server.load(format!("{}/bottom.png", asset_path)),
            side: asset_server.load(format!("{}/side.png", asset_path)),
        }
    } else {
        warn!(
            "No textures found for {}:{}. Defaulting to None.",
            namespace, name
        );
        BlockTextures::None
    }
}

pub fn setup_registry(mut registry: ResMut<BlockRegistry>, asset_server: Res<AssetServer>) {
    registry.register_block(BlockData {
        id: NamespacedId::new("superdupervoxels", "air"),
        textures: BlockTextures::None,
        is_solid: false,
    });

    registry.register_block(BlockData {
        id: NamespacedId::new("superdupervoxels", "dirt"),
        textures: load_block_textures("superdupervoxels", "dirt", &asset_server),
        is_solid: true,
    });

    registry.register_block(BlockData {
        id: NamespacedId::new("superdupervoxels", "grass"),
        textures: load_block_textures("superdupervoxels", "grass", &asset_server),
        is_solid: true,
    });
}

pub fn stitch_textures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    registry: Res<BlockRegistry>,
    mut images: ResMut<Assets<Image>>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut state: ResMut<NextState<AppState>>,
) {
    let mut all_handles = Vec::new();
    for block in registry.internal_to_data.values() {
        match &block.textures {
            BlockTextures::All(handle) => all_handles.push(handle.clone()),
            BlockTextures::Sided { top, bottom, side } => {
                all_handles.push(top.clone());
                all_handles.push(bottom.clone());
                all_handles.push(side.clone());
            }
            BlockTextures::None => {}
        }
    }

    for handle in &all_handles {
        let Some(load_state) = asset_server.get_load_state(handle) else {
            return;
        };
        match load_state {
            LoadState::Loaded => continue,
            LoadState::Failed(err) => {
                error!("A texture failed to load: {}", err);
                panic!("Texture loading failed. Check your assets/ folder paths!");
            }
            _ => return,
        }
    }

    info!("All textures loaded. Stitching atlas...");

    let mut binding = TextureAtlasBuilder::default();
    let builder = binding.padding(UVec2::new(2, 2));
    for handle in &all_handles {
        let Some(image) = images.get(handle) else {
            continue;
        };
        builder.add_texture(Some(handle.id()), image);
    }

    match builder.build() {
        Ok((atlas_layout, atlas_sources, atlas_image)) => {
            commands.insert_resource(BlockAtlas {
                image: images.add(atlas_image),
                layout: layouts.add(atlas_layout),
                sources: atlas_sources,
            });

            info!("Texture Atlas successfully created!");
            state.set(AppState::Playing);
        }
        Err(e) => error!("Failed to build texture atlas: {:?}", e),
    }
}