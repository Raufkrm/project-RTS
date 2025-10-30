use std::fmt::Write as _;

use bevy::input::mouse::MouseButton;
use bevy::log::{info, warn};
use bevy::pbr::MeshMaterial3d;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use crate::app::AppState;
use crate::core::galaxy_camera::MainCamera;
use crate::game::planet_surface::manager::{
    PlanetContext, PlanetContextLayer, PlanetLodConfig, DEFAULT_APPROACH_ERROR_THRESHOLD,
    DEFAULT_SURFACE_ERROR_THRESHOLD,
};
use crate::game::planet_surface::render::PatchStats;
use crate::game::world::planet::{
    analyze_planet_climate, apply_guardrail_adjustment, guardrail_adjustment_from_summaries,
    guardrail_adjustment_from_summary, log_planet_configuration, spawn_random_planet_inner,
    AtmosphereMaterial, GuardrailAdjustment, PlanetClimateSummary, PlanetDebugConfig, PlanetParams,
    PlanetSettings, PlanetSurfaceMaterial, PlanetTag,
};
use crate::game::world::sampling::FlatSamplerRes;
use crate::game::world::terrain::{MapRoot, MapSettings};
use crate::game::{SunDirection, SunSettings};

pub struct DevPanelPlugin;

impl Plugin for DevPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DevPanelState>()
            .init_resource::<PlanetSettings>()
            .add_systems(
                OnEnter(AppState::InGame),
                (cleanup_panel, spawn_dev_panel).chain(),
            )
            .add_systems(OnExit(AppState::InGame), cleanup_panel)
            .add_systems(
                Update,
                (
                    toggle_panel_visibility,
                    update_fps_display,
                    handle_reroll_button,
                    slider_input_system,
                    numeric_input_interactions,
                    numeric_input_editing,
                    update_value_texts,
                    update_slider_handles,
                    update_input_highlights,
                    apply_changes,
                )
                    .run_if(in_state(AppState::InGame)),
            );
        app.add_systems(
            Update,
            handle_seed_sweep_button.run_if(in_state(AppState::InGame)),
        );
        app.add_systems(
            Update,
            (
                update_planet_detail_frequency,
                sync_lod_settings_from_panel,
            )
                .run_if(in_state(AppState::InGame)),
        );
    }
}

#[derive(Resource)]
struct DevPanelState {
    open: bool,
    fps_smooth: f32,

    seed: u64,
    water_level: f32,
    radius: f32,
    height_amp: f32,
    base_freq: f32,
    detail_freq: f32,
    warp_freq: f32,
    warp_amp: f32,
    mountain_strength: f32,
    rotation_deg: f32,
    sun_brightness: f32,
    camera_altitude: f32,
    zoom_ratio: f32,
    context_layer: PlanetContextLayer,
    patches_loaded: usize,
    patches_requested: usize,
    lod_surface_error: f32,
    lod_approach_error: f32,

    dirty: bool,
    active_slider: Option<ParameterKind>,
    active_input: Option<ActiveInput>,
}

impl Default for DevPanelState {
    fn default() -> Self {
        Self {
            open: false,
            fps_smooth: 0.0,
            seed: 0,
            water_level: 0.0,
            radius: 0.0,
            height_amp: 0.0,
            base_freq: 0.0,
            detail_freq: 0.0,
            warp_freq: 0.0,
            warp_amp: 0.0,
            mountain_strength: 0.0,
            rotation_deg: 0.0,
            sun_brightness: 0.10,
            camera_altitude: 0.0,
            zoom_ratio: 1.0,
            context_layer: PlanetContextLayer::Orbit,
            patches_loaded: 0,
            patches_requested: 0,
            lod_surface_error: DEFAULT_SURFACE_ERROR_THRESHOLD,
            lod_approach_error: DEFAULT_APPROACH_ERROR_THRESHOLD,
            dirty: false,
            active_slider: None,
            active_input: None,
        }
    }
}

