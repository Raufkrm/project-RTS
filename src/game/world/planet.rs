use crate::core::planet_debug::LodDebugBands;
use crate::core::surface_model::{cube_face_dir, dir_to_face_uv, PlanetSurfaceModel, SurfaceCoord};
use crate::game::planet_surface::biome::BiomeClassifier;
use crate::game::world::sampling::FlatSamplerRes;
use crate::game::world::surface_grid::{self, SurfaceGrid};
use crate::game::world::terrain::MapSettings;
use crate::game::SunDirection;
use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::log::info;
use bevy::math::primitives::Sphere;
use bevy::pbr::{wireframe::Wireframe, MaterialExtension, StandardMaterial};
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::alpha::AlphaMode;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, PrimitiveTopology, ShaderType, TextureDimension, TextureFormat,
};
use bevy::shader::ShaderRef; // correct place for your Bevy version
use bevy_mesh::{Indices, Mesh, VertexAttributeValues};
use std::cmp::Ordering;

// -----------------------------------------------------------------------------
// Tags / params
// -----------------------------------------------------------------------------
#[derive(Component)]
pub struct PlanetTag;

#[derive(Component)]
pub struct PlanetAtmosphere;

#[derive(Component)]
pub struct PlanetClouds;

const ATMOSPHERE_SCALE_FACTOR: f32 = 1.015;
const ATMOSPHERE_ALPHA: f32 = 0.18;
const CLOUD_SCALE_FACTOR: f32 = 1.008;
const CLOUD_ALPHA: f32 = 0.08;
const CLOUD_ROTATION_SPEED: f32 = 0.012;

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct PlanetSurfaceParams {
    // ExtendedMaterial ALWAYS binds the extension uniform at @group(2) @binding(0)
    #[uniform(31)]
    pub params: PlanetSurfaceUniform,
}

#[repr(C)] // keep alignment stable
#[derive(Clone, Copy, ShaderType)] // from bevy_render::render_resource::ShaderType
pub struct PlanetSurfaceUniform {
    pub seed: u32,
    pub debug_mode: u32,
    pub _pad0: u32,
    pub _pad1: u32,
    pub sea_level: f32,
    pub base_freq: f32,
    pub detail_freq: f32,
    pub warp_freq: f32,
    pub warp_amp: f32,
    pub coast_width: f32,
    pub mountain_strength: f32,
    pub mountain_scale: f32,
    pub axial_tilt: f32,
    pub temp_shift: f32,
    pub moisture_bias_global: f32,
    pub dryness_bias_global: f32,
    pub normal_strength: f32,
    pub perceptual_roughness: f32,
    pub metallic: f32,
    pub reflectance: f32,
    pub sand_height: f32,
    pub rock_start: f32,
    pub snow_start: f32,
    pub sun_dir: Vec4,
    pub water_deep: Vec4,
    pub water_shallow: Vec4,
    pub land_sand: Vec4,
    pub land_grass: Vec4,
    pub land_rock: Vec4,
    pub land_snow: Vec4,
}

pub type PlanetSurfaceMaterial =
    bevy::pbr::ExtendedMaterial<bevy::pbr::StandardMaterial, PlanetSurfaceParams>;

#[repr(C)]
#[derive(Clone, Copy, ShaderType)]
pub struct AtmosphereUniform {
    pub color: Vec4,
    pub intensity: f32,
    pub falloff: f32,
    pub _pad: f32,
}

#[derive(Asset, AsBindGroup, TypePath, Clone)]
pub struct AtmosphereParams {
    #[uniform(31)]
    pub params: AtmosphereUniform,
}

pub type AtmosphereMaterial =
    bevy::pbr::ExtendedMaterial<bevy::pbr::StandardMaterial, AtmosphereParams>;

impl AtmosphereParams {
    pub fn new(color: LinearRgba, intensity: f32, falloff: f32) -> Self {
        Self {
            params: AtmosphereUniform {
                color: color.to_vec4(),
                intensity,
                falloff,
                _pad: 0.0,
            },
        }
    }
}

impl MaterialExtension for AtmosphereParams {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Path("shaders/planet_atmosphere.wgsl".into())
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/planet_atmosphere.wgsl".into())
    }
}

impl Default for PlanetSurfaceParams {
    fn default() -> Self {
        let default_sun = Vec3::new(0.32, 0.78, 0.54).normalize_or_zero();
        Self {
            params: PlanetSurfaceUniform {
                seed: 0,
                debug_mode: 0,
                _pad0: 0,
                _pad1: 0,
                sea_level: 0.5,
                base_freq: 0.65,
                detail_freq: 1.0,
                warp_freq: 1.6,
                warp_amp: 0.04,
                coast_width: 0.028,
                mountain_strength: 0.42,
                mountain_scale: 0.6,
                axial_tilt: 0.0,
                temp_shift: 0.0,
                moisture_bias_global: 0.0,
                dryness_bias_global: 0.0,
                normal_strength: 0.9,
                perceptual_roughness: 0.75,
                metallic: 0.0,
                reflectance: 0.04,
                sand_height: 0.08,
                rock_start: 0.54,
                snow_start: 0.82,
                sun_dir: default_sun.extend(0.0),
                water_deep: srgb_to_linear_vec3(Vec3::new(0.04, 0.11, 0.28)).extend(1.0),
                water_shallow: srgb_to_linear_vec3(Vec3::new(0.27, 0.56, 0.78)).extend(1.0),
                land_sand: srgb_to_linear_vec3(Vec3::new(0.88, 0.78, 0.52)).extend(1.0),
                land_grass: srgb_to_linear_vec3(Vec3::new(0.28, 0.58, 0.32)).extend(1.0),
                land_rock: srgb_to_linear_vec3(Vec3::new(0.48, 0.5, 0.54)).extend(1.0),
                land_snow: srgb_to_linear_vec3(Vec3::new(0.95, 0.98, 1.0)).extend(1.0),
            },
        }
    }
}

fn srgb_channel_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb_to_linear_vec3(color: Vec3) -> Vec3 {
    Vec3::new(
        srgb_channel_to_linear(color.x),
        srgb_channel_to_linear(color.y),
        srgb_channel_to_linear(color.z),
    )
}

impl PlanetSurfaceParams {
    pub fn from_settings(
        seed: u64,
        params: &PlanetParams,
        settings: &PlanetSettings,
        sun_dir: Vec3,
        debug_mode: PlanetDebugMode,
    ) -> Self {
        let seed_mix = (seed as u32) ^ ((seed >> 32) as u32);
        let climate = ClimateModel::new(seed, params, settings);
        let dir = sun_dir.normalize_or_zero();
        Self {
            params: PlanetSurfaceUniform {
                seed: seed_mix,
                debug_mode: debug_mode as u32,
                _pad0: 0,
                _pad1: 0,
                sea_level: climate.sea_level,
                base_freq: climate.base_f,
                detail_freq: climate.detail_f,
                warp_freq: climate.warp_f,
                warp_amp: climate.warp_amp,
                coast_width: settings.coast_width,
                mountain_strength: settings.mountain_strength.clamp(0.0, 1.0),
                mountain_scale: climate.mountain_scale,
                axial_tilt: climate.axial_tilt,
                temp_shift: climate.temp_shift,
                moisture_bias_global: climate.moisture_bias_global,
                dryness_bias_global: climate.dryness_bias_global,
                normal_strength: 0.9,
                perceptual_roughness: 0.75,
                metallic: 0.0,
                reflectance: 0.04,
                sand_height: settings.sand_height,
                rock_start: settings.rock_start,
                snow_start: settings.snow_start,
                sun_dir: dir.extend(0.0),
                water_deep: srgb_to_linear_vec3(settings.water_deep).extend(1.0),
                water_shallow: srgb_to_linear_vec3(settings.water_shallow).extend(1.0),
                land_sand: srgb_to_linear_vec3(settings.land_sand).extend(1.0),
                land_grass: srgb_to_linear_vec3(settings.land_grass).extend(1.0),
                land_rock: srgb_to_linear_vec3(settings.land_rock).extend(1.0),
                land_snow: srgb_to_linear_vec3(settings.land_snow).extend(1.0),
            },
        }
    }
}

