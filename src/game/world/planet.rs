use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_mesh::{Indices, Mesh};

use crate::game::world::sampling::FlatSamplerRes;
use crate::game::world::terrain::MapSettings;

// -----------------------------------------------------------------------------
// Tags / params
// -----------------------------------------------------------------------------
#[derive(Component)]
pub struct PlanetTag;

/// Stable params the rest of the game already uses (radius + sea level).
#[derive(Resource, Clone, Copy)]
pub struct PlanetParams {
    /// Base sphere radius in world units (sea sits at this radius).
    pub radius: f32,
    /// World-space vertical amplitude used when displacing above sea.
    pub height_amp: f32,
    /// Sea level in noise space [0..1].
    pub sea_level: f32,
}
impl Default for PlanetParams {
    fn default() -> Self {
        Self {
            radius: 6.0,
            height_amp: 1.5,
            sea_level: 0.50,
        }
    }
}

#[derive(Resource, Clone, Copy)]
pub struct PlanetSettings {
    // shapes
    pub base_freq: f32,
    pub detail_freq: f32,
    pub warp_freq: f32,
    pub warp_amp: f32,
    /// Extra roughness for high elevations (0..1).
    pub mountain_strength: f32,
    /// How spiky mountain ridges become (0..1).
    pub mountain_spikiness: f32,

    // water
    pub water_deep: Vec3,
    pub water_shallow: Vec3,

    // land
    pub land_sand: Vec3,
    pub land_grass: Vec3,

    /// Rock/gray color (mid/high altitudes)
    pub land_rock: Vec3,
    /// Snow/ice color (highest altitudes)
    pub land_snow: Vec3,
    /// Elevation (0..1 above sea) where grass fades to rock
    pub rock_start: f32,
    /// Elevation (0..1 above sea) where rock fades to snow
    pub snow_start: f32,
}
impl Default for PlanetSettings {
    fn default() -> Self {
        Self {
            base_freq: 0.9,
            detail_freq: 5.0,
            warp_freq: 1.8,
            warp_amp: 0.06,
            mountain_strength: 0.2,
            mountain_spikiness: 0.3, // spiky peaks

            water_deep: Vec3::new(0.08, 0.16, 0.30),
            water_shallow: Vec3::new(0.22, 0.38, 0.70),

            land_sand: Vec3::new(0.76, 0.71, 0.54),
            land_grass: Vec3::new(0.42, 0.64, 0.34),

            land_rock: Vec3::new(0.45, 0.47, 0.48), // neutral gray
            land_snow: Vec3::new(0.95, 0.97, 0.98), // cold white
            rock_start: 0.45,                       // grass fades to rock
            snow_start: 0.72,                       // rock fades to snow
        }
    }
}

// -----------------------------------------------------------------------------
// Plugin – registers resources (so they’re available in Phase 2 UI)
// -----------------------------------------------------------------------------
pub struct PlanetPlugin;
impl Plugin for PlanetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlanetParams>()
            .init_resource::<PlanetSettings>();
    }
}

// -----------------------------------------------------------------------------
// Entry points
// -----------------------------------------------------------------------------

/// Existing call-site wrapper (keeps your Dev Panel working as-is).
pub fn spawn_random_planet_inner(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    sampler_res: &FlatSamplerRes,
    map: &MapSettings,
    params: &PlanetParams,
) {
    let settings = PlanetSettings::default();

    spawn_planet_with_settings(
        commands,
        meshes,
        materials,
        sampler_res,
        map,
        params,
        &settings,
    );
}

/// Optional system if you ever want to spawn using live settings directly.
#[allow(dead_code)]
pub fn spawn_random_planet_with_settings_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sampler_res: Res<FlatSamplerRes>,
    map: Res<MapSettings>,
    params: Res<PlanetParams>,
    settings: Res<PlanetSettings>,
) {
    spawn_planet_with_settings(
        &mut commands,
        &mut meshes,
        &mut materials,
        &sampler_res,
        &map,
        &params,
        &settings,
    );
}

