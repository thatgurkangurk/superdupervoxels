use bevy::app::AppExit;
use bevy::prelude::*;
use postcard::{from_bytes, to_stdvec};
use redb::{Database, ReadableDatabase, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::blocks::{BlockRegistry, NamespacedId};
use crate::chunk::{CHUNK_SIZE, Chunk, ChunkCoord, NeedsRemesh};
use crate::player::FlyCam;

const CHUNKS_TABLE: TableDefinition<[i32; 3], &[u8]> = TableDefinition::new("chunks");
const REGION_SIZE: i32 = 32; // 32x32 chunks per region file

#[derive(Serialize, Deserialize, Default)]
pub struct WorldMeta {
    pub name: String,
    pub seed: u128,
    // add player inventory, time of day, and so on here later
}

#[derive(Resource)]
pub struct ChunkManager {
    pub loaded_chunks: HashMap<IVec3, Entity>,
    pub render_distance: i32,
    #[allow(dead_code, unused)]
    pub world_name: String,
    pub world_path: PathBuf,
    // caches open databases so we don't reopen files every frame
    open_regions: HashMap<IVec2, Database>,
}

impl ChunkManager {
    pub fn new(world_name: &str) -> Self {
        let world_path = PathBuf::from("worlds").join(world_name);
        let regions_path = world_path.join("regions");

        // create directory structure
        fs::create_dir_all(&regions_path).expect("Failed to create world directories");

        // handle world.toml
        let toml_path = world_path.join("world.toml");
        if !toml_path.exists() {
            let meta = WorldMeta {
                name: world_name.to_string(),
                seed: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
            };
            let toml_string = toml::to_string_pretty(&meta).unwrap();
            fs::write(&toml_path, toml_string).expect("Failed to write world.toml");
        }

        Self {
            loaded_chunks: HashMap::new(),
            render_distance: 4,
            world_name: world_name.to_string(),
            world_path,
            open_regions: HashMap::new(),
        }
    }

    /// gets an open database connection for a specific region, or opens it if it doesn't exist
    pub fn get_region_db(&mut self, chunk_coord: IVec3) -> &Database {
        let region_coord = IVec2::new(
            chunk_coord.x.div_euclid(REGION_SIZE),
            chunk_coord.z.div_euclid(REGION_SIZE),
        );

        self.open_regions.entry(region_coord).or_insert_with(|| {
            let db_path = self.world_path.join(format!(
                "regions/r.{}.{}.redb",
                region_coord.x, region_coord.y
            ));

            let db = Database::create(db_path).expect("Failed to create region database");

            // ensure the table exists in this new file
            let write_txn = db.begin_write().unwrap();
            write_txn.open_table(CHUNKS_TABLE).unwrap();
            write_txn.commit().unwrap();

            db
        })
    }
}

pub fn manage_chunks(
    mut commands: Commands,
    player_transform: Single<&Transform, With<FlyCam>>,
    mut chunk_manager: ResMut<ChunkManager>,
    chunk_query: Query<&Chunk>,
    registry: Res<BlockRegistry>,
) {
    let player_pos = player_transform.translation;
    let player_coord = IVec3::new(
        (player_pos.x / CHUNK_SIZE as f32).floor() as i32,
        (player_pos.y / CHUNK_SIZE as f32).floor() as i32,
        (player_pos.z / CHUNK_SIZE as f32).floor() as i32,
    );

    let dist = chunk_manager.render_distance;
    let mut expected_chunks = Vec::new();

    for x in -dist..=dist {
        for y in -2..=2 {
            for z in -dist..=dist {
                expected_chunks.push(player_coord + IVec3::new(x, y, z));
            }
        }
    }

    // --- unload & save ---
    // group chunks to unload by their region so they can be written as a batch
    let mut chunks_to_unload_by_region: HashMap<IVec2, Vec<(IVec3, Entity)>> = HashMap::new();

    for (&coord, &entity) in chunk_manager.loaded_chunks.iter() {
        if !expected_chunks.contains(&coord) {
            let region = IVec2::new(
                coord.x.div_euclid(REGION_SIZE),
                coord.z.div_euclid(REGION_SIZE),
            );
            chunks_to_unload_by_region
                .entry(region)
                .or_default()
                .push((coord, entity));
        }
    }

    for (_region_coord, chunks) in chunks_to_unload_by_region {
        // only grab the first chunk's coord to fetch the right db file (they are all in the same region)
        let db = chunk_manager.get_region_db(chunks[0].0);
        let write_txn = db.begin_write().unwrap();
        {
            let mut table = write_txn.open_table(CHUNKS_TABLE).unwrap();
            for (coord, entity) in chunks {
                if let Ok(chunk_data) = chunk_query.get(entity) {
                    if let Ok(bytes) = to_stdvec(chunk_data) {
                        table.insert(coord.to_array(), bytes.as_slice()).unwrap();
                    }
                }
                commands.entity(entity).despawn_children();
                commands.entity(entity).despawn();
            }
        }
        write_txn.commit().unwrap();
    }

    // remove unloaded chunks from the tracking map
    chunk_manager
        .loaded_chunks
        .retain(|coord, _| expected_chunks.contains(coord));

    // --- LOAD & GENERATE ---
    let mut missing_chunks_by_region: HashMap<IVec2, Vec<IVec3>> = HashMap::new();
    for coord in expected_chunks {
        if !chunk_manager.loaded_chunks.contains_key(&coord) {
            let region = IVec2::new(
                coord.x.div_euclid(REGION_SIZE),
                coord.z.div_euclid(REGION_SIZE),
            );
            missing_chunks_by_region
                .entry(region)
                .or_default()
                .push(coord);
        }
    }

    for (_region_coord, coords) in missing_chunks_by_region {
        let db = chunk_manager.get_region_db(coords[0]);
        let read_txn = db.begin_read().unwrap();
        let table_result = read_txn.open_table(CHUNKS_TABLE);

        for coord in coords {
            let mut chunk = Chunk::empty();
            let mut loaded_from_db = false;

            if let Ok(ref table) = table_result {
                if let Ok(Some(db_data)) = table.get(coord.to_array()) {
                    if let Ok(loaded_chunk) = from_bytes::<Chunk>(db_data.value()) {
                        chunk = loaded_chunk;
                        loaded_from_db = true;
                    }
                }
            }

            if !loaded_from_db {
                let dirt_id = registry
                    .get_internal_id(&NamespacedId::new("superdupervoxels", "dirt"))
                    .unwrap();
                let grass_id = registry
                    .get_internal_id(&NamespacedId::new("superdupervoxels", "grass"))
                    .unwrap();

                if coord.y == 0 {
                    for x in 0..CHUNK_SIZE {
                        for z in 0..CHUNK_SIZE {
                            chunk.blocks[x][0][z] = dirt_id;
                            chunk.blocks[x][1][z] = grass_id;
                        }
                    }
                }
            }

            let world_x = coord.x as f32 * CHUNK_SIZE as f32;
            let world_y = coord.y as f32 * CHUNK_SIZE as f32;
            let world_z = coord.z as f32 * CHUNK_SIZE as f32;

            let entity = commands
                .spawn((
                    chunk,
                    ChunkCoord(coord),
                    Transform::from_xyz(world_x, world_y, world_z),
                    NeedsRemesh,
                ))
                .id();

            chunk_manager.loaded_chunks.insert(coord, entity);
        }
    }
}

pub fn save_world_on_exit(
    mut exit_events: MessageReader<AppExit>,
    mut chunk_manager: ResMut<ChunkManager>,
    chunk_query: Query<(&Chunk, &ChunkCoord)>,
) {
    if exit_events.read().next().is_some() {
        info!("Saving active chunks to region databases...");

        let mut chunks_by_region: HashMap<IVec2, Vec<(&Chunk, IVec3)>> = HashMap::new();

        // group all active chunks
        for (chunk, coord) in chunk_query.iter() {
            let region = IVec2::new(
                coord.0.x.div_euclid(REGION_SIZE),
                coord.0.z.div_euclid(REGION_SIZE),
            );
            chunks_by_region
                .entry(region)
                .or_default()
                .push((chunk, coord.0));
        }

        // save each region
        for (_region, chunks) in chunks_by_region {
            let db = chunk_manager.get_region_db(chunks[0].1);
            let write_txn = db.begin_write().unwrap();
            {
                let mut table = write_txn.open_table(CHUNKS_TABLE).unwrap();
                for (chunk, coord) in chunks {
                    if let Ok(bytes) = to_stdvec(chunk) {
                        table.insert(coord.to_array(), bytes.as_slice()).unwrap();
                    }
                }
            }
            write_txn.commit().unwrap();
        }

        info!("World saved successfully!");
    }
}