struct ActiveInput {
    entity: Entity,
    kind: InputKind,
    buffer: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputKind {
    Seed,
    Parameter(ParameterKind),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ParameterKind {
    WaterLevel,
    Radius,
    HeightAmp,
    BaseFreq,
    DetailFreq,
    WarpFreq,
    WarpAmp,
    Mountains,
    Rotation,
    SunBrightness,
    LodSurfaceError,
    LodApproachError,
}

#[derive(Clone, Copy)]
struct ParameterDescriptor {
    kind: ParameterKind,
    label: &'static str,
    min: f32,
    max: f32,
    log_scale: bool,
    precision: usize,
}

impl ParameterDescriptor {
    fn clamp(&self, value: f32) -> f32 {
        value.clamp(self.min, self.max)
    }

    fn factor_from_value(&self, value: f32) -> f32 {
        let clamped = self.clamp(value.max(1e-6));
        if self.log_scale {
            let log_min = self.min.max(1e-6).ln();
            let log_max = self.max.max(self.min + 1e-6).ln();
            let log_val = clamped.max(1e-6).ln();
            ((log_val - log_min) / (log_max - log_min)).clamp(0.0, 1.0)
        } else {
            ((clamped - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
        }
    }

    fn value_from_factor(&self, factor: f32) -> f32 {
        let t = factor.clamp(0.0, 1.0);
        if self.log_scale {
            let log_min = self.min.max(1e-6).ln();
            let log_max = self.max.max(self.min + 1e-6).ln();
            (log_min + (log_max - log_min) * t).exp()
        } else {
            self.min + (self.max - self.min) * t
        }
    }

    fn format_value(&self, value: f32) -> String {
        let mut buffer = String::new();
        let _ = match self.precision {
            0 => write!(buffer, "{:.0}", value),
            1 => write!(buffer, "{:.1}", value),
            2 => write!(buffer, "{:.2}", value),
            3 => write!(buffer, "{:.3}", value),
            _ => write!(buffer, "{:.4}", value),
        };
        buffer
    }
}

const PARAM_DESCRIPTORS: [ParameterDescriptor; 12] = [
    ParameterDescriptor {
        kind: ParameterKind::WaterLevel,
        label: "Water Level",
        min: 0.0,
        max: 1.0,
        log_scale: false,
        precision: 2,
    },
    ParameterDescriptor {
        kind: ParameterKind::Radius,
        label: "Radius",
        min: 100.0,
        max: 2_000_000.0,
        log_scale: true,
        precision: 0,
    },
    ParameterDescriptor {
        kind: ParameterKind::HeightAmp,
        label: "Height Amp",
        min: 10.0,
        max: 20_000.0,
        log_scale: true,
        precision: 1,
    },
    ParameterDescriptor {
        kind: ParameterKind::BaseFreq,
        label: "Base Freq",
        min: 0.05,
        max: 5.0,
        log_scale: true,
        precision: 2,
    },
    ParameterDescriptor {
        kind: ParameterKind::DetailFreq,
        label: "Detail Freq",
        min: 0.1,
        max: 12.0,
        log_scale: true,
        precision: 2,
    },
    ParameterDescriptor {
        kind: ParameterKind::WarpFreq,
        label: "Warp Freq",
        min: 0.1,
        max: 6.0,
        log_scale: true,
        precision: 2,
    },
    ParameterDescriptor {
        kind: ParameterKind::WarpAmp,
        label: "Warp Amp",
        min: 0.0,
        max: 0.4,
        log_scale: false,
        precision: 3,
    },
    ParameterDescriptor {
        kind: ParameterKind::Mountains,
        label: "Mountains",
        min: 0.0,
        max: 1.0,
        log_scale: false,
        precision: 2,
    },
    ParameterDescriptor {
        kind: ParameterKind::Rotation,
        label: "Rotation (deg)",
        min: -180.0,
        max: 180.0,
        log_scale: false,
        precision: 1,
    },
    ParameterDescriptor {
        kind: ParameterKind::SunBrightness,
        label: "Sun Brightness",
        min: 0.1,
        max: 5.0,
        log_scale: true,
        precision: 2,
    },
    ParameterDescriptor {
        kind: ParameterKind::LodSurfaceError,
        label: "LOD Surface Err",
        min: 0.005,
        max: 0.08,
        log_scale: false,
        precision: 3,
    },
    ParameterDescriptor {
        kind: ParameterKind::LodApproachError,
        label: "LOD Approach Err",
        min: 0.04,
        max: 0.5,
        log_scale: false,
        precision: 3,
    },
];

const SEED_SWEEP_COUNT: u32 = 24;
const PANEL_WIDTH: f32 = 320.0;
const SLIDER_WIDTH: f32 = 180.0;
const SLIDER_HEIGHT: f32 = 6.0;
const HANDLE_WIDTH: f32 = 12.0;
const AUTOBALANCE_SWEEP_COUNT: u32 = 6;
const DIGIT_KEYS: &[(KeyCode, char)] = &[
    (KeyCode::Digit0, '0'),
    (KeyCode::Digit1, '1'),
    (KeyCode::Digit2, '2'),
    (KeyCode::Digit3, '3'),
    (KeyCode::Digit4, '4'),
    (KeyCode::Digit5, '5'),
    (KeyCode::Digit6, '6'),
    (KeyCode::Digit7, '7'),
    (KeyCode::Digit8, '8'),
    (KeyCode::Digit9, '9'),
    (KeyCode::Numpad0, '0'),
    (KeyCode::Numpad1, '1'),
    (KeyCode::Numpad2, '2'),
    (KeyCode::Numpad3, '3'),
    (KeyCode::Numpad4, '4'),
    (KeyCode::Numpad5, '5'),
    (KeyCode::Numpad6, '6'),
    (KeyCode::Numpad7, '7'),
    (KeyCode::Numpad8, '8'),
    (KeyCode::Numpad9, '9'),
];

#[inline]
fn resource_angle_from_slider(deg: f32) -> f32 {
    deg.rem_euclid(360.0)
}

#[inline]
fn slider_angle_from_resource(deg: f32) -> f32 {
    ((deg + 180.0).rem_euclid(360.0)) - 180.0
}

#[derive(Component)]
struct DevPanelRoot;

#[derive(Component)]
struct FpsText;
#[derive(Component)]
struct ContextText;

#[derive(Component)]
struct SeedInput;

#[derive(Component)]
struct SeedText;

#[derive(Component)]
struct RerollButton;
#[derive(Component)]
struct SeedSweepButton;

#[derive(Component)]
struct ParameterSlider {
    descriptor: ParameterDescriptor,
}

#[derive(Component)]
struct SliderHandle;

#[derive(Component)]
struct ParameterValueText {
    descriptor: ParameterDescriptor,
}

#[derive(Component)]
struct ParameterInput {
    descriptor: ParameterDescriptor,
}

fn descriptor_for(kind: ParameterKind) -> ParameterDescriptor {
    PARAM_DESCRIPTORS
        .iter()
        .copied()
        .find(|d| d.kind == kind)
        .expect("descriptor missing")
}

fn cleanup_panel(
    mut commands: Commands,
    roots: Query<Entity, With<DevPanelRoot>>,
    children_q: Query<&Children>,
) {
    for entity in roots.iter() {
        despawn_children_recursive(&mut commands, entity, &children_q);
    }
}

fn spawn_dev_panel(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut state: ResMut<DevPanelState>,
    map: Res<MapSettings>,
    params: Res<PlanetParams>,
    settings: Res<PlanetSettings>,
    sun_settings: Res<SunSettings>,
    lod: Res<PlanetLodConfig>,
) {
    state.open = true;
    state.active_input = None;
    state.active_slider = None;
    state.dirty = false;
    state.fps_smooth = 0.0;

    state.seed = map.seed;
    state.water_level = map.water_level;
    state.radius = params.radius;
    state.height_amp = params.height_amp;
    state.base_freq = settings.base_freq;
    state.detail_freq = settings.detail_freq;
    state.warp_freq = settings.warp_freq;
    state.warp_amp = settings.warp_amp;
    state.mountain_strength = settings.mountain_strength;
    state.rotation_deg = slider_angle_from_resource(params.rotation_deg);
    state.sun_brightness = sun_settings.brightness;
    state.camera_altitude = 0.0;
    state.zoom_ratio = 1.0;
    state.context_layer = PlanetContextLayer::Orbit;
    state.patches_loaded = 0;
    state.patches_requested = 0;
    state.lod_surface_error = lod.surface_error;
    state.lod_approach_error = lod.approach_error;

    let font = asset_server.load("fonts/arial.ttf");

    commands
        .spawn((
            DevPanelRoot,
            Node {
                width: Val::Px(PANEL_WIDTH),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                left: Val::Px(20.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.05, 0.08, 0.92)),
            BorderColor::all(Color::srgba(0.3, 0.3, 0.45, 1.0)),
            Name::new("DevPanel"),
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("DEV PANEL"),
                TextFont {
                    font: font.clone(),
                    font_size: 20.0,
                    ..default()
                },
                TextColor(Color::srgba(1.0, 1.0, 1.0, 1.0)),
            ));

            panel.spawn((
                Text::new("FPS: -- | Alt: -- km | Zoom: --"),
                TextFont {
                    font: font.clone(),
                    font_size: 16.0,
                    ..default()
                },
                TextColor(Color::srgba(0.9, 0.9, 0.9, 1.0)),
                FpsText,
            ));

            panel.spawn((
                Text::new("Context: Orbit | Surface patches: 0 / 0"),
                TextFont {
                    font: font.clone(),
                    font_size: 15.0,
                    ..default()
                },
                TextColor(Color::srgba(0.75, 0.86, 1.0, 1.0)),
                ContextText,
            ));

            panel
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    },
                    Name::new("SeedRow"),
                ))
                .with_children(|row| {
                    row.spawn((
                        Text::new("Seed"),
                        TextFont {
                            font: font.clone(),
                            font_size: 16.0,
                            ..default()
                        },
                        TextColor(Color::srgba(0.9, 0.9, 0.9, 1.0)),
                    ));

                    row.spawn((
                        SeedInput,
                        Button,
                        Interaction::default(),
                        Node {
                            width: Val::Px(120.0),
                            height: Val::Px(26.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Start,
                            padding: UiRect::horizontal(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.16, 0.16, 0.22, 1.0)),
                        BorderColor::all(Color::srgba(0.55, 0.55, 0.75, 1.0)),
                        Name::new("SeedInput"),
                    ))
                    .with_children(|input| {
                        input.spawn((
                            SeedText,
                            Text::new(map.seed.to_string()),
                            TextFont {
                                font: font.clone(),
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.95, 0.95, 0.95, 1.0)),
                        ));
                    });

                    row.spawn((
                        RerollButton,
                        Button,
                        Interaction::default(),
                        Node {
                            padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.85, 0.65, 0.1, 1.0)),
                        BorderColor::all(Color::srgba(1.0, 0.85, 0.25, 1.0)),
                        Name::new("RerollButton"),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Reroll"),
                            TextFont {
                                font: font.clone(),
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.1, 0.1, 0.15, 1.0)),
                        ));
                    });

                    row.spawn((
                        SeedSweepButton,
                        Button,
                        Interaction::default(),
                        Node {
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.25, 0.55, 0.95, 1.0)),
                        BorderColor::all(Color::srgba(0.45, 0.75, 1.0, 1.0)),
                        Name::new("SeedSweepButton"),
                    ))
                    .with_children(|button| {
                        button.spawn((
                            Text::new("Sweep ×24"),
                            TextFont {
                                font: font.clone(),
                                font_size: 16.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.06, 0.1, 0.18, 1.0)),
                        ));
                    });
                });

            panel.spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.25, 0.25, 0.35, 0.9)),
                Name::new("Divider"),
            ));

            panel.spawn((
                Text::new("PLANET"),
                TextFont {
                    font: font.clone(),
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::srgba(0.95, 0.95, 0.95, 1.0)),
            ));

            for descriptor in PARAM_DESCRIPTORS.iter().copied() {
                panel
                    .spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                        Name::new(format!("{} Row", descriptor.label)),
                    ))
                    .with_children(|row| {
                        row.spawn((
                            Text::new(descriptor.label),
                            TextFont {
                                font: font.clone(),
                                font_size: 15.0,
                                ..default()
                            },
                            TextColor(Color::srgba(0.92, 0.92, 0.96, 1.0)),
                        ));

                        row.spawn((
                            Node {
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(10.0),
                                ..default()
                            },
                            Name::new(format!("{} Controls", descriptor.label)),
                        ))
                        .with_children(|controls| {
                            controls
                                .spawn((
                                    ParameterSlider { descriptor },
                                    RelativeCursorPosition::default(),
                                    Interaction::default(),
                                    Node {
                                        width: Val::Px(SLIDER_WIDTH),
                                        height: Val::Px(SLIDER_HEIGHT),
                                        position_type: PositionType::Relative,
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.2, 0.2, 0.3, 1.0)),
                                    BorderColor::all(Color::srgba(0.45, 0.45, 0.6, 1.0)),
                                    Name::new(format!("{} Slider", descriptor.label)),
                                ))
                                .with_children(|track| {
                                    track.spawn((
                                        SliderHandle,
                                        Node {
                                            width: Val::Px(HANDLE_WIDTH),
                                            height: Val::Px(HANDLE_WIDTH),
                                            position_type: PositionType::Absolute,
                                            top: Val::Px(-(HANDLE_WIDTH - SLIDER_HEIGHT) * 0.5),
                                            left: Val::Px(0.0),
                                            ..default()
                                        },
                                        BackgroundColor(Color::srgba(0.9, 0.7, 0.25, 1.0)),
                                        BorderColor::all(Color::srgba(1.0, 0.9, 0.45, 1.0)),
                                    ));
                                });

                            controls
                                .spawn((
                                    ParameterInput { descriptor },
                                    Button,
                                    Interaction::default(),
                                    Node {
                                        width: Val::Px(90.0),
                                        height: Val::Px(26.0),
                                        align_items: AlignItems::Center,
                                        justify_content: JustifyContent::Start,
                                        padding: UiRect::horizontal(Val::Px(8.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.13, 0.13, 0.19, 1.0)),
                                    BorderColor::all(Color::srgba(0.5, 0.5, 0.7, 1.0)),
                                    Name::new(format!("{} Input", descriptor.label)),
                                ))
                                .with_children(|input| {
                                    input.spawn((
                                        ParameterValueText { descriptor },
                                        Text::new(descriptor.format_value(descriptor.min)),
                                        TextFont {
                                            font: font.clone(),
                                            font_size: 15.0,
                                            ..default()
                                        },
                                        TextColor(Color::srgba(0.95, 0.95, 0.95, 1.0)),
                                    ));
                                });
                        });
                    });
            }
        });
}

