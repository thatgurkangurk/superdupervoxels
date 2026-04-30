use bevy::input::mouse::MouseMotion;
use bevy::window::{CursorGrabMode, PrimaryWindow};
use bevy::{
    asset::{LoadState, RenderAssetUsages},
    image::TextureAtlasSources,
    mesh::Indices,
    prelude::*,
    render::render_resource::PrimitiveTopology,
    window::CursorOptions,
};
use std::collections::HashMap;

#[derive(Component)]
pub struct FlyCam {
    pub speed: f32,
    pub sensitivity: f32,
    pub pitch: f32,
    pub yaw: f32,
}

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Loading,
    Playing,
}

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
    // string_to_data: HashMap<NamespacedId, BlockData>, not needed right now, maybe at some point later on
    string_to_internal: HashMap<NamespacedId, u16>,
    internal_to_data: HashMap<u16, BlockData>,
    next_internal_id: u16,
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

pub const CHUNK_SIZE: usize = 16;

#[derive(Component)]
pub struct Chunk {
    pub blocks: [[[u16; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
}

impl Chunk {
    pub fn empty() -> Self {
        Self {
            blocks: [[[0; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
        }
    }
}

fn main() {
    let mut app = App::new();

    app.add_plugins((DefaultPlugins
        .set(ImagePlugin::default_nearest())
        .set(AssetPlugin {
            watch_for_changes_override: Some(true),
            ..default()
        }),))
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
            )
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(OnEnter(AppState::Playing), spawn_chunk)
        .run();
}

fn setup_crosshair(mut commands: Commands) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|parent| {
            parent.spawn((
                Node {
                    width: Val::Px(4.0),
                    height: Val::Px(4.0),
                    ..default()
                },
                // Color of the crosshair
                BackgroundColor(Color::WHITE),
            ));
        });
}

fn camera_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&FlyCam, &mut Transform)>,
) {
    for (cam, mut transform) in query.iter_mut() {
        let mut velocity = Vec3::ZERO;

        // Calculate the camera's local forward and right vectors based on its current rotation
        let forward = transform.rotation * Vec3::NEG_Z;
        let right = transform.rotation * Vec3::X;

        // wasd movement
        if keys.pressed(KeyCode::KeyW) {
            velocity += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            velocity -= forward;
        }
        if keys.pressed(KeyCode::KeyA) {
            velocity -= right;
        }
        if keys.pressed(KeyCode::KeyD) {
            velocity += right;
        }

        // up / down (locked to the global Y axis)
        if keys.pressed(KeyCode::Space) {
            velocity += Vec3::Y;
        }
        if keys.pressed(KeyCode::ShiftLeft) {
            velocity -= Vec3::Y;
        }

        // if the player is holding any keys, apply the movement
        if velocity.length_squared() > 0.0 {
            // normalise prevents the player from moving faster diagonally
            velocity = velocity.normalize();

            // apply speed and frame-time delta
            transform.translation += velocity * cam.speed * time.delta_secs();
        }
    }
}

fn camera_look(
    cursor_options: Query<&CursorOptions, With<PrimaryWindow>>,
    mut ev_motion: MessageReader<MouseMotion>,
    mut query: Query<(&mut FlyCam, &mut Transform)>,
) {
    let cursor = cursor_options
        .single()
        .expect("cursor options should exist");

    if cursor.grab_mode == CursorGrabMode::None {
        ev_motion.clear();
        return;
    }

    for (mut cam, mut transform) in query.iter_mut() {
        for ev in ev_motion.read() {
            cam.yaw -= ev.delta.x * cam.sensitivity;
            cam.pitch -= ev.delta.y * cam.sensitivity;
        }

        cam.pitch = cam.pitch.clamp(-1.54, 1.54);

        transform.rotation =
            Quat::from_axis_angle(Vec3::Y, cam.yaw) * Quat::from_axis_angle(Vec3::X, cam.pitch);
    }
}

fn toggle_mouse_grab(
    mut query: Query<(&mut Window, &mut CursorOptions), With<PrimaryWindow>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    let (mut window, mut cursor) = query.single_mut().expect("window and cursor should exist");

    if keys.just_pressed(KeyCode::Escape) {
        match cursor.grab_mode {
            CursorGrabMode::None => {
                cursor.grab_mode = CursorGrabMode::Locked;
                cursor.visible = false;
            }
            _ => {
                cursor.grab_mode = CursorGrabMode::None;
                cursor.visible = true;
            }
        }

        let center = Vec2::new(window.width() / 2.0, window.height() / 2.0);
        window.set_cursor_position(Some(center));
    }
}

fn setup_environment(mut commands: Commands) {
    // spawn camera once
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Transform::from_xyz(8.0, 16.0, 24.0).looking_at(Vec3::new(8.0, 4.0, 8.0), Vec3::Y),
        FlyCam {
            speed: 15.0,
            sensitivity: 0.002, // radians per pixel
            pitch: 0.0,
            yaw: 0.0,
        },
    ));

    // spawn sun once
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(8.0, 20.0, 8.0),
    ));
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

