use bevy::prelude::*;
use bevy::prelude::Cuboid;

use crate::core::surface_model::{dir_to_face_uv, PlanetSurfaceModel, SurfaceCoord};
use crate::game::world::surface_grid::SurfaceGrid;

#[derive(Component, Clone, Copy, Debug)]
pub struct GroundAnchor {
    pub coord: SurfaceCoord,
}

#[derive(Component)]
pub struct TestUnit;

pub const TEST_UNIT_MOVE_SPEED: f32 = 50_000.0;

#[derive(Component)]
pub struct MoveOrder {
    pub target: SurfaceCoord,
    pub speed: f32,
}

pub fn update_ground_anchors_system(
    surface_model: Res<PlanetSurfaceModel>,
    surface_grid: Res<SurfaceGrid>,
    mut q: Query<(&GroundAnchor, &mut Transform), (With<TestUnit>, Without<MoveOrder>)>,
) {
    if surface_grid.resolution == 0 {
        return;
    }

    for (anchor, mut transform) in q.iter_mut() {
        let coord = anchor.coord;
        let height = surface_grid.sample_height_uv(coord.face, coord.uv);
        let world_coord = SurfaceCoord {
            face: coord.face,
            uv: coord.uv,
            height,
        };
        let world_pos = surface_model.surface_to_world(world_coord);

        transform.translation = world_pos;
        let dir = world_pos.normalize_or_zero();
        if dir.length_squared() > f32::EPSILON {
            transform.look_to(dir, Vec3::Y);
        }
    }
}

pub fn update_unit_movement_system(
    time: Res<Time>,
    surface_model: Res<PlanetSurfaceModel>,
    surface_grid: Res<SurfaceGrid>,
    mut q: Query<(&mut GroundAnchor, &mut Transform, &mut MoveOrder), With<TestUnit>>,
) {
    if surface_grid.resolution == 0 {
        return;
    }

    let dt = time.delta_secs();

    for (mut anchor, mut transform, mut order) in &mut q {
        if order.speed <= 0.0 {
            continue;
        }

        let current_world = transform.translation;
        let target_height = surface_grid.sample_height_uv(order.target.face, order.target.uv);
        let target_coord = SurfaceCoord {
            face: order.target.face,
            uv: order.target.uv,
            height: target_height,
        };
        let target_world = surface_model.surface_to_world(target_coord);

        let delta = target_world - current_world;
        let distance = delta.length();

        if distance < 100.0 {
            anchor.coord = target_coord;
            transform.translation = target_world;
            order.speed = 0.0;
            continue;
        }

        let max_step = order.speed * dt;
        let step_factor = (max_step / distance).min(1.0);
        let new_world = current_world + delta * step_factor;

        if new_world.length_squared() <= f32::EPSILON {
            continue;
        }

        let dir = new_world.normalize();
        let (face, uv) = dir_to_face_uv(dir);
        let height = surface_grid.sample_height_uv(face, uv);
        let new_coord = SurfaceCoord { face, uv, height };
        let final_world = surface_model.surface_to_world(new_coord);

        anchor.coord = new_coord;
        transform.translation = final_world;

        let forward = (target_world - final_world).normalize_or_zero();
        let up = final_world.normalize_or_zero();
        if forward.length_squared() > 0.0 && up.length_squared() > 0.0 {
            transform.look_to(forward, up);
        }
    }
}

pub fn spawn_test_unit(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    coord: SurfaceCoord,
    surface_model: &PlanetSurfaceModel,
    surface_grid: &SurfaceGrid,
) {
    if surface_grid.resolution == 0 {
        return;
    }

    let height = surface_grid.sample_height_uv(coord.face, coord.uv);
    let world_coord = SurfaceCoord {
        face: coord.face,
        uv: coord.uv,
        height,
    };
    let world_pos = surface_model.surface_to_world(world_coord);

    let mesh = meshes.add(Mesh::from(Cuboid::new(5_000.0, 5_000.0, 5_000.0)));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 1.0, 0.2),
        ..Default::default()
    });

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(world_pos),
        GlobalTransform::default(),
        Visibility::default(),
        InheritedVisibility::default(),
        TestUnit,
        GroundAnchor { coord },
        MoveOrder {
            target: coord,
            speed: TEST_UNIT_MOVE_SPEED,
        },
        Name::new("TestUnit"),
    ));
}