fn toggle_panel_visibility(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<DevPanelState>,
    mut query: Query<&mut Node, With<DevPanelRoot>>,
) {
    if keys.just_pressed(KeyCode::F1) {
        state.open = !state.open;
        state.active_input = None;
        state.active_slider = None;

        if let Ok(mut node) = query.single_mut() {
            node.display = if state.open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

fn update_fps_display(
    time: Res<Time>,
    params: Res<PlanetParams>,
    context: Res<PlanetContext>,
    stats: Res<PatchStats>,
    mut state: ResMut<DevPanelState>,
    mut texts: ParamSet<(
        Query<&mut Text, With<FpsText>>,
        Query<&mut Text, With<ContextText>>,
    )>,
    mut transforms: ParamSet<(
        Query<&GlobalTransform, With<MainCamera>>,
        Query<&GlobalTransform, With<PlanetTag>>,
    )>,
) {
    state.context_layer = context.layer;
    state.patches_loaded = stats.loaded;
    state.patches_requested = stats.requested;

    let dt = time.delta_secs();
    if dt > 0.0 {
        let fps = (1.0 / dt).clamp(0.0, 9999.0);
        let alpha = 0.08;
        state.fps_smooth = if state.fps_smooth <= 0.0 {
            fps
        } else {
            state.fps_smooth * (1.0 - alpha) + fps * alpha
        };
    }

    let cam_tf = transforms.p0().iter().next().copied();
    let planet_tf = transforms.p1().iter().next().copied();
    if let (Some(cam_tf), Some(planet_tf)) = (cam_tf, planet_tf) {
        let center = planet_tf.translation();
        let dist = cam_tf.translation().distance(center);
        let radius = params.radius.max(1.0);
        let altitude = (dist - radius).max(0.0);
        state.camera_altitude = altitude;
        state.zoom_ratio = (dist / radius).max(1.0);
    }

    if let Ok(mut text) = texts.p0().single_mut() {
        let alt_km = state.camera_altitude / 1000.0;
        text.0 = format!(
            "FPS: {:.1} | Alt: {:.1} km | Zoom: {:.2}x",
            state.fps_smooth, alt_km, state.zoom_ratio
        );
    }

    if let Some(mut text) = texts.p1().iter_mut().next() {
        let layer_label = match state.context_layer {
            PlanetContextLayer::Orbit => "Orbit",
            PlanetContextLayer::Approach => "Approach",
            PlanetContextLayer::Surface => "Surface",
        };
        text.0 = format!(
            "Context: {} | Surface patches: {} / {}",
            layer_label, state.patches_loaded, state.patches_requested
        );
    }
}

fn handle_reroll_button(
    mut state: ResMut<DevPanelState>,
    mut query: Query<
        (&Interaction, &mut BackgroundColor),
        (With<RerollButton>, Changed<Interaction>),
    >,
) {
    for (interaction, mut color) in query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgba(1.0, 0.75, 0.25, 1.0));
                state.seed = state.seed.wrapping_add(1);
                state.dirty = true;
                state.active_input = None;
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgba(0.95, 0.7, 0.2, 1.0));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgba(0.85, 0.65, 0.1, 1.0));
            }
        }
    }
}