impl MaterialExtension for PlanetSurfaceParams {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/planet_surface.wgsl".into())
    }

    fn deferred_fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/planet_surface.wgsl".into())
    }
}

/// Stable params the rest of the game already uses (radius + sea level).
#[derive(Resource, Clone, Copy)]
pub struct PlanetParams {
    /// Base sphere radius in world units (sea sits at this radius).
    pub radius: f32,
    /// World-space vertical amplitude used when displacing above sea.
    pub height_amp: f32,
    /// Sea level in noise space [0..1].
    pub sea_level: f32,
    /// Planetary rotation in degrees around +Y (0 = prime meridian facing sun).
    pub rotation_deg: f32,
}
impl Default for PlanetParams {
    fn default() -> Self {
        Self {
            radius: 500000.0,
            height_amp: 1200.0,
            sea_level: 0.50,
            rotation_deg: 0.0,
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
    /// Elevation (0..1 above sea) where sand fades to soil/grass
    pub sand_height: f32,
    /// Elevation (0..1 above sea) where grass fades to rock
    pub rock_start: f32,
    /// Elevation (0..1 above sea) where rock fades to snow
    pub snow_start: f32,
    pub coast_width: f32,
}
impl Default for PlanetSettings {
    fn default() -> Self {
        Self {
            base_freq: 0.65,
            detail_freq: 1.0,
            warp_freq: 1.6,
            warp_amp: 0.04,
            coast_width: 0.028,
            mountain_strength: 0.35,
            mountain_spikiness: 0.55, // richer ridges by default

            water_deep: Vec3::new(0.04, 0.11, 0.28),
            water_shallow: Vec3::new(0.27, 0.56, 0.78),

            land_sand: Vec3::new(0.88, 0.78, 0.52),
            land_grass: Vec3::new(0.28, 0.58, 0.32),

            land_rock: Vec3::new(0.48, 0.5, 0.54), // cooler granite
            land_snow: Vec3::new(0.95, 0.98, 1.0), // crisp snow
            sand_height: 0.08,                     // sandy shelf depth
            rock_start: 0.54,                      // grass fades to rock
            snow_start: 0.82,                      // rock fades to snow
        }
    }
}

pub const DEFAULT_BIOME_MAP_RESOLUTION: u32 = 256;
pub const SURFACE_GRID_RESOLUTION: u32 = 512; // start with 512; can tune later
pub const SKIRT_LAND_MASK_BITS: u32 = 0xFFFF;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CubeFace {
    PositiveX = 0,
    NegativeX = 1,
    PositiveY = 2,
    NegativeY = 3,
    PositiveZ = 4,
    NegativeZ = 5,
}

impl CubeFace {
    pub const ALL: [CubeFace; 6] = [
        CubeFace::PositiveX,
        CubeFace::NegativeX,
        CubeFace::PositiveY,
        CubeFace::NegativeY,
        CubeFace::PositiveZ,
        CubeFace::NegativeZ,
    ];

    #[inline]
    pub const fn as_index(self) -> usize {
        self as usize
    }
}

#[derive(Resource, Clone)]
pub struct GlobalBiomeMap {
    pub resolution: u32,
    pub faces: [Vec<u8>; 6],
}

impl GlobalBiomeMap {
    #[inline]
    pub fn texels_for_face(&self, face: CubeFace) -> &[u8] {
        &self.faces[face.as_index()]
    }

    #[inline]
    pub fn texels_for_face_mut(&mut self, face: CubeFace) -> &mut [u8] {
        &mut self.faces[face.as_index()]
    }

    #[inline]
    pub fn texel_index(&self, x: u32, y: u32) -> usize {
        (y * self.resolution + x) as usize
    }

    #[inline]
    pub fn sample_biome(&self, dir: Vec3) -> u8 {
        if dir.length_squared() <= f32::EPSILON {
            return 0;
        }
        let normalized = dir.normalize_or_zero();
        let (face, uv) = dir_to_face_uv(normalized);
        let coord = SurfaceCoord {
            face,
            uv,
            height: 0.0,
        };
        sample_biome_face_uv(self, coord)
    }
}

#[inline]
fn sample_biome_face_uv(map: &GlobalBiomeMap, coord: SurfaceCoord) -> u8 {
    if map.resolution == 0 {
        return 0;
    }
    let res = map.resolution;
    let max_idx = res - 1;
    let sample_idx = |value: f32| -> u32 {
        let clamped = value.clamp(0.0, 1.0);
        let scaled = clamped * res as f32;
        scaled.clamp(0.0, max_idx as f32).floor() as u32
    };
    let x = sample_idx(coord.uv.x);
    let y = sample_idx(coord.uv.y);
    let idx = (y * res + x) as usize;
    let face_index = coord.face.min(5) as usize;
    let texels = &map.faces[face_index];
    texels.get(idx).copied().unwrap_or(0)
}

#[derive(Clone, Copy, Debug)]
pub struct PlanetClimateSample {
    pub uv1: [f32; 2],
    pub packed: [f32; 4],
}

#[allow(dead_code)]
fn cube_face_direction(face: CubeFace, u: f32, v: f32) -> Vec3 {
    let sx = u * 2.0 - 1.0;
    let sy = v * 2.0 - 1.0;
    match face {
        CubeFace::PositiveX => Vec3::new(1.0, -sy, -sx),
        CubeFace::NegativeX => Vec3::new(-1.0, -sy, sx),
        CubeFace::PositiveY => Vec3::new(sx, 1.0, sy),
        CubeFace::NegativeY => Vec3::new(sx, -1.0, -sy),
        CubeFace::PositiveZ => Vec3::new(sx, -sy, 1.0),
        CubeFace::NegativeZ => Vec3::new(-sx, -sy, -1.0),
    }
    .normalize_or_zero()
}

#[allow(dead_code)]
fn direction_to_cube_face_uv(dir: Vec3) -> (CubeFace, f32, f32) {
    let abs = dir.abs();
    let (face, major, uc, vc) = if abs.x >= abs.y && abs.x >= abs.z {
        if dir.x >= 0.0 {
            (CubeFace::PositiveX, abs.x, -dir.z, dir.y)
        } else {
            (CubeFace::NegativeX, abs.x, dir.z, dir.y)
        }
    } else if abs.y >= abs.x && abs.y >= abs.z {
        if dir.y >= 0.0 {
            (CubeFace::PositiveY, abs.y, dir.x, dir.z)
        } else {
            (CubeFace::NegativeY, abs.y, dir.x, -dir.z)
        }
    } else if dir.z >= 0.0 {
        (CubeFace::PositiveZ, abs.z, dir.x, -dir.y)
    } else {
        (CubeFace::NegativeZ, abs.z, -dir.x, -dir.y)
    };

    if major <= f32::EPSILON {
        return (CubeFace::PositiveZ, 0.5, 0.5);
    }

    let u = 0.5 * (uc / major + 1.0);
    let v = 0.5 * (vc / major + 1.0);
    (face, u.clamp(0.0, 1.0), v.clamp(0.0, 1.0))
}

pub fn bake_global_biome_map(
    climate: &ClimateModel,
    classifier: &BiomeClassifier,
    resolution: u32,
) -> GlobalBiomeMap {
    let face_len = (resolution * resolution) as usize;
    let mut faces = std::array::from_fn(|_| vec![0u8; face_len]);

    for face in CubeFace::ALL {
        let texels = &mut faces[face.as_index()];
        for y in 0..resolution {
            let v = (y as f32 + 0.5) / resolution as f32;
            for x in 0..resolution {
                let u = (x as f32 + 0.5) / resolution as f32;
                let uv = Vec2::new(u, v);
                let dir = cube_face_dir(face.as_index() as u8, uv);
                let sample = climate.sample(dir);
                let biome = classifier.classify_sample(&sample);
                texels[(y * resolution + x) as usize] = biome as u8;
            }
        }
    }

    GlobalBiomeMap { resolution, faces }
}

fn bake_surface_grid(
    climate: &ClimateModel,
    classifier: &BiomeClassifier,
    resolution: u32,
) -> SurfaceGrid {
    let mut grid = SurfaceGrid::default();
    if resolution == 0 {
        return grid;
    }

    grid.resolution = resolution;
    let face_len = (resolution * resolution) as usize;
    for face in 0..6 {
        grid.heights[face].resize(face_len, 0.0);
        grid.biomes[face].resize(face_len, 0);
        for y in 0..resolution {
            let v = (y as f32 + 0.5) / resolution as f32;
            for x in 0..resolution {
                let u = (x as f32 + 0.5) / resolution as f32;
                let uv = Vec2::new(u, v);
                let dir = cube_face_dir(face as u8, uv).normalize_or_zero();
                let eval = climate.evaluate(dir);
                let sample = PlanetClimateSample::from_eval(&eval);
                let biome = classifier.classify_sample(&sample) as u8;
                let index = surface_grid::idx(resolution, x, y);
                grid.heights[face][index] = eval.mountain_offset;
                grid.biomes[face][index] = biome;
            }
        }
    }

    grid
}

pub(crate) struct ClimateModel<'a> {
    params: &'a PlanetParams,
    settings: &'a PlanetSettings,
    seed: u64,
    base_f: f32,
    detail_f: f32,
    warp_f: f32,
    warp_amp: f32,
    mountain_scale: f32,
    axial_tilt: f32,
    temp_shift: f32,
    moisture_bias_global: f32,
    dryness_bias_global: f32,
    sea_level: f32,
}

#[derive(Clone, Copy)]
struct ClimateFields {
    continent_value: f32,
    land_mask: f32,
    macro_relief: f32,
    mountain_seed: f32,
    erosion: f32,
    detail: f32,
    moisture_noise: f32,
}

struct ClimateEval {
    final_pos: Vec3,
    continent_value: f32,
    land_mask: f32,
    height01: f32,
    elev01: f32,
    slope: f32,
    moisture: f32,
    temperature: f32,
    dryness: f32,
    polar_mix: f32,
    depth: f32,
    shore_mix: f32,
    micro_relief: f32,
    ridge_light: f32,
    valley_shadow: f32,
    slope_highlight: f32,
    lat_abs: f32,
    polar_cap: f32,
    snow_score: f32,
    allow_alpine: bool,
    coast_band: f32,
    mountain_mask: f32,
    mountain_offset: f32,
    gradient: Vec3,
}

impl PlanetClimateSample {
    fn from_eval(eval: &ClimateEval) -> Self {
        let packed_md = pack_pair(eval.moisture, eval.dryness);
        let packed_cs = pack_pair(eval.coast_band, eval.snow_score);
        let packed_ht = pack_pair(eval.height01, eval.temperature);
        let continent_norm = ((eval.continent_value + 1.0) * 0.5).clamp(0.0, 1.0);
        let packed_sl = pack_pair(eval.slope.clamp(0.0, 1.0), continent_norm);
        let packed_mr = pack_pair(
            eval.mountain_mask.clamp(0.0, 1.0),
            eval.ridge_light.clamp(0.0, 1.0),
        );
        let depth_or_valley = if eval.depth > 1e-4 {
            eval.depth
        } else {
            eval.valley_shadow.clamp(0.0, 1.0)
        };
        let packed_lv = pack_pair(depth_or_valley, eval.land_mask.clamp(0.0, 1.0));

        Self {
            uv1: [packed_md, packed_cs],
            packed: [packed_ht, packed_sl, packed_mr, packed_lv],
        }
    }
}

impl<'a> ClimateModel<'a> {
    fn new(seed: u64, params: &'a PlanetParams, settings: &'a PlanetSettings) -> Self {
        let base_f = settings.base_freq.max(1e-5);
        let detail_f = settings.detail_freq.max(1e-5);
        let warp_f = settings.warp_freq.max(1e-5);
        let mountain_detail_factor = (detail_f / 1.0).sqrt().clamp(0.35, 1.5);
        let adaptive_mountain = settings.mountain_strength.clamp(0.0, 1.0) * mountain_detail_factor;
        let mountain_scale = 0.3 + adaptive_mountain * 0.7;
        let axial_noise = h01_3(seed ^ 0xA0, 0, 0, 0);
        let axial_tilt = 0.18 + axial_noise * 0.22;
        let temp_shift = (h01_3(seed ^ 0xA1, 1, 0, 0) - 0.5) * 0.18;
        let moisture_bias_global = (h01_3(seed ^ 0xA2, 2, 0, 0) - 0.5) * 0.35;
        let dryness_bias_global = (h01_3(seed ^ 0xA3, 3, 0, 0) - 0.5) * 0.3;
        let sea_bias = (h01_3(seed ^ 0xA4, 4, 0, 0) - 0.5) * 0.12;
        let sea_level = (params.sea_level + sea_bias).clamp(0.22, 0.58);

        Self {
            params,
            settings,
            seed,
            base_f,
            detail_f,
            warp_f,
            warp_amp: settings.warp_amp,
            mountain_scale,
            axial_tilt,
            temp_shift,
            moisture_bias_global,
            dryness_bias_global,
            sea_level,
        }
    }

