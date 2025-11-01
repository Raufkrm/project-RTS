use crate::app::AppState;
use crate::game::ui::pause_menu::pause_menu_hidden;
use crate::game::ui::settings_menu::settings_menu_hidden;
use crate::game::world::planet::{PlanetDebugConfig, PlanetDebugMode, PlanetParams, PlanetTag};
use bevy::math::Isometry3d;
use bevy::prelude::*;
use std::f32::consts::FRAC_PI_2;

#[derive(Resource, Default, Clone)]
pub struct LodDebugBands {
    pub radii: Vec<f32>,
}

pub struct PlanetDebugPlugin;
impl Plugin for PlanetDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LodDebugBands>()
            .insert_resource(LodDebugBands {
                // scale multipliers relative to the planet radius (1.0 = surface)
                radii: vec![1.20, 1.68, 3.60, 5.40],
            })
            .add_systems(
                Update,
                (draw_lod_bands, cycle_debug_mode)
                    .run_if(in_state(AppState::InGame))
                    .run_if(pause_menu_hidden)
                    .run_if(settings_menu_hidden),
            );
    }
}

fn draw_lod_bands(
    bands: Res<LodDebugBands>,
    params: Res<PlanetParams>,
    q_planet: Query<&GlobalTransform, With<PlanetTag>>,
    mut gizmos: Gizmos,
) {
    let Ok(planet_tf) = q_planet.single() else {
        return;
    };
    let center = planet_tf.translation();
    let colors = [
        Color::srgb(0.9, 0.3, 0.3),
        Color::srgb(0.9, 0.6, 0.2),
        Color::srgb(0.6, 0.8, 0.2),
        Color::srgb(0.2, 0.6, 0.9),
    ];
    let base_radius = params.radius.max(1.0);
    for (i, band) in bands.radii.iter().enumerate() {
        let color = colors.get(i).copied().unwrap_or(Color::WHITE);
        let iso = Isometry3d::new(center, Quat::from_rotation_x(FRAC_PI_2));
        gizmos.circle(iso, base_radius * *band, color);
    }
}

fn cycle_debug_mode(keys: Res<ButtonInput<KeyCode>>, mut debug_view: ResMut<PlanetDebugConfig>) {
    if keys.just_pressed(KeyCode::F7) {
        debug_view.mode = debug_view.mode.next();
        info!("Planet debug view set to {:?}", debug_view.mode);
    } else if keys.just_pressed(KeyCode::F6) {
        debug_view.mode = PlanetDebugMode::None;
        info!("Planet debug view cleared");
    }
}