fn handle_seed_sweep_button(
    mut state: ResMut<DevPanelState>,
    mut map: ResMut<MapSettings>,
    mut params: ResMut<PlanetParams>,
    mut settings: ResMut<PlanetSettings>,
    mut query: Query<
        (&Interaction, &mut BackgroundColor),
        (With<SeedSweepButton>, Changed<Interaction>),
    >,
) {
    for (interaction, mut color) in query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgba(0.22, 0.48, 0.88, 1.0));
                log_seed_sweep(
                    state.seed,
                    SEED_SWEEP_COUNT,
                    &mut state,
                    &mut map,
                    &mut params,
                    &mut settings,
                );
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgba(0.28, 0.58, 0.95, 1.0));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgba(0.25, 0.55, 0.95, 1.0));
            }
        }
    }
}

fn log_seed_sweep(
    base_seed: u64,
    count: u32,
    state: &mut DevPanelState,
    map: &mut MapSettings,
    params: &mut PlanetParams,
    settings: &mut PlanetSettings,
) {
    if count == 0 {
        return;
    }

    info!("seed sweep starting at {} ({} variants)", base_seed, count);

    let mut flagged: Vec<(u64, f32, f32)> = Vec::new();
    let mut summaries: Vec<PlanetClimateSummary> = Vec::with_capacity(count as usize);

    for offset in 0..count {
        let seed = base_seed.wrapping_add(offset as u64);
        let summary = analyze_planet_climate(seed, params, settings);
        summaries.push(summary);

        let water_pct = summary.water_fraction * 100.0;
        let deep_pct = summary.deep_water_fraction * 100.0;
        let coast_pct = summary.coastline_fraction * 100.0;
        let snow_pct = summary.snow_land_fraction * 100.0;

        info!(
            "  seed {seed:>6}: water={water_pct:5.1}% deep={deep_pct:5.1}% coast={coast_pct:5.1}% snow_land={snow_pct:5.1}% temp={:.3} moist={:.3} dry={:.3}",
            summary.avg_land_temperature,
            summary.avg_land_moisture,
            summary.avg_land_dryness,
        );

        if water_pct < 35.0 || water_pct > 65.0 || snow_pct > 35.0 {
            flagged.push((seed, water_pct, snow_pct));
        }
    }

    if flagged.is_empty() {
        info!("seed sweep complete: no outliers beyond thresholds");
    } else {
        for (seed, water_pct, snow_pct) in flagged {
            warn!("  seed {seed} flagged (water={water_pct:.1}% snow_land={snow_pct:.1}%)");
        }
    }

    if let Some(adjustment) = guardrail_adjustment_from_summaries(params, settings, &summaries) {
        if !adjustment.is_empty() {
            apply_guardrail_adjustment(params, settings, &adjustment);
            map.water_level = params.sea_level;
            apply_guardrail_to_state(state, &adjustment);
            info!(
                "guardrail adjustment applied after sweep: sea_level={:?}, height_amp={:?}, mountains={:?}",
                adjustment.sea_level,
                adjustment.height_amp,
                adjustment.mountain_strength
            );
        }
    }
}

