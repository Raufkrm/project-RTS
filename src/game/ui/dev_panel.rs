use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::ui::widget::ImageNode;

use crate::game::world::terrain::{spawn_random_map, MapRoot, MapSettings};

pub struct DevPanelPlugin;
impl Plugin for DevPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DevUiState>().add_systems(
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
        Self {
            open: true,
            fps_smooth: 0.0,
        }
    }
}

#[derive(Component)]
struct DevPanelRoot;
#[derive(Component)]
struct FpsText;
#[derive(Component)]
struct SeedText;
#[derive(Component)]
struct InfoText;

#[derive(Component)]
struct DevMapRoot; // top-right container
#[derive(Component)]
struct HeightmapWidget; // image inside it

#[derive(Component, Clone, Copy)]
enum ButtonKind {
    Reroll,
    WaterMinus,
    WaterPlus,
}

// ---------- systems ----------
fn toggle_panel(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut ui: ResMut<DevUiState>,
    assets: Res<AssetServer>,
    root_q: Query<Entity, With<DevPanelRoot>>,
    children_q: Query<&Children>,

    // heightmap preview
    mut images: ResMut<Assets<Image>>,
    map: Res<MapSettings>,
    map_root_q: Query<Entity, With<DevMapRoot>>,
) {
    if keys.just_pressed(KeyCode::F1) {
        ui.open = !ui.open;
    }

    if ui.open {
        // left dev panel
        if root_q.is_empty() {
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
                        position_type: PositionType::Absolute,
                        top: Val::Px(12.0),
                        left: Val::Px(12.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.85)),
                ))
                .with_children(|c| {
                    c.spawn((
                        Text::new("DEV PANEL"),
                        TextFont {
                            font: font.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgb(1.0, 1.0, 1.0)),
                    ));
                    c.spawn((
                        FpsText,
                        Text::new("FPS: ..."),
                        TextFont {
                            font: font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.8, 0.8, 0.8)),
                    ));
                    c.spawn((
                        SeedText,
                        Text::new("Seed: ..."),
                        TextFont {
                            font: font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.8, 0.8, 0.8)),
                    ));
                    c.spawn((
                        InfoText,
                        Text::new("Size: ...   Water: ..."),
                        TextFont {
                            font: font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.8, 0.8, 0.8)),
                    ));

                    // Buttons row
                    c.spawn((Node {
                        display: Display::Flex,
                        column_gap: Val::Px(8.0),
                        ..default()
                    },))
                        .with_children(|row| {
                            // Reroll
                            row.spawn((
                                ButtonKind::Reroll,
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                                BorderColor::all(Color::BLACK),
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Text::new("Reroll [R]"),
                                    TextFont {
                                        font: font.clone(),
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(Color::BLACK),
                                ));
                            });

                            // Water -
                            row.spawn((
                                ButtonKind::WaterMinus,
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                                BorderColor::all(Color::BLACK),
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Text::new("Water -"),
                                    TextFont {
                                        font: font.clone(),
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(Color::BLACK),
                                ));
                            });

                            // Water +
                            row.spawn((
                                ButtonKind::WaterPlus,
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
                                BorderColor::all(Color::BLACK),
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Text::new("Water +"),
                                    TextFont {
                                        font,
                                        font_size: 14.0,
                                        ..default()
                                    },
                                    TextColor(Color::BLACK),
                                ));
                            });
                        });
                });
        }

        // top-right heightmap preview box
        if map_root_q.is_empty() {
            spawn_heightmap_widget(&mut commands, &mut images, &map);
        }
    }

    // hide if closed
    if !ui.open {
        if let Ok(e) = root_q.single() {
            despawn_recursive(&mut commands, e, &children_q);
        }
        if let Ok(e) = map_root_q.single() {
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
        t.0 = format!(
            "Size: {}×{}   Water: {:.2}",
            map.width, map.height, map.water_level
        );
    }
}

fn panel_buttons(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut map: ResMut<MapSettings>,
    mut q: Query<
        (&Interaction, &mut BackgroundColor, &ButtonKind),
        (Changed<Interaction>, With<Button>),
    >,
    roots: Query<Entity, With<MapRoot>>,
    children_q: Query<&Children>,

    // refresh heightmap preview:
    mut images: ResMut<Assets<Image>>,
    map_root_q: Query<Entity, With<DevMapRoot>>,
) {
    let mut changed = false;

    for (interaction, mut bg, kind) in &mut q {
        match *interaction {
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(1.0, 0.9, 0.2)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.95, 0.82, 0.10)),
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.9, 0.8, 0.1));
                match kind {
                    ButtonKind::Reroll => {
                        map.seed = map.seed.wrapping_add(1);
                        changed = true;
                    }
                    ButtonKind::WaterMinus => {
                        map.water_level = (map.water_level - 0.02).clamp(0.05, 0.9);
                        changed = true;
                    }
                    ButtonKind::WaterPlus => {
                        map.water_level = (map.water_level + 0.02).clamp(0.05, 0.9);
                        changed = true;
                    }
                }
            }
        }
    }

    if changed {
        for e in &roots {
            despawn_recursive(&mut commands, e, &children_q);
        }
        spawn_random_map(&mut commands, &mut meshes, &mut materials, &map);

        // refresh mini heightmap
        if let Ok(root) = map_root_q.single() {
            commands.entity(root).despawn();
        }
        spawn_heightmap_widget(&mut commands, &mut images, &map);
    }
}