fn spawn_planet_with_settings(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    sampler_res: &FlatSamplerRes,
    _map: &MapSettings,
    params: &PlanetParams,
    settings: &PlanetSettings,
) {
    let land_mesh = build_colored_planet_mesh(settings, sampler_res, params);
    let handle = meshes.add(land_mesh);

    let mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.95, 0.95), // vertex color driven
        perceptual_roughness: 0.75,
        metallic: 0.0,
        reflectance: 0.04,
        ..default()
    });

    commands.spawn((
        PlanetTag,
        Mesh3d(handle),
        MeshMaterial3d(mat),
        Transform::default(),
        GlobalTransform::default(),
        Name::new("Planet"),
    ));
}

// -----------------------------------------------------------------------------
// Mesh generation
// -----------------------------------------------------------------------------

fn build_colored_planet_mesh(
    settings: &PlanetSettings,
    sampler_res: &FlatSamplerRes,
    params: &PlanetParams,
) -> Mesh {
    // 5 is a good sweet spot; lower if you need cheaper.
    let (mut verts, indices_u32) = generate_icosphere(7);

    // Seed is shared with your flat terrain for consistent rerolls.
    let seed = sampler_res.0.seed;

    // Pre-warm multipliers
    let base_f = settings.base_freq.max(1e-4);
    let detail_f = settings.detail_freq.max(1e-4);
    let warp_f = settings.warp_freq.max(1e-4);

    // Per-vertex color + displacement
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(verts.len());

    for v in &mut verts {
        let unit = v.normalize_or_zero();

        // --- seamless 3D domain-warped coords ---
        let wu = fbm3(seed ^ 0xA1, unit.x, unit.y, unit.z, warp_f);
        let wv = fbm3(seed ^ 0xB2, unit.z, unit.x, unit.y, warp_f);
        let ww = fbm3(seed ^ 0xC3, unit.y, unit.z, unit.x, warp_f);
        let warped = Vec3::new(
            unit.x + (wu - 0.5) * settings.warp_amp,
            unit.y + (wv - 0.5) * settings.warp_amp,
            unit.z + (ww - 0.5) * settings.warp_amp,
        );

        // continents + details (seamless)
        let continent = fbm3(seed, warped.x, warped.y, warped.z, base_f);
        let detail = fbm3(seed ^ 0xCC, warped.z, warped.x, warped.y, detail_f);

        // ridged “spiky” term from detail
        let ridge = 1.0 - ((detail * 2.0 - 1.0).abs());

        // blend to n∈[0,1]
        let mut n = (continent * 0.75 + detail * 0.25).clamp(0.0, 1.0);

        // highland shaping (rough + spiky)
        let above = (n - params.sea_level).max(0.0);
        if above > 0.0 {
            let spiky = (ridge * settings.mountain_spikiness).powf(1.35); // *0.6 to tame if needed
            n = (n + above.powf(1.6) * (settings.mountain_strength * 0.7 + spiky * 0.6))
                .clamp(0.0, 1.0);
        }

        // --- slope estimate in 3D (for rock bias) ---
        // tangent frame around the normal `unit`
        let up = if unit.y.abs() < 0.99 {
            Vec3::Y
        } else {
            Vec3::X
        };
        let t1 = unit.cross(up).normalize_or_zero();
        let t2 = unit.cross(t1).normalize_or_zero();

        let eps = 0.015;
        let d0 = detail;
        let dx = fbm3(
            seed ^ 0xCC,
            warped.x + t1.x * eps,
            warped.y + t1.y * eps,
            warped.z + t1.z * eps,
            detail_f,
        );
        let dy = fbm3(
            seed ^ 0xCC,
            warped.x + t2.x * eps,
            warped.y + t2.y * eps,
            warped.z + t2.z * eps,
            detail_f,
        );
        let slope = ((dx - d0).abs() + (dy - d0).abs()).min(1.0);

        // world height + displace
        let h_world = (n - params.sea_level) * params.height_amp;
        let r = params.radius + h_world.max(0.0);
        *v = unit * r;

        // --- biome variation (subtle; seamless) ---
        let biome = fbm3(
            seed ^ 0x77,
            warped.x * 3.0,
            warped.y * 3.0,
            warped.z * 3.0,
            1.0,
        );

        // ----- COLORING -----
        // normalized land elevation above sea: 0..1
        let elev01 = (h_world / params.height_amp).clamp(0.0, 1.0);

        let (r_col, g_col, b_col) = if h_world < 0.0 {
            // water: deeper → darker
            let d = (-h_world / params.height_amp).clamp(0.0, 1.0).powf(0.6);
            let c = settings.water_deep.lerp(settings.water_shallow, 1.0 - d)
                * (0.95 + 0.1 * (biome - 0.5)); // tiny variation
            (c.x, c.y, c.z)
        } else {
            // beach band near sea
            let sand_hi = 0.08;
            let mut c = if elev01 < sand_hi {
                let k = elev01 / sand_hi;
                settings.land_sand.lerp(settings.land_grass, k)
            } else {
                // grass base with slight variation
                settings.land_grass * (0.92 + 0.18 * (biome - 0.5))
            };

            // rock line (altitude OR steep slope)
            let rock_k = ((elev01 - settings.rock_start)
                / (settings.snow_start - settings.rock_start))
                .clamp(0.0, 1.0);
            let slope_bias = (slope * 2.2).clamp(0.0, 1.0); // steep → more rock
            let to_rock = rock_k.max(slope_bias);
            c = c.lerp(settings.land_rock, to_rock);

            // snow line (very high → white)
            if elev01 > settings.snow_start {
                let snow_k =
                    ((elev01 - settings.snow_start) / (1.0 - settings.snow_start)).clamp(0.0, 1.0);
                c = c.lerp(settings.land_snow, snow_k);
            }

            c = c.clamp(Vec3::ZERO, Vec3::splat(1.0));
            (c.x, c.y, c.z)
        };

        colors.push([r_col, g_col, b_col, 1.0]);
    }

    // normals = unit vector (good enough after small displacements)
    let normals: Vec<[f32; 3]> = compute_smooth_normals(&verts, &indices_u32);

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices_u32));
    mesh
}

