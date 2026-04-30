use bevy::{
    input::mouse::MouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::chunk::{CHUNK_SIZE, Chunk, NeedsRemesh};

#[derive(Component)]
pub struct FlyCam {
    pub speed: f32,
    pub sensitivity: f32,
    pub pitch: f32,
    pub yaw: f32,
}

pub fn setup_environment(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Msaa::Off,
        Transform::from_xyz(8.0, 16.0, 24.0).looking_at(Vec3::new(8.0, 4.0, 8.0), Vec3::Y),
        FlyCam {
            speed: 15.0,
            sensitivity: 0.002,
            pitch: 0.0,
            yaw: 0.0,
        },
    ));

    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(8.0, 20.0, 8.0),
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

pub fn camera_movement(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mut query: Query<(&FlyCam, &mut Transform)>,
) {
    for (cam, mut transform) in query.iter_mut() {
        let mut velocity = Vec3::ZERO;

        let forward = transform.rotation * Vec3::NEG_Z;
        let right = transform.rotation * Vec3::X;

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

        if keys.pressed(KeyCode::Space) {
            velocity += Vec3::Y;
        }
        if keys.pressed(KeyCode::ShiftLeft) {
            velocity -= Vec3::Y;
        }

        if velocity.length_squared() > 0.0 {
            velocity = velocity.normalize();
            transform.translation += velocity * cam.speed * time.delta_secs();
        }
    }
}

pub fn camera_look(
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
    mut chunk_query: Query<(Entity, &mut Chunk)>,
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
    let step = 0.05; // accuracy of the raycast (smaller = more precise)
    let max_steps = (reach / step) as usize;

    for (entity, mut chunk) in chunk_query.iter_mut() {
        let mut hit = false;

        // Step forward from the camera
        for i in 0..max_steps {
            let point = origin + forward * (i as f32 * step);

            let x = point.x.floor() as isize;
            let y = point.y.floor() as isize;
            let z = point.z.floor() as isize;

            // make sure the coordinate is within the chunk's 16x16x16 bounds
            if x >= 0
                && x < CHUNK_SIZE as isize
                && y >= 0
                && y < CHUNK_SIZE as isize
                && z >= 0
                && z < CHUNK_SIZE as isize
            {
                let ux = x as usize;
                let uy = y as usize;
                let uz = z as usize;

                // if the block is not air (0)
                if chunk.blocks[ux][uy][uz] != 0 {
                    chunk.blocks[ux][uy][uz] = 0; // Set to Air

                    // tag the chunk so the meshing system rebuilds it this frame
                    commands.entity(entity).insert(NeedsRemesh);
                    hit = true;
                    break;
                }
            }
        }

        if hit {
            break; // stop checking chunks if we already broke a block
        }
    }
}
