use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[allow(unused_imports)]
use crate::app::AppState;
use crate::core::galaxy_camera::MainCamera;
use crate::core::surface_model::{PlanetSurfaceModel, SurfaceCoord};
use crate::game::units::{spawn_test_unit, MoveOrder, TestUnit, TEST_UNIT_MOVE_SPEED};
use crate::game::world::surface_grid::SurfaceGrid;

#[derive(Resource, Default)]
pub struct LastSurfacePick {
    pub coord: Option<SurfaceCoord>,
    pub world_pos: Option<Vec3>,
}

pub fn init_surface_pick_res(mut commands: Commands) {
    commands.init_resource::<LastSurfacePick>();
}

pub fn debug_pick_surface_coord(
    mut commands: Commands,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keyboard: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    surface: Res<PlanetSurfaceModel>,
    surface_grid: Res<SurfaceGrid>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last_pick: ResMut<LastSurfacePick>,
    mut units_q: Query<(Entity, &mut MoveOrder), With<TestUnit>>,
) {
    if !mouse_buttons.just_pressed(MouseButton::Right) {
        return;
    }

    if !(keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)) {
        return;
    }

    let mut window_iter = windows.iter();
    let Some(window) = window_iter.next() else {
        return;
    };

    let cursor_pos = if let Some(pos) = window.cursor_position() {
        pos
    } else {
        return;
    };

    let mut cam_iter = camera_q.iter();
    let Some((camera, cam_transform)) = cam_iter.next() else {
        return;
    };

    let Ok(ray) = camera.viewport_to_world(cam_transform, cursor_pos) else {
        info!("viewport_to_world failed for cursor {cursor_pos:?}");
        return;
    };
    let ray_origin = ray.origin;
    let ray_dir: Vec3 = ray.direction.into();

    let radius = surface.radius;
    let a = ray_dir.dot(ray_dir);
    let b = 2.0 * ray_origin.dot(ray_dir);
    let c = ray_origin.dot(ray_origin) - radius * radius;
    let disc: f32 = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return;
    }
    let sqrt_disc = disc.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);
    let t = if t1 > 0.0 {
        t1
    } else if t2 > 0.0 {
        t2
    } else {
        return;
    };

    let hit_world = ray_origin + ray_dir * t;
    let coord = surface.world_to_surface(hit_world);

    last_pick.coord = Some(coord);
    last_pick.world_pos = Some(hit_world);

    info!(
        "Surface pick: face={} uv=({:.4},{:.4}) height={:.2} world=({:.1},{:.1},{:.1})",
        coord.face,
        coord.uv.x,
        coord.uv.y,
        coord.height,
        hit_world.x,
        hit_world.y,
        hit_world.z
    );

    let mut units_iter = units_q.iter_mut();
    if let Some((_entity, mut order)) = units_iter.next() {
        order.target = coord;
        if order.speed <= 0.0 {
            order.speed = TEST_UNIT_MOVE_SPEED;
        }
        for (_entity, mut other) in units_iter {
            other.target = coord;
            if other.speed <= 0.0 {
                other.speed = TEST_UNIT_MOVE_SPEED;
            }
        }
    } else {
        spawn_test_unit(
            &mut commands,
            &mut meshes,
            &mut materials,
            coord,
            &surface,
            &surface_grid,
        );
    }
}
