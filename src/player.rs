use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::chunk::{CHUNK_SIZE, Chunk, ChunkCoord, NeedsRemesh};

#[derive(Component)]
pub struct Player {
    pub speed: f32,
    pub sensitivity: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub velocity: Vec3,
    pub jump_force: f32,
    pub gravity: f32,
    pub is_grounded: bool,
}

pub fn setup_environment(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Transform::from_xyz(8.0, 32.0, 8.0).looking_at(Vec3::new(8.0, 32.0, 7.0), Vec3::Y),
        Player {
            speed: 6.0,
            sensitivity: 0.002,
            pitch: 0.0,
            yaw: 0.0,
            velocity: Vec3::ZERO,
            jump_force: 6.5,
            gravity: 20.0,
            is_grounded: false,
        },
    ));

    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(8.0, 50.0, 8.0),
    ));
}

pub fn setup_crosshair(mut commands: Commands) {
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
                BackgroundColor(Color::WHITE),
            ));
        });
}

pub fn toggle_mouse_grab(
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

pub fn break_blocks(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    camera_query: Query<&Transform, With<Camera3d>>,
    mut chunk_query: Query<(Entity, &mut Chunk, &ChunkCoord)>,
) {
    // only trigger on left click
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(cam_transform) = camera_query.single() else {
        return;
    };

    let forward = cam_transform.rotation * Vec3::NEG_Z;
    let origin = cam_transform.translation;

    let reach = 5.0; // how far the player can reach
    let step = 0.05; // accuracy of the raycast
    let max_steps = (reach / step) as usize;

    for i in 0..max_steps {
        let point = origin + forward * (i as f32 * step);

        let world_x = point.x.floor() as i32;
        let world_y = point.y.floor() as i32;
        let world_z = point.z.floor() as i32;

        let chunk_pos = IVec3::new(
            world_x.div_euclid(CHUNK_SIZE as i32),
            world_y.div_euclid(CHUNK_SIZE as i32),
            world_z.div_euclid(CHUNK_SIZE as i32),
        );

        let local_x = world_x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_y = world_y.rem_euclid(CHUNK_SIZE as i32) as usize;
        let local_z = world_z.rem_euclid(CHUNK_SIZE as i32) as usize;

        let mut hit = false;

        // find the specific chunk the ray is currently inside
        for (entity, mut chunk, chunk_coord) in chunk_query.iter_mut() {
            if chunk_coord.0 == chunk_pos {
                // if the block is not air (0)
                if chunk.blocks[local_x][local_y][local_z] != 0 {
                    chunk.blocks[local_x][local_y][local_z] = 0; // Break it

                    // remesh
                    commands.entity(entity).insert(NeedsRemesh);
                    hit = true;
                }
                break; // we found the right chunk, no need to check the rest
            }
        }

        if hit {
            break; // stop stepping the ray forward if we broke a block
        }
    }
}

pub fn player_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&mut Player, &mut Transform)>,
    chunk_query: Query<(&Chunk, &ChunkCoord)>,
) {
    let dt = time.delta_secs();

    // player physical dimensions
    let radius = 0.3; // half width of the player
    let height = 1.8; // total height of the player
    let eye_offset = 1.6; // how high up the camera sits from the feet

    for (mut player, mut transform) in query.iter_mut() {
        let forward = Quat::from_axis_angle(Vec3::Y, player.yaw) * Vec3::NEG_Z;
        let right = Quat::from_axis_angle(Vec3::Y, player.yaw) * Vec3::X;

        let mut input_dir = Vec3::ZERO;
        if keys.pressed(KeyCode::KeyW) {
            input_dir += forward;
        }
        if keys.pressed(KeyCode::KeyS) {
            input_dir -= forward;
        }
        if keys.pressed(KeyCode::KeyA) {
            input_dir -= right;
        }
        if keys.pressed(KeyCode::KeyD) {
            input_dir += right;
        }

        if input_dir.length_squared() > 0.0 {
            input_dir = input_dir.normalize();
        }

        // apply horizontal movement intent
        player.velocity.x = input_dir.x * player.speed;
        player.velocity.z = input_dir.z * player.speed;

        // apply gravity & jumping
        if player.is_grounded && keys.pressed(KeyCode::Space) {
            player.velocity.y = player.jump_force;
            player.is_grounded = false;
        } else {
            player.velocity.y -= player.gravity * dt;
        }

        let is_colliding = |test_pos: Vec3| -> bool {
            let min = test_pos - Vec3::new(radius, eye_offset, radius);
            let max = test_pos + Vec3::new(radius, height - eye_offset, radius);

            let min_x = min.x.floor() as i32;
            let max_x = max.x.floor() as i32;
            let min_y = min.y.floor() as i32;
            let max_y = max.y.floor() as i32;
            let min_z = min.z.floor() as i32;
            let max_z = max.z.floor() as i32;

            for x in min_x..=max_x {
                for y in min_y..=max_y {
                    for z in min_z..=max_z {
                        let chunk_pos = IVec3::new(
                            x.div_euclid(CHUNK_SIZE as i32),
                            y.div_euclid(CHUNK_SIZE as i32),
                            z.div_euclid(CHUNK_SIZE as i32),
                        );
                        let local_x = x.rem_euclid(CHUNK_SIZE as i32) as usize;
                        let local_y = y.rem_euclid(CHUNK_SIZE as i32) as usize;
                        let local_z = z.rem_euclid(CHUNK_SIZE as i32) as usize;

                        // check if the block is solid
                        for (chunk, chunk_coord) in chunk_query.iter() {
                            if chunk_coord.0 == chunk_pos {
                                if chunk.blocks[local_x][local_y][local_z] != 0 {
                                    return true;
                                }
                                break;
                            }
                        }
                    }
                }
            }
            false
        };

        let mut pos = transform.translation;

        pos.y += player.velocity.y * dt;
        if is_colliding(pos) {
            if player.velocity.y < 0.0 {
                let bottom_y = pos.y - eye_offset;
                pos.y = bottom_y.floor() + 1.001 + eye_offset;
                player.is_grounded = true;
            } else {
                let top_y = pos.y + (height - eye_offset);
                pos.y = top_y.floor() - 0.001 - (height - eye_offset);
            }
            player.velocity.y = 0.0;
        } else {
            player.is_grounded = false;
        }

        pos.x += player.velocity.x * dt;
        if is_colliding(pos) {
            if player.velocity.x > 0.0 {
                pos.x = (pos.x + radius).floor() - 0.001 - radius;
            } else {
                pos.x = (pos.x - radius).floor() + 1.001 + radius;
            }
        }

        pos.z += player.velocity.z * dt;
        if is_colliding(pos) {
            if player.velocity.z > 0.0 {
                pos.z = (pos.z + radius).floor() - 0.001 - radius;
            } else {
                pos.z = (pos.z - radius).floor() + 1.001 + radius;
            }
        }

        transform.translation = pos;
    }
}

pub fn camera_look(
    cursor_options: Query<&CursorOptions, With<PrimaryWindow>>,
    mut ev_motion: MessageReader<MouseMotion>,
    mut query: Query<(&mut Player, &mut Transform)>,
) {
    let cursor = cursor_options
        .single()
        .expect("cursor options should exist");

    if cursor.grab_mode == CursorGrabMode::None {
        ev_motion.clear();
        return;
    }

    for (mut player, mut transform) in query.iter_mut() {
        for ev in ev_motion.read() {
            player.yaw -= ev.delta.x * player.sensitivity;
            player.pitch -= ev.delta.y * player.sensitivity;
        }

        player.pitch = player.pitch.clamp(-1.54, 1.54);
        transform.rotation = Quat::from_axis_angle(Vec3::Y, player.yaw)
            * Quat::from_axis_angle(Vec3::X, player.pitch);
    }
}