    pub fn sample(&self, dir: Vec3) -> PlanetClimateSample {
        let eval = self.evaluate(dir.normalize_or_zero());
        PlanetClimateSample::from_eval(&eval)
    }

    fn sample_fields(&self, coord: Vec3) -> ClimateFields {
        let to_signed = |v: f32| v * 2.0 - 1.0;

        let plate_a = to_signed(fbm3(
            self.seed,
            coord.x * 0.55,
            coord.y * 0.53,
            coord.z * 0.57,
            self.base_f * 0.55,
        ));
        let plate_b = to_signed(fbm3(
            self.seed ^ 0x11,
            coord.z * 0.48,
            coord.x * 0.52,
            coord.y * 0.51,
            self.base_f * 0.45,
        ));
        let coast_noise = to_signed(fbm3(
            self.seed ^ 0x21,
            coord.x * 1.2,
            coord.y * 1.1,
            coord.z * 1.15,
            self.base_f * 1.05,
        ));
        let continent_value = plate_a * 0.75 + plate_b * 0.45 + coast_noise * 0.28 - 0.05;
        let land_mask = smoothstep(-0.18, 0.22, continent_value);

        let macro_relief = fbm3(
            self.seed ^ 0x33,
            coord.x * 0.95,
            coord.y * 0.85,
            coord.z * 0.9,
            self.base_f * 0.9,
        )
        .powf(1.6);

        let ridge_noise = fbm3(
            self.seed ^ 0x44,
            coord.x * 2.4,
            coord.y * 2.6,
            coord.z * 2.2,
            self.base_f * 2.2,
        );
        let ridged = (1.0 - (ridge_noise * 2.0 - 1.0).abs()).powf(2.4);
        let mountain_seed = (ridged * land_mask.powf(2.2)).clamp(0.0, 1.0);

        let erosion = fbm3(
            self.seed ^ 0x55,
            coord.y * 1.2,
            coord.z * 1.15,
            coord.x * 1.1,
            self.base_f * 1.2,
        );

        let detail = fbm3(
            self.seed ^ 0x66,
            coord.x * 6.5,
            coord.y * 6.0,
            coord.z * 6.2,
            self.detail_f * 2.1,
        );

        let moisture_noise = fbm3(
            self.seed ^ 0x77,
            coord.x * 1.6,
            coord.y * 1.7,
            coord.z * 1.55,
            self.detail_f * 1.3,
        );

        ClimateFields {
            continent_value,
            land_mask,
            macro_relief,
            mountain_seed,
            erosion,
            detail,
            moisture_noise,
        }
    }

