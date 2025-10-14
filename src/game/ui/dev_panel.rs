use bevy::prelude::*;
use crate::game::world::terrain::{MapSettings, MapRoot, spawn_random_map};

pub struct DevPanelPlugin;
impl Plugin for DevPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DevUiState>()
            .add_systems(
                Update,
                (toggle_panel, update_fps, panel_buttons, hotkeys)
                    .run_if(in_state(crate::app::AppState::InGame)),
            );
    }
}

// ---------- state ----------
#[derive(Resource)]
struct DevUiState {
    open: bool,
    fps_smooth: f32,
}
impl Default for DevUiState {
    fn default() -> Self {
        Self { open: true, fps_smooth: 0.0 }
    }
}

#[derive(Component)] struct DevPanelRoot;
#[derive(Component)] struct FpsText;
#[derive(Component)] struct SeedText;
#[derive(Component)] struct InfoText;

#[derive(Component, Clone, Copy)]
enum ButtonKind { Reroll, WaterMinus, WaterPlus }

// ---------- systems ----------
fn toggle_panel(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<DevUiState>,
    assets: Res<AssetServer>,
    root_q: Query<Entity, With<DevPanelRoot>>,
    children_q: Query<&Children>,               // <-- add
) {
    if keys.just_pressed(KeyCode::F1) {
        ui.open = !ui.open;
    }

    // spawn if open and not present
    if ui.open && root_q.is_empty() {
        let font: Handle<Font> = assets.load("fonts/arial.ttf");

        commands
            .spawn((
                DevPanelRoot,
                Node {
                    width: Val::Px(340.0),
                    height: Val::Auto,
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(8.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.85)),
            ))
            .with_children(|c| {
                c.spawn((
                    Text::new("DEV PANEL"),
                    TextFont { font: font.clone(), font_size: 16.0, ..default() },
                    TextColor(Color::srgb(1.0,1.0,1.0)),
                ));
                c.spawn((
                    FpsText,
                    Text::new("FPS: ..."),
                    TextFont { font: font.clone(), font_size: 14.0, ..default() },
                    TextColor(Color::srgb(0.8,0.8,0.8)),
                ));
                c.spawn((
                    SeedText,
                    Text::new("Seed: ..."),
                    TextFont { font: font.clone(), font_size: 14.0, ..default() },
                    TextColor(Color::srgb(0.8,0.8,0.8)),
                ));
                c.spawn((
                    InfoText,
                    Text::new("Size: ...   Water: ..."),
                    TextFont { font: font.clone(), font_size: 14.0, ..default() },
                    TextColor(Color::srgb(0.8,0.8,0.8)),
                ));

                // Buttons row
                c.spawn((
                    Node {
                        display: Display::Flex,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },
                ))
                .with_children(|row| {
                    // Reroll
                    row.spawn((
                        ButtonKind::Reroll,
                        Button,
                        Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), ..default() },
                        BackgroundColor(Color::srgb(0.95,0.82,0.10)),
                        BorderColor::all(Color::BLACK),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("Reroll [R]"),
                            TextFont { font: font.clone(), font_size: 14.0, ..default() },
                            TextColor(Color::BLACK),
                        ));
                    });

                    // Water -
                    row.spawn((
                        ButtonKind::WaterMinus,
                        Button,
                        Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), ..default() },
                        BackgroundColor(Color::srgb(0.95,0.82,0.10)),
                        BorderColor::all(Color::BLACK),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("Water -"),
                            TextFont { font: font.clone(), font_size: 14.0, ..default() },
                            TextColor(Color::BLACK),
                        ));
                    });

                    // Water +
                    row.spawn((
                        ButtonKind::WaterPlus,
                        Button,
                        Node { padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)), ..default() },
                        BackgroundColor(Color::srgb(0.95,0.82,0.10)),
                        BorderColor::all(Color::BLACK),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new("Water +"),
                            TextFont { font, font_size: 14.0, ..default() },
                            TextColor(Color::BLACK),
                        ));
                    });
                });
            });
    }

    // hide if closed
    if !ui.open {
        if let Ok(e) = root_q.single() {
            despawn_recursive(&mut commands, e, &children_q);
        }
    }
}

fn update_fps(
    time: Res<Time>,
    mut ui: ResMut<DevUiState>,
    mut fps_q: Query<&mut Text, With<FpsText>>,
    map: Res<MapSettings>,
    mut seed_q: Query<&mut Text, (With<SeedText>, Without<FpsText>)>,
    mut info_q: Query<&mut Text, (With<InfoText>, Without<FpsText>, Without<SeedText>)>,
) {
    let dt = time.delta_secs().max(1e-6);
    let instant = 1.0 / dt;
    ui.fps_smooth = if ui.fps_smooth == 0.0 {
        instant
    } else {
        ui.fps_smooth * 0.9 + instant * 0.1
    };

    if let Ok(mut t) = fps_q.single_mut() {
        t.0 = format!("FPS: {:.1}", ui.fps_smooth);
    }
    if let Ok(mut t) = seed_q.single_mut() {
        t.0 = format!("Seed: {}", map.seed);
    }
    if let Ok(mut t) = info_q.single_mut() {
        t.0 = format!("Size: {}×{}   Water: {:.2}", map.width, map.height, map.water_level);
    }
}

fn panel_buttons(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut map: ResMut<MapSettings>,
    mut q: Query<(&Interaction, &mut BackgroundColor, &ButtonKind), (Changed<Interaction>, With<Button>)>,
    roots: Query<Entity, With<MapRoot>>,
    children_q: Query<&Children>,                                  // <-- add
) {
    let mut changed = false;

    for (interaction, mut bg, kind) in &mut q {
        match *interaction {
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(1.0,0.9,0.2)),
            Interaction::None    => *bg = BackgroundColor(Color::srgb(0.95,0.82,0.10)),
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.9,0.8,0.1));
                match kind {
                    ButtonKind::Reroll => { map.seed = map.seed.wrapping_add(1); changed = true; }
                    ButtonKind::WaterMinus => { map.water_level = (map.water_level - 0.02).clamp(0.05, 0.9); changed = true; }
                    ButtonKind::WaterPlus  => { map.water_level = (map.water_level + 0.02).clamp(0.05, 0.9); changed = true; }
                }
            }
        }
    }

    if changed {
        if let Ok(e) = roots.single() {
            despawn_recursive(&mut commands, e, &children_q);
        }
        spawn_random_map(&mut commands, &mut meshes, &mut materials, &map);
    }
}

fn hotkeys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut map: ResMut<MapSettings>,
    roots: Query<Entity, With<MapRoot>>,
    children_q: Query<&Children>,                                  // <-- add
) {
    if keys.just_pressed(KeyCode::KeyR) {
        map.seed = map.seed.wrapping_add(1);
        if let Ok(e) = roots.single() {
            despawn_recursive(&mut commands, e, &children_q);
        }
        spawn_random_map(&mut commands, &mut meshes, &mut materials, &map);
    }
}

// ---------- helpers ----------
fn despawn_recursive(commands: &mut Commands, entity: Entity, children_q: &Query<&Children>) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_recursive(commands, child, children_q);
        }
    }
    commands.entity(entity).despawn();
}