fn hotkeys(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut map: ResMut<MapSettings>,
    roots: Query<Entity, With<MapRoot>>,
    children_q: Query<&Children>,

    // refresh heightmap preview:
    mut images: ResMut<Assets<Image>>,
    map_root_q: Query<Entity, With<DevMapRoot>>,
) {
    if keys.just_pressed(KeyCode::KeyR) {
        map.seed = map.seed.wrapping_add(1);
        for e in &roots {
            despawn_recursive(&mut commands, e, &children_q);
        }
        spawn_random_map(&mut commands, &mut meshes, &mut materials, &map);

        if let Ok(root) = map_root_q.single() {
            commands.entity(root).despawn();
        }
        spawn_heightmap_widget(&mut commands, &mut images, &map);
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

// -------- heightmap preview (matches terrain sampling) --------

#[inline]
fn hm_hash(seed: u64, x: i32, y: i32) -> u32 {
    let mut v = seed
        ^ ((x as u64).wrapping_mul(0x9E37_79B1_85EB_CA87))
        ^ ((y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    v ^= v >> 33;
    v = v.wrapping_mul(0xff51_afd7_ed55_8ccd);
    v ^= v >> 33;
    v = v.wrapping_mul(0xc4ceb9fe1a85ec53);
    v ^= v >> 33;
    (v & 0xFFFF_FFFF) as u32
}
#[inline]
fn hm_h01(seed: u64, x: i32, y: i32) -> f32 {
    (hm_hash(seed, x, y) as f32) / (u32::MAX as f32)
}
#[inline]
fn hm_lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
#[inline]
fn hm_smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn hm_value(seed: u64, x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let tx = hm_smooth(x - x.floor());
    let ty = hm_smooth(y - y.floor());
    let v00 = hm_h01(seed, x0, y0);
    let v10 = hm_h01(seed, x1, y0);
    let v01 = hm_h01(seed, x0, y1);
    let v11 = hm_h01(seed, x1, y1);
    let a = hm_lerp(v00, v10, tx);
    let b = hm_lerp(v01, v11, tx);
    hm_lerp(a, b, ty)
}

fn hm_fbm(seed: u64, x: f32, y: f32, base_freq: f32) -> f32 {
    let mut amp = 1.0;
    let mut freq = base_freq.max(0.000_01);
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..5 {
        sum += hm_value(seed, x * freq, y * freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (sum / norm).clamp(0.0, 1.0)
}

fn hm_effective_water(map: &MapSettings, seed: u64, wx: f32, wz: f32) -> f32 {
    if map.water_var_amp <= 0.0 {
        return map.water_level;
    }
    let mask = hm_value(
        seed.wrapping_add(0xBEEF),
        wx * map.water_var_freq,
        wz * map.water_var_freq,
    );
    map.water_level + map.water_var_amp * (mask - 0.5)
}

fn generate_heightmap_image(map: &MapSettings, images: &mut Assets<Image>) -> Handle<Image> {
    let w = map.width as usize;
    let h = map.height as usize;
    let ts = map.tile_size;

    let total_w = w as f32 * ts;
    let total_h = h as f32 * ts;
    let x0 = -0.5 * total_w;
    let z0 = -0.5 * total_h;

    let mut px = vec![0u8; w * h * 4];

    for gy in 0..h {
        for gx in 0..w {
            let wx = x0 + (gx as f32) * ts;
            let wz = z0 + (gy as f32) * ts;

            let n = hm_fbm(map.seed, wx, wz, map.base_freq);
            let water = hm_effective_water(map, map.seed, wx, wz);
            let h_world = (n - water) * map.height_amplitude;

            // [-amp, +amp] -> [0,1]
            let t = (0.5 + 0.5 * (h_world / map.height_amplitude)).clamp(0.0, 1.0);
            let v = (t * 255.0).round() as u8;

            let i = (gy * w + gx) * 4;
            px[i + 0] = v;
            px[i + 1] = v;
            px[i + 2] = v;
            px[i + 3] = 255;
        }
    }

    let img = Image::new(
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );

    images.add(img)
}

fn spawn_heightmap_widget(commands: &mut Commands, images: &mut Assets<Image>, map: &MapSettings) {
    let hm = generate_heightmap_image(map, images);
    commands
        .spawn((
            DevMapRoot,
            Node {
                width: Val::Px(170.0),
                height: Val::Px(170.0),
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                right: Val::Px(12.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.06, 0.85)),
        ))
        .with_children(|p| {
            p.spawn((
                HeightmapWidget,
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    ..default()
                },
                ImageNode::new(hm),
            ));
        });
}