    fn compose_height(&self, sample: &ClimateFields) -> f32 {
        let mut height = sample.continent_value * 0.48 + (sample.land_mask - 0.5) * 0.65;
        height += (sample.macro_relief - 0.5) * 0.42;
        height += (sample.erosion - 0.5) * 0.22;
        height += sample.mountain_seed.powf(0.9) * self.mountain_scale * 1.12;
        height += (sample.detail - 0.5) * 0.09;
        height += sample.land_mask.powf(3.2) * 0.08;
        (height * 0.58 + 0.5).clamp(0.0, 1.0)
    }

    fn clamp_height_to_surface(&self, sample: &ClimateFields, raw_height: f32) -> f32 {
        if sample.land_mask > 0.5 {
            let inland = ((sample.land_mask - 0.5) / 0.5).clamp(0.0, 1.0);
            let uplift = inland.powf(1.35) * 0.08;
            raw_height.max((self.sea_level + uplift).min(0.99))
        } else {
            self.sea_level
        }
    }

    fn evaluate(&self, unit: Vec3) -> ClimateEval {
        let warp_offset = Vec3::new(
            fbm3(self.seed ^ 0xA1, unit.x, unit.y, unit.z, self.warp_f) - 0.5,
            fbm3(self.seed ^ 0xB2, unit.z, unit.x, unit.y, self.warp_f) - 0.5,
            fbm3(self.seed ^ 0xC3, unit.y, unit.z, unit.x, self.warp_f) - 0.5,
        );
        let warped = unit + warp_offset * self.warp_amp;

        let center_sample = self.sample_fields(warped);
        let raw_height01 = self.compose_height(&center_sample);
        let height01 = self.clamp_height_to_surface(&center_sample, raw_height01);

        let eps = 0.012;
        let sample_px = self.sample_fields(warped + Vec3::new(eps, 0.0, 0.0));
        let sample_mx = self.sample_fields(warped - Vec3::new(eps, 0.0, 0.0));
        let sample_py = self.sample_fields(warped + Vec3::new(0.0, eps, 0.0));
        let sample_my = self.sample_fields(warped - Vec3::new(0.0, eps, 0.0));
        let sample_pz = self.sample_fields(warped + Vec3::new(0.0, 0.0, eps));
        let sample_mz = self.sample_fields(warped - Vec3::new(0.0, 0.0, eps));

        let hx1_raw = self.compose_height(&sample_px);
        let hx0_raw = self.compose_height(&sample_mx);
        let hy1_raw = self.compose_height(&sample_py);
        let hy0_raw = self.compose_height(&sample_my);
        let hz1_raw = self.compose_height(&sample_pz);
        let hz0_raw = self.compose_height(&sample_mz);

        let hx1 = self.clamp_height_to_surface(&sample_px, hx1_raw);
        let hx0 = self.clamp_height_to_surface(&sample_mx, hx0_raw);
        let hy1 = self.clamp_height_to_surface(&sample_py, hy1_raw);
        let hy0 = self.clamp_height_to_surface(&sample_my, hy0_raw);
        let hz1 = self.clamp_height_to_surface(&sample_pz, hz1_raw);
        let hz0 = self.clamp_height_to_surface(&sample_mz, hz0_raw);

        let grad_x = (hx1 - hx0) / (2.0 * eps);
        let grad_y = (hy1 - hy0) / (2.0 * eps);
        let grad_z = (hz1 - hz0) / (2.0 * eps);

        let slope = (grad_x * grad_x + grad_y * grad_y + grad_z * grad_z)
            .sqrt()
            .clamp(0.0, 1.4)
            .min(1.0);
        let avg_height = (hx1 + hx0 + hy1 + hy0 + hz1 + hz0) / 6.0;

        let elev01 = (height01 - self.sea_level).max(0.0) / (1.0 - self.sea_level).max(1e-3);
        let land_factor = center_sample.land_mask.clamp(0.0, 1.0);
        let mountain_mask = ((center_sample.mountain_seed - 0.65).max(0.0)).powf(2.2) * land_factor;
        let mountain_offset = mountain_mask * self.params.height_amp * self.mountain_scale * 0.4;
        let final_pos = unit * (self.params.radius + mountain_offset);

        let lat_abs = unit.y.abs();
        let land_grad_x = (sample_px.land_mask - sample_mx.land_mask).abs();
        let land_grad_y = (sample_py.land_mask - sample_my.land_mask).abs();
        let land_grad_z = (sample_pz.land_mask - sample_mz.land_mask).abs();
        let coast_gradient =
            ((land_grad_x * land_grad_x + land_grad_y * land_grad_y + land_grad_z * land_grad_z)
                / 3.0)
                .sqrt()
                .clamp(0.0, 1.0);

        let coast_band =
            (center_sample.land_mask * (1.0 - center_sample.land_mask) * 4.0).clamp(0.0, 1.0);
        let interior = center_sample.land_mask.powf(3.0);
        let dryness_distance = (1.0 - (coast_band + coast_gradient * 0.6).clamp(0.0, 1.2)).max(0.0);
        let dryness_base = (interior.powf(1.1) * dryness_distance.powf(1.2)).clamp(0.0, 1.0);
        let dryness_noise = (center_sample.detail - 0.5) * 0.2;
        let dryness = (dryness_base + self.dryness_bias_global + dryness_noise).clamp(0.0, 1.0);

        let rain_shadow = (center_sample.mountain_seed * dryness_distance).powf(1.3);
        let mut moisture = (center_sample.moisture_noise * 0.55 + coast_band * 1.05
            - interior * 0.35)
            .clamp(0.0, 1.0);
        moisture = (moisture - rain_shadow * 0.45).clamp(0.0, 1.0);
        moisture += (1.0 - interior) * 0.04;
        moisture += self.moisture_bias_global;
        moisture = moisture.clamp(0.0, 1.0);
        moisture = (moisture - dryness * 0.25).clamp(0.0, 1.0);

        let lat_scale = lat_abs;
        let tilt_cooling = (self.axial_tilt * 1.15) * lat_scale.powf(1.1);
        let base_temp =
            (1.0 - lat_scale.powf(1.45) - tilt_cooling + self.temp_shift).clamp(0.0, 1.0);
        let altitude_cooling = elev01.powf(0.65) * 0.7 + slope.powf(0.75) * 0.15;
        let temp_variation = (center_sample.detail - 0.5) * 0.18;
        let moisture_cooling = moisture * 0.1;
        let mut temperature = (base_temp + temp_variation - altitude_cooling - moisture_cooling
            + dryness * 0.05)
            .clamp(0.0, 1.0);
        temperature -= smoothstep(0.72, 0.98, lat_abs) * 0.18;
        temperature = temperature.clamp(0.0, 1.0);

        let depth = ((self.sea_level - raw_height01) / self.sea_level.max(1e-3)).clamp(0.0, 1.0);
        let shore_mix = (1.0 - depth).powf(0.6);
        let polar_mix = smoothstep(0.68, 0.95, lat_abs);
        let curvature_signed = height01 - avg_height;
        let ridge_light = curvature_signed.max(0.0).powf(0.8) * 0.35;
        let valley_shadow = (-curvature_signed).max(0.0).powf(0.9) * 0.45;
        let micro_relief = (center_sample.detail - 0.5) * 0.18;
        let slope_highlight = slope.powf(0.6) * 0.15;

        let polar_cap = smoothstep(0.88, 1.0, lat_scale);
        let high_altitude = smoothstep(self.settings.rock_start + 0.12, 0.98, elev01);
        let cold_factor = ((0.26 - temperature) / 0.26).clamp(0.0, 1.0);
        let moisture_factor = ((moisture - 0.55) / 0.45).clamp(0.0, 1.0);
        let mut snow_score =
            (polar_cap * 0.7 + high_altitude * 0.35 + cold_factor * 0.55).clamp(0.0, 1.0);
        let temp_suppression = ((0.45 - temperature) / 0.25).clamp(0.0, 1.0);
        snow_score *= temp_suppression;
        snow_score *= moisture_factor;
        snow_score *= 1.0 - dryness.powf(1.5);
        let allow_alpine = temp_suppression > 0.0
            && high_altitude > 0.6
            && cold_factor > 0.6
            && moisture_factor > 0.4;

        ClimateEval {
            final_pos,
            continent_value: center_sample.continent_value,
            land_mask: center_sample.land_mask,
            height01,
            elev01,
            slope,
            moisture,
            temperature,
            dryness,
            polar_mix,
            depth,
            shore_mix,
            micro_relief,
            ridge_light,
            valley_shadow,
            slope_highlight,
            lat_abs,
            polar_cap,
            snow_score,
            allow_alpine,
            coast_band,
            mountain_mask,
            mountain_offset,
            gradient: Vec3::new(grad_x, grad_y, grad_z),
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct PlanetClimateSummary {
    pub water_fraction: f32,
    pub deep_water_fraction: f32,
    pub coastline_fraction: f32,
    pub snow_land_fraction: f32,
    pub avg_land_temperature: f32,
    pub avg_land_moisture: f32,
    pub avg_land_dryness: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GuardrailAdjustment {
    pub sea_level: Option<f32>,
    pub height_amp: Option<f32>,
    pub mountain_strength: Option<f32>,
}
impl GuardrailAdjustment {
    pub fn is_empty(&self) -> bool {
        self.sea_level.is_none() && self.height_amp.is_none() && self.mountain_strength.is_none()
    }
}

const WATER_TARGET_MIN: f32 = 0.45;
const WATER_TARGET_MAX: f32 = 0.55;
const SNOW_TARGET_MAX: f32 = 0.32;
const SNOW_TARGET_MIN: f32 = 0.02;
const DRYNESS_TARGET_MIN: f32 = 0.35;
const DRYNESS_TARGET_MAX: f32 = 0.65;
pub fn average_climate_summary(summaries: &[PlanetClimateSummary]) -> Option<PlanetClimateSummary> {
    if summaries.is_empty() {
        return None;
    }
    let mut water = 0.0;
    let mut deep = 0.0;
    let mut coast = 0.0;
    let mut snow = 0.0;
    let mut temp = 0.0;
    let mut moisture = 0.0;
    let mut dryness = 0.0;
    for summary in summaries {
        water += summary.water_fraction;
        deep += summary.deep_water_fraction;
        coast += summary.coastline_fraction;
        snow += summary.snow_land_fraction;
        temp += summary.avg_land_temperature;
        moisture += summary.avg_land_moisture;
        dryness += summary.avg_land_dryness;
    }
    let count = summaries.len() as f32;
    Some(PlanetClimateSummary {
        water_fraction: (water / count).clamp(0.0, 1.0),
        deep_water_fraction: (deep / count).clamp(0.0, 1.0),
        coastline_fraction: (coast / count).clamp(0.0, 1.0),
        snow_land_fraction: (snow / count).clamp(0.0, 1.0),
        avg_land_temperature: (temp / count).clamp(0.0, 1.0),
        avg_land_moisture: (moisture / count).clamp(0.0, 1.0),
        avg_land_dryness: (dryness / count).clamp(0.0, 1.0),
    })
}

pub fn guardrail_adjustment_from_summary(
    params: &PlanetParams,
    settings: &PlanetSettings,
    summary: &PlanetClimateSummary,
) -> GuardrailAdjustment {
    let mut adjustment = GuardrailAdjustment::default();
    let mut sea_target = params.sea_level;
    let mut height_target = params.height_amp;
    let mut mountain_target = settings.mountain_strength;

    let mut changed_sea = false;
    let mut changed_height = false;
    let mut changed_mountain = false;

    if summary.water_fraction > WATER_TARGET_MAX {
        let delta = ((summary.water_fraction - WATER_TARGET_MAX) * 0.6).clamp(0.0, 0.08);
        sea_target = (sea_target - delta).clamp(0.2, 0.78);
        changed_sea = true;
    } else if summary.water_fraction < WATER_TARGET_MIN {
        let delta = ((WATER_TARGET_MIN - summary.water_fraction) * 0.6).clamp(0.0, 0.08);
        sea_target = (sea_target + delta).clamp(0.2, 0.78);
        changed_sea = true;
    }

    if summary.snow_land_fraction > SNOW_TARGET_MAX {
        let factor = ((summary.snow_land_fraction - SNOW_TARGET_MAX) / (0.5 - SNOW_TARGET_MAX))
            .clamp(0.0, 1.0);
        height_target *= 1.0 - 0.18 * factor;
        height_target = height_target.max(50.0);
        mountain_target *= 1.0 - 0.35 * factor;
        mountain_target = mountain_target.clamp(0.05, 0.9);
        changed_height = true;
        changed_mountain = true;
    } else if summary.snow_land_fraction < SNOW_TARGET_MIN {
        let factor = ((SNOW_TARGET_MIN - summary.snow_land_fraction) / SNOW_TARGET_MIN.max(1e-3))
            .clamp(0.0, 1.0);
        height_target *= 1.0 + 0.12 * factor;
        mountain_target *= 1.0 + 0.25 * factor;
        height_target = height_target.min(params.height_amp * 1.35);
        mountain_target = mountain_target.clamp(0.05, 1.0);
        changed_height = true;
        changed_mountain = true;
    }

    if summary.avg_land_dryness > DRYNESS_TARGET_MAX {
        let factor = ((summary.avg_land_dryness - DRYNESS_TARGET_MAX) / (1.0 - DRYNESS_TARGET_MAX))
            .clamp(0.0, 1.0);
        mountain_target *= 1.0 - 0.25 * factor;
        mountain_target = mountain_target.clamp(0.05, 0.9);
        changed_mountain = true;
    } else if summary.avg_land_dryness < DRYNESS_TARGET_MIN {
        let factor =
            ((DRYNESS_TARGET_MIN - summary.avg_land_dryness) / DRYNESS_TARGET_MIN).clamp(0.0, 1.0);
        mountain_target += 0.25 * factor;
        mountain_target = mountain_target.clamp(0.05, 1.0);
        changed_mountain = true;
    }

    if changed_sea {
        adjustment.sea_level = Some(sea_target);
    }
    if changed_height {
        adjustment.height_amp = Some(height_target);
    }
    if changed_mountain {
        adjustment.mountain_strength = Some(mountain_target);
    }

    adjustment
}

pub fn guardrail_adjustment_from_summaries(
    params: &PlanetParams,
    settings: &PlanetSettings,
    summaries: &[PlanetClimateSummary],
) -> Option<GuardrailAdjustment> {
    let avg = average_climate_summary(summaries)?;
    let adjustment = guardrail_adjustment_from_summary(params, settings, &avg);
    (!adjustment.is_empty()).then_some(adjustment)
}

pub fn apply_guardrail_adjustment(
    params: &mut PlanetParams,
    settings: &mut PlanetSettings,
    adjustment: &GuardrailAdjustment,
) {
    if let Some(sea) = adjustment.sea_level {
        params.sea_level = sea.clamp(0.2, 0.78);
    }
    if let Some(height) = adjustment.height_amp {
        params.height_amp = height.max(1.0);
    }
    if let Some(mountain) = adjustment.mountain_strength {
        settings.mountain_strength = mountain.clamp(0.0, 1.0);
    }
}

pub fn analyze_planet_climate(
    seed: u64,
    params: &PlanetParams,
    settings: &PlanetSettings,
) -> PlanetClimateSummary {
    const ANALYSIS_SUBDIV: u32 = 4;
    let climate = ClimateModel::new(seed, params, settings);
    let (dirs, _) = generate_icosphere(ANALYSIS_SUBDIV);

    let mut water = 0usize;
    let mut deep_water = 0usize;
    let mut coast_sum = 0.0_f32;
    let mut land = 0usize;
    let mut temp_sum = 0.0_f32;
    let mut moisture_sum = 0.0_f32;
    let mut dryness_sum = 0.0_f32;
    let mut snow_sum = 0.0_f32;

    for dir in dirs {
        let eval = climate.evaluate(dir.normalize_or_zero());
        coast_sum += eval.coast_band;
        if eval.height01 < climate.sea_level {
            water += 1;
            if eval.depth > 0.6 {
                deep_water += 1;
            }
        } else {
            land += 1;
            temp_sum += eval.temperature;
            moisture_sum += eval.moisture;
            dryness_sum += eval.dryness;
            let temp_suppression = ((0.45 - eval.temperature) / 0.25).clamp(0.0, 1.0);
            let effective_snow = eval.snow_score * temp_suppression;
            if effective_snow > 0.62 && (eval.lat_abs > 0.88 || eval.allow_alpine) {
                let snow_k = ((effective_snow - 0.62) / 0.38).clamp(0.0, 1.0);
                snow_sum += snow_k;
            }
        }
    }

    let total = (water + land).max(1) as f32;
    let water_fraction = water as f32 / total;
    let deep_water_fraction = if water > 0 {
        deep_water as f32 / water as f32
    } else {
        0.0
    };
    let coastline_fraction = (coast_sum / total).clamp(0.0, 1.0);
    let snow_land_fraction = if land > 0 {
        (snow_sum / land as f32).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let avg_land_temperature = if land > 0 {
        temp_sum / land as f32
    } else {
        0.0
    };
    let avg_land_moisture = if land > 0 {
        moisture_sum / land as f32
    } else {
        0.0
    };
    let avg_land_dryness = if land > 0 {
        dryness_sum / land as f32
    } else {
        0.0
    };

    PlanetClimateSummary {
        water_fraction,
        deep_water_fraction,
        coastline_fraction,
        snow_land_fraction,
        avg_land_temperature,
        avg_land_moisture,
        avg_land_dryness,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanetDebugMode {
    None,
    Continents,
    LandMask,
    Elevation,
    Moisture,
    Temperature,
    Slope,
}
impl PlanetDebugMode {
    pub fn next(self) -> Self {
        use PlanetDebugMode::*;
        match self {
            None => Continents,
            Continents => LandMask,
            LandMask => Elevation,
            Elevation => Moisture,
            Moisture => Temperature,
            Temperature => Slope,
            Slope => None,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct PlanetDebugConfig {
    pub mode: PlanetDebugMode,
}
impl Default for PlanetDebugConfig {
    fn default() -> Self {
        Self {
            mode: PlanetDebugMode::None,
        }
    }
}

// -----------------------------------------------------------------------------
// Plugin ΓÇô registers resources (so theyΓÇÖre available in Phase 2 UI)
// -----------------------------------------------------------------------------
pub struct PlanetPlugin;
impl Plugin for PlanetPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlanetParams>()
            .init_resource::<PlanetSettings>()
            .init_resource::<PlanetDebugConfig>();
    }
}

// -----------------------------------------------------------------------------
// Entry points
// -----------------------------------------------------------------------------

/// Existing call-site wrapper (keeps your Dev Panel working as-is).
pub fn spawn_random_planet_inner(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    planet_materials: &mut Assets<PlanetSurfaceMaterial>,
    atmosphere_materials: &mut Assets<AtmosphereMaterial>,
    standard_materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    sampler_res: &FlatSamplerRes,
    map: &MapSettings,
    params: &PlanetParams,
    settings: &PlanetSettings,
    debug: &PlanetDebugConfig,
    sun_direction: &SunDirection,
    surface_model: &mut PlanetSurfaceModel,
) {
    log_planet_configuration(
        "spawn_random_planet_inner",
        map,
        params,
        settings,
        sampler_res,
    );

    spawn_planet_with_settings(
        commands,
        meshes,
        planet_materials,
        atmosphere_materials,
        standard_materials,
        images,
        sampler_res,
        map,
        params,
        settings,
        sun_direction,
        debug,
        surface_model,
    );
}

/// Optional system if you ever want to spawn using live settings directly.
#[allow(dead_code)]
pub fn spawn_random_planet_with_settings_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut planet_materials: ResMut<Assets<PlanetSurfaceMaterial>>,
    mut atmosphere_materials: ResMut<Assets<AtmosphereMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    sampler_res: Res<FlatSamplerRes>,
    map: Res<MapSettings>,
    params: Res<PlanetParams>,
    settings: Res<PlanetSettings>,
    debug: Res<PlanetDebugConfig>,
    sun_direction: Res<SunDirection>,
    mut surface_model: ResMut<PlanetSurfaceModel>,
) {
    log_planet_configuration(
        "spawn_random_planet_with_settings_system",
        &map,
        &params,
        &settings,
        &sampler_res,
    );
    surface_model.radius = params.radius;
    surface_model.biome_resolution = DEFAULT_BIOME_MAP_RESOLUTION;

    spawn_planet_with_settings(
        &mut commands,
        &mut meshes,
        &mut *planet_materials,
        &mut *atmosphere_materials,
        &mut *standard_materials,
        &mut *images,
        &sampler_res,
        &map,
        &params,
        &settings,
        &*sun_direction,
        &debug,
        &mut *surface_model,
    );
}

fn spawn_planet_with_settings(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    planet_materials: &mut Assets<PlanetSurfaceMaterial>,
    atmosphere_materials: &mut Assets<AtmosphereMaterial>,
    standard_materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
    sampler_res: &FlatSamplerRes,
    _map: &MapSettings,
    params: &PlanetParams,
    settings: &PlanetSettings,
    sun_direction: &SunDirection,
    debug: &PlanetDebugConfig,
    surface_model: &mut PlanetSurfaceModel,
) {
    let climate_summary = analyze_planet_climate(sampler_res.0.seed, params, settings);
    info!(
        "climate summary: water={:.2} deep={:.2} coast={:.2} snow={:.2} temp={:.2} moisture={:.2} dryness={:.2}",
        climate_summary.water_fraction,
        climate_summary.deep_water_fraction,
        climate_summary.coastline_fraction,
        climate_summary.snow_land_fraction,
        climate_summary.avg_land_temperature,
        climate_summary.avg_land_moisture,
        climate_summary.avg_land_dryness,
    );

    let classifier = BiomeClassifier::default();
    let climate_for_bake = ClimateModel::new(sampler_res.0.seed, params, settings);
    let biome_map =
        bake_global_biome_map(&climate_for_bake, &classifier, DEFAULT_BIOME_MAP_RESOLUTION);
    info!(
        "baked global biome map at {}^2 texels per cube face",
        biome_map.resolution
    );
    let surface_grid =
        bake_surface_grid(&climate_for_bake, &classifier, SURFACE_GRID_RESOLUTION);
    if surface_grid.resolution > 0 {
        info!(
            "baked surface grid at {}^2 samples per cube face",
            surface_grid.resolution
        );
    } else {
        info!("surface grid resolution is zero; mesh will fall back to procedural sampling");
    }
    update_surface_model_biome_faces(surface_model, &biome_map, images);

    let land_mesh = build_colored_planet_mesh(
        settings,
        sampler_res,
        params,
        debug.mode,
        Some(&biome_map),
        surface_model,
        &surface_grid,
    );
    if let Some(VertexAttributeValues::Float32x3(positions)) =
        land_mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    {
        info!("planet vertex count: {}", positions.len());
    }
    let handle = meshes.add(land_mesh);
    commands.insert_resource(surface_grid);
    commands.insert_resource(biome_map);

    let extension = PlanetSurfaceParams::from_settings(
        sampler_res.0.seed,
        params,
        settings,
        sun_direction.0,
        debug.mode,
    );
    let base_material = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: extension.params.perceptual_roughness,
        metallic: extension.params.metallic,
        reflectance: extension.params.reflectance,
        ..default()
    };
    let planet_material = planet_materials.add(PlanetSurfaceMaterial {
        base: base_material,
        extension,
    });

    let rotation = Quat::from_rotation_y(params.rotation_deg.to_radians());
    let planet_entity = commands
        .spawn((
            PlanetTag,
            PlanetLod { level: 5 },
            Mesh3d(handle),
            MeshMaterial3d(planet_material.clone()),
            Transform::from_rotation(rotation),
            GlobalTransform::default(),
            Visibility::default(),
            InheritedVisibility::default(),
            Name::new("Planet"),
        ))
        .id();

    let atmosphere_mesh = meshes.add(Sphere::new(1.0));
    let atmosphere_material = atmosphere_materials.add(AtmosphereMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(0.0, 0.0, 0.0, 0.0),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            double_sided: true,
            ..default()
        },
        extension: AtmosphereParams::new(LinearRgba::from(Color::srgb(0.18, 0.46, 0.94)), 0.6, 4.8),
    });
    commands.entity(planet_entity).with_children(|parent| {
        parent.spawn((
            PlanetAtmosphere,
            Mesh3d(atmosphere_mesh),
            MeshMaterial3d(atmosphere_material),
            Transform::from_scale(Vec3::splat(params.radius * ATMOSPHERE_SCALE_FACTOR)),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            Name::new("PlanetAtmosphere"),
        ));

        let cloud_mesh = meshes.add(Sphere::new(1.0));
        let cloud_material = standard_materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, CLOUD_ALPHA),
            emissive: Color::srgba(0.9, 0.95, 1.0, CLOUD_ALPHA * 0.6).into(),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            ..default()
        });
        parent.spawn((
            PlanetClouds,
            Mesh3d(cloud_mesh),
            MeshMaterial3d(cloud_material),
            Transform::from_scale(Vec3::splat(params.radius * CLOUD_SCALE_FACTOR)),
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
            Name::new("PlanetClouds"),
        ));
    });
}

fn update_surface_model_biome_faces(
    surface_model: &mut PlanetSurfaceModel,
    biome_map: &GlobalBiomeMap,
    images: &mut Assets<Image>,
) {
    let extent = Extent3d {
        width: biome_map.resolution.max(1),
        height: biome_map.resolution.max(1),
        depth_or_array_layers: 1,
    };
    for (index, face_data) in biome_map.faces.iter().enumerate() {
        let mut image = Image::new_fill(
            extent,
            TextureDimension::D2,
            face_data.as_slice(),
            TextureFormat::R8Uint,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::nearest();

        let handle = &mut surface_model.biome_faces[index];
        if handle.is_strong() {
            let _ = images.insert(handle.id(), image);
        } else {
            *handle = images.add(image);
        }
    }
    surface_model.biome_resolution = biome_map.resolution;
}

pub fn auto_clip_planes(
    mut q_cam: Query<(&GlobalTransform, &mut Projection), With<Camera3d>>,
    q_planet: Query<&GlobalTransform, With<PlanetTag>>,
    params: Res<PlanetParams>,
) {
    let Ok((cam_tf, mut proj)) = q_cam.single_mut() else {
        return;
    };
    let Ok(planet_tf) = q_planet.single() else {
        return;
    };

    let center = planet_tf.translation();
    let dist = cam_tf.translation().distance(center);
    let alt = (dist - params.radius).max(0.1);

    // Near plane small near surface, larger in orbit (prevents z-fighting)
    let near = (alt * 0.002).clamp(0.01, 5.0);
    // Far plane scales with planet size and altitude
    let far = (params.radius * 6.0).max(alt * 12.0);

    if let Projection::Perspective(p) = &mut *proj {
        p.near = near;
        p.far = far;
    }
}

pub fn log_planet_configuration(
    context: &str,
    map: &MapSettings,
    params: &PlanetParams,
    settings: &PlanetSettings,
    sampler: &FlatSamplerRes,
) {
    info!(
        "planet config [{}]: map_seed={}, sampler_seed={}, water_level={:.3}, radius={:.3}, height_amp={:.3}, base_freq={:.4}, detail_freq={:.4}, warp_freq={:.4}, warp_amp={:.4}, mountains={:.3}, mountain_spikiness={:.3}, rotation_deg={:.1}",
        context,
        map.seed,
        sampler.0.seed,
        map.water_level,
        params.radius,
        params.height_amp,
        settings.base_freq,
        settings.detail_freq,
        settings.warp_freq,
        settings.warp_amp,
        settings.mountain_strength,
        settings.mountain_spikiness,
        params.rotation_deg
    );
}

pub fn spin_planet_clouds(time: Res<Time>, mut clouds: Query<&mut Transform, With<PlanetClouds>>) {
    let delta = time.delta_secs() * CLOUD_ROTATION_SPEED;
    if delta.abs() < f32::EPSILON {
        return;
    }
    for mut transform in &mut clouds {
        transform.rotate_y(delta);
    }
}

pub fn toggle_planet_wireframe(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    planets: Query<(Entity, Option<&Wireframe>), With<PlanetTag>>,
) {
    if !keys.just_pressed(KeyCode::KeyV) {
        return;
    }

    for (entity, has_wireframe) in planets.iter() {
        let mut ec = commands.entity(entity);
        if has_wireframe.is_some() {
            ec.remove::<Wireframe>();
        } else {
            ec.insert(Wireframe);
        }
    }
}

pub fn sync_planet_material_uniforms(
    debug: Res<PlanetDebugConfig>,
    sun_direction: Res<SunDirection>,
    mut materials: ResMut<Assets<PlanetSurfaceMaterial>>,
    q_planet: Query<&MeshMaterial3d<PlanetSurfaceMaterial>, With<PlanetTag>>,
) {
    if !debug.is_changed() && !sun_direction.is_changed() {
        return;
    }
    for material_handle in &q_planet {
        if let Some(material) = materials.get_mut(&material_handle.0) {
            if debug.is_changed() {
                material.extension.params.debug_mode = debug.mode as u32;
            }
            if sun_direction.is_changed() {
                material.extension.params.sun_dir = sun_direction.as_vec4();
            }
        }
    }
}

pub fn update_planet_lod(
    mut meshes: ResMut<Assets<Mesh>>,
    mut q_planet: Query<(&mut PlanetLod, &Mesh3d, &GlobalTransform), With<PlanetTag>>,
    q_cam: Query<&GlobalTransform, (With<Camera3d>, Without<PlanetTag>)>,
    params: Res<PlanetParams>,
    sampler_res: Res<FlatSamplerRes>,
    _map: Res<MapSettings>,        // ok to keep; unused is fine for now
    settings: Res<PlanetSettings>, // <-- add this
    biome_map: Option<Res<GlobalBiomeMap>>,
    surface_model: Res<PlanetSurfaceModel>,
    surface_grid: Res<SurfaceGrid>,
    lod_bands: Res<LodDebugBands>,
    debug: Res<PlanetDebugConfig>,
) {
    // single() single() in 0.18
    let Ok(cam_tf) = q_cam.single() else {
        return;
    };

    let biome_map_ref = biome_map.as_ref().map(|map| &**map);

    for (mut lod, mesh_h, planet_tf) in &mut q_planet {
        let center = planet_tf.translation();
        let dist = cam_tf.translation().distance(center).max(1.0);

        let radius = params.radius.max(1.0);
        let altitude = (dist - radius).max(0.0);
        let _altitude_ratio = altitude / radius;

        // Near surface we want maximum tessellation, progressively dropping detail
        // as the camera pulls away. These thresholds are aligned with the debug gizmo rings.
        let mut ring_altitudes: Vec<f32> = lod_bands
            .radii
            .iter()
            .copied()
            .filter(|scale| *scale > 1.0 + f32::EPSILON)
            .map(|scale| (scale - 1.0) * radius)
            .collect();
        if ring_altitudes.len() < 3 {
            const FALLBACK_FRAC: [f32; 3] = [0.01, 0.05, 0.2];
            ring_altitudes.extend(FALLBACK_FRAC.iter().map(|frac| radius * frac));
        }
        ring_altitudes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
        ring_altitudes.dedup_by(|a, b| (*a - *b).abs() < 1.0);
        if ring_altitudes.len() < 3 {
            continue;
        }
        let start = ring_altitudes.len() - 3;
        let ring_altitudes = [
            ring_altitudes[start],
            ring_altitudes[start + 1],
            ring_altitudes[start + 2],
        ];

        let hysteresis = |idx: usize| -> f32 {
            let band = ring_altitudes[idx];
            let mut value = band * 0.25;
            value = value.max(params.height_amp * 0.08);
            value = value.max(25.0);
            value
        };

        if lod.level > 5 {
            lod.level = 5;
        }
        let mut target = lod.level;

        match lod.level {
            5 => {
                if altitude > ring_altitudes[0] + hysteresis(0) {
                    target = 4;
                }
            }
            4 => {
                if altitude < (ring_altitudes[0] - hysteresis(0)).max(0.0) {
                    target = 5;
                } else if altitude > ring_altitudes[1] + hysteresis(1) {
                    target = 3;
                }
            }
            3 => {
                if altitude < (ring_altitudes[1] - hysteresis(1)).max(0.0) {
                    target = 4;
                } else if altitude > ring_altitudes[2] + hysteresis(2) {
                    target = 2;
                }
            }
            2 => {
                if altitude < (ring_altitudes[2] - hysteresis(2)).max(0.0) {
                    target = 3;
                }
            }
            _ => {}
        }

        if target != lod.level {
            info!(
                "Planet LOD switched to level {} (alt {:.1}, radius {:.1})",
                target, altitude, radius
            );
            lod.level = target;

            if let Some(mesh) = meshes.get_mut(&mesh_h.0) {
                let new = build_colored_planet_mesh_with_subdiv(
                    lod.level,
                    &sampler_res,
                    &params,
                    &settings,
                    debug.mode,
                    biome_map_ref,
                    &surface_model,
                    &surface_grid,
                );
                if let Some(positions) = new.attribute(Mesh::ATTRIBUTE_POSITION) {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
                }
                if let Some(normals) = new.attribute(Mesh::ATTRIBUTE_NORMAL) {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals.clone());
                }
                if let Some(colors) = new.attribute(Mesh::ATTRIBUTE_COLOR) {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors.clone());
                }
                if let Some(uvs) = new.attribute(Mesh::ATTRIBUTE_UV_0) {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs.clone());
                }
                if let Some(uvs1) = new.attribute(Mesh::ATTRIBUTE_UV_1) {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uvs1.clone());
                }
                if let Some(tangents) = new.attribute(Mesh::ATTRIBUTE_TANGENT) {
                    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangents.clone());
                }
                if let Some(idx) = new.indices().cloned() {
                    mesh.insert_indices(idx);
                }
            }
        }
    }
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn pack_pair(a: f32, b: f32) -> f32 {
    let encode = |value: f32| -> u32 { (value.clamp(0.0, 1.0) * 65535.0).round() as u32 };
    let high = encode(a) << 16;
    let low = encode(b) & 0xFFFF;
    f32::from_bits(high | low)
}