fn slider_input_system(
    mut state: ResMut<DevPanelState>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut sliders: Query<(&ParameterSlider, &RelativeCursorPosition, &Interaction)>,
) {
    if !mouse_buttons.pressed(MouseButton::Left) {
        state.active_slider = None;
    }

    for (slider, cursor_pos, interaction) in sliders.iter_mut() {
        if *interaction == Interaction::Pressed {
            state.active_slider = Some(slider.descriptor.kind);
        }

        if let Some(active) = state.active_slider {
            if active == slider.descriptor.kind && mouse_buttons.pressed(MouseButton::Left) {
                if cursor_pos.cursor_over {
                    if let Some(pos) = cursor_pos.normalized {
                        let value = slider.descriptor.value_from_factor(pos.x);
                        state.set_parameter(slider.descriptor.kind, value);
                    }
                }
            }
        }
    }
}

fn numeric_input_interactions(
    mut state: ResMut<DevPanelState>,
    mut query: Query<
        (
            Entity,
            &Interaction,
            Option<&ParameterInput>,
            Option<&SeedInput>,
        ),
        Changed<Interaction>,
    >,
) {
    for (entity, interaction, parameter_input, seed_input) in query.iter_mut() {
        if *interaction != Interaction::Pressed {
            continue;
        }

        let kind = if let Some(input) = parameter_input {
            InputKind::Parameter(input.descriptor.kind)
        } else if seed_input.is_some() {
            InputKind::Seed
        } else {
            continue;
        };

        let buffer = match kind {
            InputKind::Seed => state.seed.to_string(),
            InputKind::Parameter(parameter_kind) => {
                descriptor_for(parameter_kind).format_value(state.parameter_value(parameter_kind))
            }
        };

        state.active_input = Some(ActiveInput {
            entity,
            kind,
            buffer,
        });
    }
}

