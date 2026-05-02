use crate::consts::VERSION;
use bevy::{
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
};
use std::collections::BTreeMap;

pub struct DebugUiPlugin;

impl Plugin for DebugUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DebugInfo>()
            .add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(Startup, setup_debug_ui)
            .add_systems(
                Update,
                (
                    toggle_debug_ui,
                    update_fps_debug_stats,
                    update_version_debug_info,
                    render_debug_ui,
                )
                    .chain(),
            );
    }
}

#[derive(Clone)]
pub struct DebugEntry {
    pub label: Option<String>,
    pub value: String,
}

/// central debug resource that anything can write to (i wont regret this at all)
#[derive(Resource)]
pub struct DebugInfo {
    pub is_active: bool,
    // BTreeMap keeps the entries sorted alphabetically by key
    pub entries: BTreeMap<String, DebugEntry>,
}

impl Default for DebugInfo {
    fn default() -> Self {
        Self {
            is_active: false,
            entries: BTreeMap::new(),
        }
    }
}

/// marker component for UI text
#[derive(Component)]
pub struct DebugTextRoot;

fn setup_debug_ui(mut commands: Commands) {
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            row_gap: Val::Px(2.0),
            ..default()
        },
        Visibility::Hidden,
        DebugTextRoot,
    ));
}

fn toggle_debug_ui(keys: Res<ButtonInput<KeyCode>>, mut debug_info: ResMut<DebugInfo>) {
    if keys.just_pressed(KeyCode::F3) {
        debug_info.is_active = !debug_info.is_active;
    }
}

fn render_debug_ui(
    mut commands: Commands,
    debug_info: Res<DebugInfo>,
    mut root_query: Query<(Entity, &mut Visibility, Option<&Children>), With<DebugTextRoot>>,
    mut text_query: Query<&mut Text>,
) {
    let Ok((root_entity, mut visibility, children)) = root_query.single_mut() else {
        return;
    };

    if !debug_info.is_active {
        *visibility = Visibility::Hidden;
        return;
    }

    *visibility = Visibility::Visible;

    let num_entries = debug_info.entries.len();
    let num_children = children.map_or(0, |c| c.len());

    // spawn new UI line entities if we have more entries than child nodes
    if num_children < num_entries {
        for _ in num_children..num_entries {
            commands
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)), // Per-line background
                    Text::new(""),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ))
                .set_parent_in_place(root_entity);
        }
    }
    // despawn extra UI line entities if an entry was removed
    else if num_children > num_entries {
        if let Some(children_slice) = children {
            for i in num_entries..num_children {
                commands.entity(children_slice[i]).despawn_children();
                commands.entity(children_slice[i]).despawn();
            }
        }
    }

    // update all valid children
    if let Some(children_slice) = children {
        let mut entry_iter = debug_info.entries.values();

        for (i, child) in children_slice.iter().enumerate() {
            if i >= num_entries {
                break;
            }

            if let Ok(mut text) = text_query.get_mut(child) {
                if let Some(entry) = entry_iter.next() {
                    if let Some(label) = &entry.label {
                        text.0 = format!("{}: {}", label, entry.value);
                    } else {
                        text.0 = entry.value.clone();
                    }
                }
            }
        }
    }
}

pub fn update_version_debug_info(mut debug_info: ResMut<DebugInfo>) {
    if !debug_info.is_active {
        return;
    }

    debug_info.entries.insert(
        "00_VERSION".to_string(),
        DebugEntry {
            label: None,
            value: format!("superdupervoxels {VERSION}"),
        },
    );
}

pub fn update_fps_debug_stats(
    diagnostics: Res<DiagnosticsStore>,
    mut debug_info: ResMut<DebugInfo>,
) {
    if !debug_info.is_active {
        return;
    }

    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
            debug_info.entries.insert(
                "0_FPS".to_string(), // internal: forces it to the top
                DebugEntry {
                    label: Some("FPS".to_string()), // user label
                    value: format!("{value:.0}"),
                },
            );
        }
    }
}