// -----------------------------------------------------------------------------
// Mesh generation
// -----------------------------------------------------------------------------
fn build_colored_planet_mesh(
    settings: &PlanetSettings,
    sampler_res: &FlatSamplerRes,
    params: &PlanetParams,
    debug_mode: PlanetDebugMode,
    biome_map: Option<&GlobalBiomeMap>,
    surface_model: &PlanetSurfaceModel,
    surface_grid: &SurfaceGrid,
) -> Mesh {
    build_colored_planet_mesh_with_subdiv(
        7,
        sampler_res,
        params,
        settings,
        debug_mode,
        biome_map,
        surface_model,
        surface_grid,
    )
}

fn build_colored_planet_mesh_with_subdiv(
    subdiv: u32,
    sampler_res: &FlatSamplerRes,
    params: &PlanetParams,
    settings: &PlanetSettings,
    _debug_mode: PlanetDebugMode,
    biome_map: Option<&GlobalBiomeMap>,
    surface_model: &PlanetSurfaceModel,
    surface_grid: &SurfaceGrid,
) -> Mesh {
    let (mut verts, indices_u32) = generate_icosphere(subdiv);
    let climate = ClimateModel::new(sampler_res.0.seed, params, settings);
    let grid_resolution = surface_grid.resolution;
    let mut packed_attributes: Vec<[f32; 4]> = Vec::with_capacity(verts.len());
    let mut blended_normals: Vec<Vec3> = Vec::with_capacity(verts.len());
    let mut biome_tags: Vec<[f32; 2]> = Vec::with_capacity(verts.len());
    let mut climate_channels: Vec<[f32; 2]> = Vec::with_capacity(verts.len());
    let classifier = BiomeClassifier::default();

    for v in &mut verts {
        let dir = {
            let normalized = v.normalize_or_zero();
            if normalized.length_squared() <= f32::EPSILON {
                Vec3::Y
            } else {
                normalized
            }
        };
        let eval = climate.evaluate(dir);
        let (face, uv) = dir_to_face_uv(dir);
        let height = if grid_resolution > 0 {
            surface_grid.sample_height_uv(face, uv)
        } else {
            eval.mountain_offset
        };
        let world_pos = dir * (surface_model.radius + height);
        *v = world_pos;

        let climate_sample = PlanetClimateSample::from_eval(&eval);
        climate_channels.push(climate_sample.uv1);
        packed_attributes.push(climate_sample.packed);
        let biome_u8 = if grid_resolution > 0 {
            surface_grid.sample_biome_uv(face, uv)
        } else {
            let coord = SurfaceCoord { face, uv, height };
            biome_map
                .map(|map| sample_biome_face_uv(map, coord))
                .unwrap_or_else(|| classifier.classify_sample(&climate_sample) as u8)
        };
        biome_tags.push([biome_u8 as f32 / 255.0, eval.elev01]);

        let mountain_weight = eval.mountain_mask;
        if mountain_weight > 1e-3 {
            let grad = eval.gradient * mountain_weight;
            let mut detail_normal = Vec3::new(-grad.x, 0.6, -grad.z);
            if detail_normal.length_squared() < 1e-6 {
                detail_normal = dir;
            } else {
                detail_normal = detail_normal.normalize();
            }
            let blended = (detail_normal * mountain_weight + dir * (1.0 - mountain_weight))
                .normalize_or_zero();
            blended_normals.push(blended);
        } else {
            blended_normals.push(dir);
        }
    }

    let smooth_normals = compute_smooth_normals(&verts, &indices_u32);
    let normals: Vec<[f32; 3]> = smooth_normals
        .into_iter()
        .zip(blended_normals.into_iter())
        .map(|(smooth, blended)| {
            let smooth_v = Vec3::from_array(smooth);
            let n = (smooth_v * 0.55 + blended * 0.45).normalize_or_zero();
            n.to_array()
        })
        .collect();

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, verts);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, biome_tags);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, climate_channels);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, packed_attributes);
    mesh.insert_indices(Indices::U32(indices_u32));
    mesh
}
#[derive(Component, Clone, Copy)]
pub struct PlanetLod {
    /// Orbit-view subdivision level; future streaming tiers follow the notes in `Lod_context.txt`.
    pub level: u32, // 0..7 is sane; 6 is already heavy
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