fn numeric_input_editing(mut state: ResMut<DevPanelState>, keys: Res<ButtonInput<KeyCode>>) {
    let active_opt = state.active_input.as_mut();
    let Some(active) = active_opt else {
        return;
    };

    for &(key, ch) in DIGIT_KEYS.iter() {
        if keys.just_pressed(key) {
            active.buffer.push(ch);
        }
    }

    if keys.just_pressed(KeyCode::Period)
        || keys.just_pressed(KeyCode::NumpadDecimal)
        || keys.just_pressed(KeyCode::NumpadComma)
    {
        if !active.buffer.contains('.') {
            active.buffer.push('.');
        }
    }

    if keys.just_pressed(KeyCode::Minus) || keys.just_pressed(KeyCode::NumpadSubtract) {
        if active.buffer.starts_with('-') {
            active.buffer.remove(0);
        } else {
            active.buffer.insert(0, '-');
        }
    }

    if keys.just_pressed(KeyCode::Backspace) {
        active.buffer.pop();
    }

    if keys.just_pressed(KeyCode::Escape) {
        state.active_input = None;
        return;
    }

    if keys.just_pressed(KeyCode::Enter) {
        match active.kind {
            InputKind::Seed => {
                if let Ok(value) = active.buffer.trim().parse::<u64>() {
                    if value != state.seed {
                        state.seed = value;
                        state.dirty = true;
                    }
                }
            }
            InputKind::Parameter(kind) => {
                if let Ok(value) = active.buffer.trim().parse::<f32>() {
                    state.set_parameter(kind, value);
                }
            }
        }
        state.active_input = None;
    }
}

fn update_value_texts(
    state: Res<DevPanelState>,
    mut seed_text: Query<&mut Text, (With<SeedText>, Without<ParameterValueText>)>,
    mut param_texts: Query<(&mut Text, &ParameterValueText), Without<SeedText>>,
) {
    if let Ok(mut text) = seed_text.single_mut() {
        if let Some(active) = state.active_input.as_ref() {
            if matches!(active.kind, InputKind::Seed) {
                text.0 = active.buffer.clone();
            } else {
                text.0 = state.seed.to_string();
            }
        } else {
            text.0 = state.seed.to_string();
        }
    }

    for (mut text, value_text) in param_texts.iter_mut() {
        if let Some(active) = state.active_input.as_ref() {
            if let InputKind::Parameter(kind) = active.kind {
                if kind == value_text.descriptor.kind {
                    text.0 = active.buffer.clone();
                    continue;
                }
            }
        }
        let value = state.parameter_value(value_text.descriptor.kind);
        text.0 = value_text.descriptor.format_value(value);
    }
}

fn update_slider_handles(
    state: Res<DevPanelState>,
    mut sliders: Query<(&ParameterSlider, &Children)>,
    mut handles: Query<&mut Node, With<SliderHandle>>,
) {
    for (slider, children) in sliders.iter_mut() {
        let value = state.parameter_value(slider.descriptor.kind);
        let factor = slider.descriptor.factor_from_value(value);
        let left = factor * (SLIDER_WIDTH - HANDLE_WIDTH);

        for child in children.iter() {
            if let Ok(mut node) = handles.get_mut(child) {
                node.left = Val::Px(left);
            }
        }
    }
}

fn update_input_highlights(
    state: Res<DevPanelState>,
    mut parameter_inputs: Query<
        (Entity, &mut BackgroundColor),
        (With<ParameterInput>, Without<SeedInput>),
    >,
    mut seed_inputs: Query<
        (Entity, &mut BackgroundColor),
        (With<SeedInput>, Without<ParameterInput>),
    >,
) {
    for (entity, mut color) in parameter_inputs.iter_mut() {
        let active = state
            .active_input
            .as_ref()
            .map(|input| input.entity == entity)
            .unwrap_or(false);
        *color = if active {
            BackgroundColor(Color::srgba(0.18, 0.18, 0.25, 1.0))
        } else {
            BackgroundColor(Color::srgba(0.13, 0.13, 0.19, 1.0))
        };
    }

    for (entity, mut color) in seed_inputs.iter_mut() {
        let active = state
            .active_input
            .as_ref()
            .map(|input| input.entity == entity)
            .unwrap_or(false);
        *color = if active {
            BackgroundColor(Color::srgba(0.2, 0.2, 0.28, 1.0))
        } else {
            BackgroundColor(Color::srgba(0.16, 0.16, 0.22, 1.0))
        };
    }
}