// -----------------------------------------------------------------------------
// Seamless 3D value-noise FBM (0..1), same hash style as your project.
// -----------------------------------------------------------------------------
#[inline]
fn h01_3(seed: u64, ix: i32, iy: i32, iz: i32) -> f32 {
    let mut v = seed
        ^ ((ix as u64).wrapping_mul(0x9E37_79B1_85EB_CA87))
        ^ ((iy as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F))
        ^ ((iz as u64).wrapping_mul(0x1656_67B1_F3C6_AF85));
    v ^= v >> 33;
    v = v.wrapping_mul(0xff51_afd7_ed55_8ccd);
    v ^= v >> 33;
    v = v.wrapping_mul(0xc4ceb9fe1a85ec53);
    v ^= v >> 33;
    (v as u32) as f32 / (u32::MAX as f32)
}
#[inline]
fn smooth3(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn value3(seed: u64, x: f32, y: f32, z: f32) -> f32 {
    let x0 = x.floor() as i32;
    let x1 = x0 + 1;
    let y0 = y.floor() as i32;
    let y1 = y0 + 1;
    let z0 = z.floor() as i32;
    let z1 = z0 + 1;

    let tx = smooth3(x - x.floor());
    let ty = smooth3(y - y.floor());
    let tz = smooth3(z - z.floor());

    let c000 = h01_3(seed, x0, y0, z0);
    let c100 = h01_3(seed, x1, y0, z0);
    let c010 = h01_3(seed, x0, y1, z0);
    let c110 = h01_3(seed, x1, y1, z0);
    let c001 = h01_3(seed, x0, y0, z1);
    let c101 = h01_3(seed, x1, y0, z1);
    let c011 = h01_3(seed, x0, y1, z1);
    let c111 = h01_3(seed, x1, y1, z1);

    let x00 = lerp(c000, c100, tx);
    let x10 = lerp(c010, c110, tx);
    let x01 = lerp(c001, c101, tx);
    let x11 = lerp(c011, c111, tx);
    let y0_ = lerp(x00, x10, ty);
    let y1_ = lerp(x01, x11, ty);
    lerp(y0_, y1_, tz)
}

fn fbm3(seed: u64, x: f32, y: f32, z: f32, base_freq: f32) -> f32 {
    let mut amp = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    let mut f = base_freq.max(1e-4);
    for _ in 0..5 {
        sum += value3(seed, x * f, y * f, z * f) * amp;
        norm += amp;
        amp *= 0.5;
        f *= 2.0;
    }
    (sum / norm).clamp(0.0, 1.0)
}

// -----------------------------------------------------------------------------
// Icosphere
// -----------------------------------------------------------------------------
fn generate_icosphere(subdivisions: u32) -> (Vec<Vec3>, Vec<u32>) {
    use std::collections::HashMap;
    let phi = (1.0 + 5.0_f32.sqrt()) * 0.5;

    let mut verts = vec![
        Vec3::new(-1.0, phi, 0.0),
        Vec3::new(1.0, phi, 0.0),
        Vec3::new(-1.0, -phi, 0.0),
        Vec3::new(1.0, -phi, 0.0),
        Vec3::new(0.0, -1.0, phi),
        Vec3::new(0.0, 1.0, phi),
        Vec3::new(0.0, -1.0, -phi),
        Vec3::new(0.0, 1.0, -phi),
        Vec3::new(phi, 0.0, -1.0),
        Vec3::new(phi, 0.0, 1.0),
        Vec3::new(-phi, 0.0, -1.0),
        Vec3::new(-phi, 0.0, 1.0),
    ];
    for v in &mut verts {
        *v = v.normalize();
    }

    let mut faces: Vec<[u32; 3]> = vec![
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    let mut cache = HashMap::new();
    let add_mid = |a: u32, b: u32, verts: &mut Vec<Vec3>, cache: &mut HashMap<(u32, u32), u32>| {
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(&i) = cache.get(&key) {
            return i;
        }
        let mid = (verts[a as usize] + verts[b as usize]) * 0.5;
        let i = verts.len() as u32;
        verts.push(mid.normalize());
        cache.insert(key, i);
        i
    };

    for _ in 0..subdivisions {
        let mut nf = Vec::with_capacity(faces.len() * 4);
        for [a, b, c] in faces.iter().copied() {
            let ab = add_mid(a, b, &mut verts, &mut cache);
            let bc = add_mid(b, c, &mut verts, &mut cache);
            let ca = add_mid(c, a, &mut verts, &mut cache);
            nf.push([a, ab, ca]);
            nf.push([b, bc, ab]);
            nf.push([c, ca, bc]);
            nf.push([ab, bc, ca]);
        }
        faces = nf;
    }

    let mut idx = Vec::with_capacity(faces.len() * 3);
    for [a, b, c] in faces {
        idx.extend_from_slice(&[a, b, c]);
    }
    (verts, idx)
}
fn compute_smooth_normals(verts: &[Vec3], indices: &[u32]) -> Vec<[f32; 3]> {
    // accumulate face normals (area-weighted via cross-product magnitude)
    let mut acc: Vec<Vec3> = vec![Vec3::ZERO; verts.len()];

    for tri in indices.chunks_exact(3) {
        let ia = tri[0] as usize;
        let ib = tri[1] as usize;
        let ic = tri[2] as usize;

        let a = verts[ia];
        let b = verts[ib];
        let c = verts[ic];

        // face normal; winding from your icosphere generator is consistent
        let n = (b - a).cross(c - a);
        if n.length_squared() > 0.0 {
            acc[ia] += n;
            acc[ib] += n;
            acc[ic] += n;
        }
    }

    acc.into_iter()
        .enumerate()
        .map(|(i, n)| {
            let nn = if n.length_squared() > 0.0 {
                n.normalize()
            } else {
                // fallback to unit-sphere normal if degenerate
                verts[i].normalize_or_zero()
            };
            nn.to_array()
        })
        .collect()
}