fn load_block_textures(namespace: &str, name: &str, asset_server: &AssetServer) -> BlockTextures {
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

fn setup_registry(mut registry: ResMut<BlockRegistry>, asset_server: Res<AssetServer>) {
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

fn stitch_textures(
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

fn spawn_chunk(
    mut commands: Commands,
    registry: Res<BlockRegistry>,
    atlas: Res<BlockAtlas>,
    images: Res<Assets<Image>>,
    layouts: Res<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut chunk = Chunk::empty();

    let dirt_id = registry
        .get_internal_id(&NamespacedId::new("superdupervoxels", "dirt"))
        .unwrap();
    let grass_id = registry
        .get_internal_id(&NamespacedId::new("superdupervoxels", "grass"))
        .unwrap();

    for x in 0..CHUNK_SIZE {
        for z in 0..CHUNK_SIZE {
            chunk.blocks[x][0][z] = dirt_id;
            chunk.blocks[x][1][z] = grass_id;
        }
    }

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut vertex_count = 0;

    let atlas_image = images.get(&atlas.image).unwrap();
    let atlas_size = Vec2::new(
        atlas_image.texture_descriptor.size.width as f32,
        atlas_image.texture_descriptor.size.height as f32,
    );
    let layout = layouts.get(&atlas.layout).unwrap();

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let block_id = chunk.blocks[x][y][z];
                if block_id == 0 {
                    continue;
                }

                let block_data = registry.get_data_by_internal(block_id).unwrap();

                let get_uvs = |face_name: &str| -> [[f32; 2]; 4] {
                    let handle = match &block_data.textures {
                        BlockTextures::All(h) => h,
                        BlockTextures::Sided { top, bottom, side } => match face_name {
                            "top" => top,
                            "bottom" => bottom,
                            _ => side,
                        },
                        BlockTextures::None => return [[0.; 2]; 4],
                    };

                    let texture_index = atlas.sources.texture_index(handle.id()).unwrap();
                    let rect = layout.textures[texture_index];

                    // exactly half a pixel in UV space
                    let half_pixel_x = 0.5 / atlas_size.x;
                    let half_pixel_y = 0.5 / atlas_size.y;

                    // apply the half pixel inset (no black lines between the blocks)
                    let min_x = (rect.min.x as f32 / atlas_size.x) + half_pixel_x;
                    let min_y = (rect.min.y as f32 / atlas_size.y) + half_pixel_y;
                    let max_x = (rect.max.x as f32 / atlas_size.x) - half_pixel_x;
                    let max_y = (rect.max.y as f32 / atlas_size.y) - half_pixel_y;

                    [
                        [min_x, max_y],
                        [max_x, max_y],
                        [max_x, min_y],
                        [min_x, min_y],
                    ]
                };

                let mut add_face = |face_positions: [[f32; 3]; 4],
                                    face_normal: [f32; 3],
                                    face_uvs: [[f32; 2]; 4]| {
                    positions.extend_from_slice(&face_positions);
                    normals.extend_from_slice(&[face_normal; 4]);
                    uvs.extend_from_slice(&face_uvs);

                    indices.extend_from_slice(&[
                        vertex_count,
                        vertex_count + 1,
                        vertex_count + 2,
                        vertex_count,
                        vertex_count + 2,
                        vertex_count + 3,
                    ]);
                    vertex_count += 4;
                };

                let fx = x as f32;
                let fy = y as f32;
                let fz = z as f32;

                if y == CHUNK_SIZE - 1 || chunk.blocks[x][y + 1][z] == 0 {
                    add_face(
                        [
                            [fx, fy + 1.0, fz + 1.0],
                            [fx + 1.0, fy + 1.0, fz + 1.0],
                            [fx + 1.0, fy + 1.0, fz],
                            [fx, fy + 1.0, fz],
                        ],
                        [0.0, 1.0, 0.0],
                        get_uvs("top"),
                    );
                }

                if y == 0 || chunk.blocks[x][y - 1][z] == 0 {
                    add_face(
                        [
                            [fx, fy, fz],
                            [fx + 1.0, fy, fz],
                            [fx + 1.0, fy, fz + 1.0],
                            [fx, fy, fz + 1.0],
                        ],
                        [0.0, -1.0, 0.0],
                        get_uvs("bottom"),
                    );
                }

                if x == CHUNK_SIZE - 1 || chunk.blocks[x + 1][y][z] == 0 {
                    add_face(
                        [
                            [fx + 1.0, fy, fz + 1.0],
                            [fx + 1.0, fy, fz],
                            [fx + 1.0, fy + 1.0, fz],
                            [fx + 1.0, fy + 1.0, fz + 1.0],
                        ],
                        [1.0, 0.0, 0.0],
                        get_uvs("side"),
                    );
                }

                if x == 0 || chunk.blocks[x - 1][y][z] == 0 {
                    add_face(
                        [
                            [fx, fy, fz],
                            [fx, fy, fz + 1.0],
                            [fx, fy + 1.0, fz + 1.0],
                            [fx, fy + 1.0, fz],
                        ],
                        [-1.0, 0.0, 0.0],
                        get_uvs("side"),
                    );
                }

                if z == CHUNK_SIZE - 1 || chunk.blocks[x][y][z + 1] == 0 {
                    add_face(
                        [
                            [fx, fy, fz + 1.0],
                            [fx + 1.0, fy, fz + 1.0],
                            [fx + 1.0, fy + 1.0, fz + 1.0],
                            [fx, fy + 1.0, fz + 1.0],
                        ],
                        [0.0, 0.0, 1.0],
                        get_uvs("side"),
                    );
                }

                if z == 0 || chunk.blocks[x][y][z - 1] == 0 {
                    add_face(
                        [
                            [fx + 1.0, fy, fz],
                            [fx, fy, fz],
                            [fx, fy + 1.0, fz],
                            [fx + 1.0, fy + 1.0, fz],
                        ],
                        [0.0, 0.0, -1.0],
                        get_uvs("side"),
                    );
                }
            }
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    commands.spawn((
        Mesh3d(meshes.add(mesh)),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(atlas.image.clone()),
            perceptual_roughness: 1.0,
            metallic: 0.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
        chunk,
    ));
}