fn apply_changes(
    mut state: ResMut<DevPanelState>,
    mut map: ResMut<MapSettings>,
    mut sampler: ResMut<FlatSamplerRes>,
    mut planet_params: ResMut<PlanetParams>,
    mut planet_settings: ResMut<PlanetSettings>,
    mut sun_settings: ResMut<SunSettings>,
    sun_direction: Res<SunDirection>,
    debug: Res<PlanetDebugConfig>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut planet_materials: ResMut<Assets<PlanetSurfaceMaterial>>,
    mut atmosphere_materials: ResMut<Assets<AtmosphereMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    planets: Query<Entity, With<PlanetTag>>,
    roots: Query<Entity, With<MapRoot>>,
    children_q: Query<&Children>,
) {
    if !state.dirty {
        return;
    }
    if state.active_slider.is_some() {
        return;
    }
    state.dirty = false;

    map.seed = state.seed;
    map.water_level = state.water_level.clamp(0.0, 1.0);

    sampler.0.seed = map.seed;

    planet_params.radius = state.radius.max(1.0);
    planet_params.height_amp = state.height_amp.max(0.0);
    planet_params.sea_level = map.water_level;
    planet_params.rotation_deg = resource_angle_from_slider(state.rotation_deg);

    planet_settings.base_freq = state.base_freq.max(0.0001);
    planet_settings.detail_freq = state.detail_freq.max(0.0);
    planet_settings.warp_freq = state.warp_freq.max(0.0);
    planet_settings.warp_amp = state.warp_amp.max(0.0);
    planet_settings.mountain_strength = state.mountain_strength.clamp(0.0, 1.0);

    let sun_descriptor = descriptor_for(ParameterKind::SunBrightness);
    let brightness = sun_descriptor.clamp(state.sun_brightness);
    if (sun_settings.brightness - brightness).abs() > f32::EPSILON {
        sun_settings.brightness = brightness;
    }
    state.sun_brightness = brightness;

    for _ in 0..2 {
        let mut summaries = Vec::with_capacity(AUTOBALANCE_SWEEP_COUNT as usize);
        for offset in 0..AUTOBALANCE_SWEEP_COUNT {
            let seed = state.seed.wrapping_add(offset as u64);
            summaries.push(analyze_planet_climate(
                seed,
                &*planet_params,
                &*planet_settings,
            ));
        }
        let Some(adjustment) =
            guardrail_adjustment_from_summaries(&*planet_params, &*planet_settings, &summaries)
        else {
            break;
        };
        if adjustment.is_empty() {
            break;
        }
        apply_guardrail_adjustment(&mut *planet_params, &mut *planet_settings, &adjustment);
        map.water_level = planet_params.sea_level.clamp(0.0, 1.0);
        info!(
            "auto-balance applied (sea_level={:?}, height_amp={:?}, mountains={:?})",
            adjustment.sea_level, adjustment.height_amp, adjustment.mountain_strength
        );
    }

    for _ in 0..3 {
        let summary = analyze_planet_climate(map.seed, &*planet_params, &*planet_settings);
        let guard_adjustment =
            guardrail_adjustment_from_summary(&*planet_params, &*planet_settings, &summary);
        if guard_adjustment.is_empty() {
            break;
        }
        apply_guardrail_adjustment(
            &mut *planet_params,
            &mut *planet_settings,
            &guard_adjustment,
        );
        map.water_level = planet_params.sea_level.clamp(0.0, 1.0);
    }
    state.water_level = map.water_level;
    state.height_amp = planet_params.height_amp;
    state.mountain_strength = planet_settings.mountain_strength;

    for entity in planets.iter() {
        despawn_children_recursive(&mut commands, entity, &children_q);
    }
    for entity in roots.iter() {
        despawn_children_recursive(&mut commands, entity, &children_q);
    }

    log_planet_configuration(
        "dev_panel_apply_changes",
        &map,
        &planet_params,
        &planet_settings,
        &sampler,
    );

    spawn_random_planet_inner(
        &mut commands,
        &mut meshes,
        &mut *planet_materials,
        &mut *atmosphere_materials,
        &mut *standard_materials,
        &*sampler,
        &*map,
        &*planet_params,
        &*planet_settings,
        &*debug,
        &*sun_direction,
    );
}

const DETAIL_FREQ_SURFACE_NEAR: f32 = 12.0;
const DETAIL_ALT_BLEND_START_KM: f32 = 40.0;
const DETAIL_ALT_BLEND_END_KM: f32 = 180.0;

