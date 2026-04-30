use crate::blocks::{BlockAtlas, BlockRegistry, BlockTextures, NamespacedId};
use bevy::{
    asset::RenderAssetUsages, mesh::Indices, prelude::*,
    render::render_resource::PrimitiveTopology,
};

pub const CHUNK_SIZE: usize = 16;

#[derive(Component)]
pub struct Chunk {
    pub blocks: [[[u16; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
}

/// marker component to tell that this chunk needs a new mesh
#[derive(Component)]
pub struct NeedsRemesh;

impl Chunk {
    pub fn empty() -> Self {
        Self {
            blocks: [[[0; CHUNK_SIZE]; CHUNK_SIZE]; CHUNK_SIZE],
        }
    }
}

pub fn spawn_chunk(mut commands: Commands, registry: Res<BlockRegistry>) {
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

    // spawn the data and tag it with NeedsRemesh
    commands.spawn((
        chunk,
        Transform::from_xyz(0.0, 0.0, 0.0),
        NeedsRemesh,
    ));
}

pub fn remesh_chunks(
    mut commands: Commands,
    query: Query<(Entity, &Chunk), With<NeedsRemesh>>,
    registry: Res<BlockRegistry>,
    atlas: Res<BlockAtlas>,
    images: Res<Assets<Image>>,
    layouts: Res<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut existing_meshes: Query<&mut Mesh3d>,
) {
    let atlas_image = images.get(&atlas.image).unwrap();
    let atlas_size = Vec2::new(
        atlas_image.texture_descriptor.size.width as f32,
        atlas_image.texture_descriptor.size.height as f32,
    );
    let layout = layouts.get(&atlas.layout).unwrap();

    for (entity, chunk) in query.iter() {
        // Remove the tag so we don't remesh every frame!
        commands.entity(entity).remove::<NeedsRemesh>();

        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut vertex_count = 0;

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

                        let half_pixel_x = 0.5 / atlas_size.x;
                        let half_pixel_y = 0.5 / atlas_size.y;

                        let min_x = (rect.min.x as f32 / atlas_size.x) + half_pixel_x;
                        let min_y = (rect.min.y as f32 / atlas_size.y) + half_pixel_y;
                        let max_x = (rect.max.x as f32 / atlas_size.x) - half_pixel_x;
                        let max_y = (rect.max.y as f32 / atlas_size.y) - half_pixel_y;

                        [[min_x, max_y], [max_x, max_y], [max_x, min_y], [min_x, min_y]]
                    };

                    let mut add_face = |face_positions: [[f32; 3]; 4],
                                        face_normal: [f32; 3],
                                        face_uvs: [[f32; 2]; 4]| {
                        positions.extend_from_slice(&face_positions);
                        normals.extend_from_slice(&[face_normal; 4]);
                        uvs.extend_from_slice(&face_uvs);

                        indices.extend_from_slice(&[
                            vertex_count, vertex_count + 1, vertex_count + 2, 
                            vertex_count, vertex_count + 2, vertex_count + 3,
                        ]);
                        vertex_count += 4;
                    };

                    let fx = x as f32;
                    let fy = y as f32;
                    let fz = z as f32;

                    if y == CHUNK_SIZE - 1 || chunk.blocks[x][y + 1][z] == 0 {
                        add_face([[fx, fy + 1.0, fz + 1.0], [fx + 1.0, fy + 1.0, fz + 1.0], [fx + 1.0, fy + 1.0, fz], [fx, fy + 1.0, fz]], [0.0, 1.0, 0.0], get_uvs("top"));
                    }
                    if y == 0 || chunk.blocks[x][y - 1][z] == 0 {
                        add_face([[fx, fy, fz], [fx + 1.0, fy, fz], [fx + 1.0, fy, fz + 1.0], [fx, fy, fz + 1.0]], [0.0, -1.0, 0.0], get_uvs("bottom"));
                    }
                    if x == CHUNK_SIZE - 1 || chunk.blocks[x + 1][y][z] == 0 {
                        add_face([[fx + 1.0, fy, fz + 1.0], [fx + 1.0, fy, fz], [fx + 1.0, fy + 1.0, fz], [fx + 1.0, fy + 1.0, fz + 1.0]], [1.0, 0.0, 0.0], get_uvs("side"));
                    }
                    if x == 0 || chunk.blocks[x - 1][y][z] == 0 {
                        add_face([[fx, fy, fz], [fx, fy, fz + 1.0], [fx, fy + 1.0, fz + 1.0], [fx, fy + 1.0, fz]], [-1.0, 0.0, 0.0], get_uvs("side"));
                    }
                    if z == CHUNK_SIZE - 1 || chunk.blocks[x][y][z + 1] == 0 {
                        add_face([[fx, fy, fz + 1.0], [fx + 1.0, fy, fz + 1.0], [fx + 1.0, fy + 1.0, fz + 1.0], [fx, fy + 1.0, fz + 1.0]], [0.0, 0.0, 1.0], get_uvs("side"));
                    }
                    if z == 0 || chunk.blocks[x][y][z - 1] == 0 {
                        add_face([[fx + 1.0, fy, fz], [fx, fy, fz], [fx, fy + 1.0, fz], [fx + 1.0, fy + 1.0, fz]], [0.0, 0.0, -1.0], get_uvs("side"));
                    }
                }
            }
        }

        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        mesh.insert_indices(Indices::U32(indices));

        let mesh_handle = meshes.add(mesh);

        // if the entity already has a mesh (re-meshing), just swap the handle.
        // otherwise (first time), insert the Mesh3d and Material components.
        if let Ok(mut existing) = existing_meshes.get_mut(entity) {
            existing.0 = mesh_handle;
        } else {
            commands.entity(entity).insert((
                Mesh3d(mesh_handle),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color_texture: Some(atlas.image.clone()),
                    perceptual_roughness: 1.0,
                    metallic: 0.0,
                    ..default()
                })),
            ));
        }
    }
}