fn update_planet_detail_frequency(
    planet_params: Res<PlanetParams>,
    planet_settings: Res<PlanetSettings>,
    mut materials: ResMut<Assets<PlanetSurfaceMaterial>>,
    q_planet: Query<(&GlobalTransform, &MeshMaterial3d<PlanetSurfaceMaterial>), With<PlanetTag>>,
    q_camera: Query<&GlobalTransform, With<MainCamera>>,
) {
    let Some(camera_tf) = q_camera.iter().next() else {
        return;
    };
    let Some((planet_tf, material_handle)) = q_planet.iter().next() else {
        return;
    };

    let distance = camera_tf.translation().distance(planet_tf.translation());
    let radius = planet_params.radius.max(1.0);
    let altitude = (distance - radius).max(0.0);
    let altitude_km = altitude * 0.001;

    let blend = if altitude_km <= DETAIL_ALT_BLEND_START_KM {
        1.0
    } else if altitude_km >= DETAIL_ALT_BLEND_END_KM {
        0.0
    } else {
        1.0 - ((altitude_km - DETAIL_ALT_BLEND_START_KM)
            / (DETAIL_ALT_BLEND_END_KM - DETAIL_ALT_BLEND_START_KM))
    };

    let orbit_freq = planet_settings.detail_freq.max(0.0001);
    let near_freq = DETAIL_FREQ_SURFACE_NEAR.max(orbit_freq);
    let target_freq = orbit_freq + (near_freq - orbit_freq) * blend;

    let handle = material_handle.0.clone();
    if let Some(material) = materials.get_mut(&handle) {
        let current = material.extension.params.detail_freq;
        if (current - target_freq).abs() > 1e-3 {
            material.extension.params.detail_freq = target_freq;
        }
    }
}

fn sync_lod_settings_from_panel(
    mut state: ResMut<DevPanelState>,
    mut lod: ResMut<PlanetLodConfig>,
) {
    let surface_desc = descriptor_for(ParameterKind::LodSurfaceError);
    let approach_desc = descriptor_for(ParameterKind::LodApproachError);

    let surface = surface_desc.clamp(state.lod_surface_error);
    let mut approach = approach_desc.clamp(state.lod_approach_error);

    if approach < surface {
        approach = surface;
    }

    if (surface - state.lod_surface_error).abs() > 1e-6 {
        state.lod_surface_error = surface;
    }
    if (approach - state.lod_approach_error).abs() > 1e-6 {
        state.lod_approach_error = approach;
    }

    let mut changed = false;
    if (lod.surface_error - surface).abs() > 1e-5 {
        lod.surface_error = surface;
        changed = true;
    }
    if (lod.approach_error - approach).abs() > 1e-5 {
        lod.approach_error = approach;
        changed = true;
    }

    if changed {
        // No additional action required; planet context will pick up the new
        // thresholds on the next update tick.
    }
}

fn despawn_children_recursive(
    commands: &mut Commands,
    entity: Entity,
    children_q: &Query<&Children>,
) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_children_recursive(commands, child, children_q);
        }
    }
    commands.entity(entity).despawn();
}

impl DevPanelState {
    fn parameter_value(&self, kind: ParameterKind) -> f32 {
        match kind {
            ParameterKind::WaterLevel => self.water_level,
            ParameterKind::Radius => self.radius,
            ParameterKind::HeightAmp => self.height_amp,
            ParameterKind::BaseFreq => self.base_freq,
            ParameterKind::DetailFreq => self.detail_freq,
            ParameterKind::WarpFreq => self.warp_freq,
        ParameterKind::WarpAmp => self.warp_amp,
        ParameterKind::Mountains => self.mountain_strength,
        ParameterKind::Rotation => self.rotation_deg,
        ParameterKind::SunBrightness => self.sun_brightness,
        ParameterKind::LodSurfaceError => self.lod_surface_error,
        ParameterKind::LodApproachError => self.lod_approach_error,
    }
}

    fn set_parameter(&mut self, kind: ParameterKind, value: f32) {
        let descriptor = descriptor_for(kind);
        let clamped = descriptor.clamp(value);
        let target = match kind {
            ParameterKind::WaterLevel => &mut self.water_level,
            ParameterKind::Radius => &mut self.radius,
            ParameterKind::HeightAmp => &mut self.height_amp,
            ParameterKind::BaseFreq => &mut self.base_freq,
            ParameterKind::DetailFreq => &mut self.detail_freq,
            ParameterKind::WarpFreq => &mut self.warp_freq,
            ParameterKind::WarpAmp => &mut self.warp_amp,
            ParameterKind::Mountains => &mut self.mountain_strength,
            ParameterKind::Rotation => &mut self.rotation_deg,
            ParameterKind::SunBrightness => &mut self.sun_brightness,
            ParameterKind::LodSurfaceError => &mut self.lod_surface_error,
            ParameterKind::LodApproachError => &mut self.lod_approach_error,
        };

        if (clamped - *target).abs() > f32::EPSILON {
            *target = clamped;
            if !matches!(kind, ParameterKind::LodSurfaceError | ParameterKind::LodApproachError) {
                self.dirty = true;
            }
        }
    }
}
fn apply_guardrail_to_state(state: &mut DevPanelState, adjustment: &GuardrailAdjustment) {
    if let Some(sea) = adjustment.sea_level {
        state.set_parameter(ParameterKind::WaterLevel, sea);
    }
    if let Some(height) = adjustment.height_amp {
        state.set_parameter(ParameterKind::HeightAmp, height);
    }
    if let Some(mountain) = adjustment.mountain_strength {
        state.set_parameter(ParameterKind::Mountains, mountain);
    }
}
