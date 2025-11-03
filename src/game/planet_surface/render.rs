//! Patch entity manager placeholder.
//!
//! The render layer will consume `stream::PatchAssetReady` events and spawn /
//! recycle entities with the appropriate meshes.  For now, we only provide type
//! definitions and no-op systems so other code can depend on them.

use std::time::Instant;
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
};

use bevy::math::primitives::{Cylinder, Sphere};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::prelude::*;
use bevy_mesh::Mesh;

use super::{
    lod::PatchKey,
    stream::{PatchPropPlaceholder, PatchRequestQueue},
};
use crate::game::world::planet::{PlanetSettings, PlanetSurfaceMaterial};

#[derive(Component)]
pub struct SurfacePatch {
    pub key: PatchKey,
}

#[derive(Resource, Default)]
pub struct PatchStats {
    pub loaded: usize,
    pub requested: usize,
}

#[derive(Resource)]
pub struct PatchCacheMetrics {
    pub disk_hits: u64,
    pub disk_misses: u64,
    pub procedural_builds: u64,
    pub procedural_failures: u64,
    pub evictions: u64,
    pub active_patches: usize,
    pub max_active_budget: usize,
    pub spawn_budget_last: u32,
    pub morph_time_last_ms: f32,
    pub morph_time_avg_ms: f32,
    pub active_history: VecDeque<usize>,
    pub spawn_history: VecDeque<u32>,
    pub history_capacity: usize,
    pub last_morph_log: Option<Instant>,
}

impl Default for PatchCacheMetrics {
    fn default() -> Self {
        Self {
            disk_hits: 0,
            disk_misses: 0,
            procedural_builds: 0,
            procedural_failures: 0,
            evictions: 0,
            active_patches: 0,
            max_active_budget: 0,
            spawn_budget_last: 0,
            morph_time_last_ms: 0.0,
            morph_time_avg_ms: 0.0,
            active_history: VecDeque::new(),
            spawn_history: VecDeque::new(),
            history_capacity: 240,
            last_morph_log: None,
        }
    }
}

#[derive(Clone)]
pub struct PatchGeometry {
    pub sphere_positions: Vec<Vec3>,
    pub displaced_positions: Vec<Vec3>,
    pub sphere_normals: Vec<Vec3>,
    pub displaced_normals: Vec<Vec3>,
}

#[derive(Clone)]
pub struct PatchCacheEntry {
    pub mesh: Handle<Mesh>,
    pub entity: Option<Entity>,
    pub active_children: u8,
    pub geometry: Option<PatchGeometry>,
    pub current_morph: f32,
    pub materials: Vec<String>,
    pub props: Vec<PatchPropPlaceholder>,
    pub resolved_material: Option<Handle<PlanetSurfaceMaterial>>,
    pub albedo_texture: Option<Handle<Image>>,
    pub normal_texture: Option<Handle<Image>>,
    pub roughness_texture: Option<Handle<Image>>,
}

#[derive(Resource, Default)]
pub struct PatchRegistry {
    pub entries: HashMap<PatchKey, PatchCacheEntry>,
}

#[derive(Resource, Default)]
pub struct PatchMaterialLibrary {
    pub base: Option<Handle<PlanetSurfaceMaterial>>,
    pub variants: HashMap<Vec<String>, Handle<PlanetSurfaceMaterial>>,
}

#[derive(Resource, Default)]
pub struct PropGizmoAssets {
    pub sphere: Option<Handle<Mesh>>,
    pub cylinder: Option<Handle<Mesh>>,
    pub materials: HashMap<String, Handle<StandardMaterial>>,
}

#[derive(Component, Clone, Debug)]
pub struct PatchMaterials {
    pub identifiers: Vec<String>,
}

#[derive(Component, Clone, Debug)]
pub struct PatchPropInstance {
    pub kind: String,
}

pub fn update_patch_stats(
    mut stats: ResMut<PatchStats>,
    patches: Query<&SurfacePatch>,
    queue: Res<PatchRequestQueue>,
) {
    stats.loaded = patches.iter().len();
    stats.requested = queue.len();
}

impl PatchMaterialLibrary {
    pub fn resolve(
        &mut self,
        identifiers: &[String],
        base_handle: &Handle<PlanetSurfaceMaterial>,
        materials: &mut Assets<PlanetSurfaceMaterial>,
        settings: &PlanetSettings,
    ) -> Handle<PlanetSurfaceMaterial> {
        if identifiers.is_empty() {
            return base_handle.clone();
        }

        if let Some(handle) = self.variants.get(identifiers) {
            return handle.clone();
        }

        let Some(base_material) = materials.get(base_handle).cloned() else {
            return base_handle.clone();
        };

        let mut variant = base_material.clone();
        let mut changed = false;
        for identifier in identifiers {
            changed |= apply_material_identifier(&mut variant, identifier.as_str(), settings);
        }

        if !changed {
            return base_handle.clone();
        }

        let handle = materials.add(variant);
        self.variants.insert(identifiers.to_vec(), handle.clone());
        handle
    }

    pub fn update_morph(&self, materials: &mut Assets<PlanetSurfaceMaterial>, morph: f32) {
        if let Some(base) = self.base.as_ref() {
            if let Some(material) = materials.get_mut(base) {
                material.extension.params.surface_morph = morph;
            }
        }
        for handle in self.variants.values() {
            if let Some(material) = materials.get_mut(handle) {
                material.extension.params.surface_morph = morph;
            }
        }
    }
}

fn apply_material_identifier(
    material: &mut PlanetSurfaceMaterial,
    identifier: &str,
    settings: &PlanetSettings,
) -> bool {
    if let Some(palette) = identifier.strip_prefix("palette:") {
        return apply_palette_variant(material, palette, settings);
    }
    if let Some(value) = identifier.strip_prefix("detail:amp=") {
        if let Ok(parsed) = value.parse::<f32>() {
            material.extension.params.surface_detail_amp = parsed.max(0.0);
            return true;
        }
    }
    if let Some(value) = identifier.strip_prefix("detail:scale=") {
        if let Ok(parsed) = value.parse::<f32>() {
            material.extension.params.surface_detail_scale = parsed.max(0.01);
            return true;
        }
    }
    false
}

fn apply_palette_variant(
    material: &mut PlanetSurfaceMaterial,
    palette: &str,
    settings: &PlanetSettings,
) -> bool {
    let mut colors = PaletteColors::from_settings(settings);
    match palette {
        "default" | "" => {
            set_palette_from_colors(material, &colors);
            material.extension.params.surface_detail_amp = 0.06;
            material.extension.params.surface_detail_scale = 14.0;
            true
        }
        "debug" => {
            colors.water_deep = Vec3::new(0.1, 0.4, 0.95);
            colors.water_shallow = Vec3::new(0.4, 0.75, 1.0);
            colors.land_sand = Vec3::new(0.95, 0.4, 0.25);
            colors.land_grass = Vec3::new(0.25, 0.95, 0.35);
            colors.land_rock = Vec3::new(0.4, 0.25, 0.95);
            colors.land_snow = Vec3::new(0.95, 0.95, 0.95);
            set_palette_from_colors(material, &colors);
            material.extension.params.surface_detail_amp = 0.09;
            material.extension.params.surface_detail_scale = 16.0;
            true
        }
        "lush" => {
            colors.water_deep = colors.water_deep.lerp(Vec3::new(0.08, 0.18, 0.36), 0.35);
            colors.water_shallow = colors.water_shallow.lerp(Vec3::new(0.26, 0.58, 0.82), 0.4);
            colors.land_grass =
                (colors.land_grass * 1.12).clamp(Vec3::splat(0.0), Vec3::splat(1.0));
            colors.land_sand = colors.land_sand.lerp(Vec3::new(0.88, 0.78, 0.55), 0.25);
            set_palette_from_colors(material, &colors);
            material.extension.params.surface_detail_amp = 0.08;
            material.extension.params.surface_detail_scale = 18.0;
            true
        }
        "arid" | "desert" => {
            colors.water_shallow = colors.water_shallow.lerp(Vec3::new(0.38, 0.62, 0.64), 0.25);
            colors.land_sand = (colors.land_sand * 1.08).min(Vec3::splat(1.0));
            colors.land_grass = colors.land_grass.lerp(colors.land_sand, 0.65);
            colors.land_rock = colors.land_rock.lerp(Vec3::new(0.56, 0.46, 0.38), 0.4);
            colors.land_snow = colors.land_snow.lerp(Vec3::new(0.92, 0.92, 0.92), 0.5);
            set_palette_from_colors(material, &colors);
            material.extension.params.surface_detail_amp = 0.04;
            material.extension.params.surface_detail_scale = 10.0;
            true
        }
        "ice" | "polar" => {
            colors.water_deep = colors.water_deep.lerp(Vec3::new(0.12, 0.24, 0.46), 0.5);
            colors.water_shallow = colors.water_shallow.lerp(Vec3::new(0.54, 0.74, 0.9), 0.45);
            colors.land_grass = colors.land_grass.lerp(Vec3::new(0.68, 0.74, 0.82), 0.7);
            colors.land_sand = colors.land_sand.lerp(Vec3::new(0.82, 0.86, 0.9), 0.6);
            colors.land_rock = colors.land_rock.lerp(Vec3::new(0.72, 0.76, 0.8), 0.55);
            colors.land_snow = colors.land_snow.lerp(Vec3::new(0.96, 0.98, 1.0), 0.5);
            set_palette_from_colors(material, &colors);
            material.extension.params.surface_detail_amp = 0.03;
            material.extension.params.surface_detail_scale = 12.0;
            true
        }
        "volcanic" => {
            colors.water_deep = colors.water_deep.lerp(Vec3::new(0.1, 0.05, 0.15), 0.6);
            colors.water_shallow = colors.water_shallow.lerp(Vec3::new(0.28, 0.18, 0.22), 0.55);
            colors.land_sand = colors.land_sand.lerp(Vec3::new(0.48, 0.27, 0.18), 0.7);
            colors.land_grass = colors.land_grass.lerp(Vec3::new(0.18, 0.28, 0.18), 0.75);
            colors.land_rock = colors.land_rock.lerp(Vec3::new(0.32, 0.18, 0.12), 0.65);
            colors.land_snow = colors.land_snow.lerp(Vec3::new(0.65, 0.68, 0.72), 0.6);
            set_palette_from_colors(material, &colors);
            material.extension.params.surface_detail_amp = 0.07;
            material.extension.params.surface_detail_scale = 11.0;
            true
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
struct PaletteColors {
    water_deep: Vec3,
    water_shallow: Vec3,
    land_sand: Vec3,
    land_grass: Vec3,
    land_rock: Vec3,
    land_snow: Vec3,
}

impl PaletteColors {
    fn from_settings(settings: &PlanetSettings) -> Self {
        Self {
            water_deep: settings.water_deep,
            water_shallow: settings.water_shallow,
            land_sand: settings.land_sand,
            land_grass: settings.land_grass,
            land_rock: settings.land_rock,
            land_snow: settings.land_snow,
        }
    }
}

fn set_palette_from_colors(material: &mut PlanetSurfaceMaterial, palette: &PaletteColors) {
    material.extension.params.water_deep = srgb_vec_to_linear(palette.water_deep);
    material.extension.params.water_shallow = srgb_vec_to_linear(palette.water_shallow);
    material.extension.params.land_sand = srgb_vec_to_linear(palette.land_sand);
    material.extension.params.land_grass = srgb_vec_to_linear(palette.land_grass);
    material.extension.params.land_rock = srgb_vec_to_linear(palette.land_rock);
    material.extension.params.land_snow = srgb_vec_to_linear(palette.land_snow);
}

fn srgb_vec_to_linear(color: Vec3) -> Vec4 {
    Vec4::new(
        srgb_channel_to_linear(color.x),
        srgb_channel_to_linear(color.y),
        srgb_channel_to_linear(color.z),
        1.0,
    )
}

fn srgb_channel_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

pub fn update_material_palette_base(
    library: &mut PatchMaterialLibrary,
    new_base: Handle<PlanetSurfaceMaterial>,
) -> bool {
    if library.base.as_ref() != Some(&new_base) {
        library.base = Some(new_base);
        library.variants.clear();
        return true;
    }
    false
}

pub fn attach_prop_gizmos(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut assets: ResMut<PropGizmoAssets>,
    query: Query<(Entity, &PatchPropInstance), Added<PatchPropInstance>>,
) {
    if query.is_empty() {
        return;
    }

    if assets.sphere.is_none() {
        assets.sphere = Some(meshes.add(Mesh::from(Sphere::new(0.5))));
    }
    if assets.cylinder.is_none() {
        assets.cylinder = Some(meshes.add(Mesh::from(Cylinder::new(0.25, 1.0))));
    }

    for (entity, prop) in &query {
        let (key, use_cylinder, color) = gizmo_descriptor(&prop.kind);
        let mesh_handle = if use_cylinder {
            assets.cylinder.clone().unwrap()
        } else {
            assets.sphere.clone().unwrap()
        };

        let material_handle = assets
            .materials
            .entry(key.into_owned())
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: color,
                    alpha_mode: AlphaMode::Opaque,
                    unlit: true,
                    ..default()
                })
            })
            .clone();

        commands
            .entity(entity)
            .insert((Mesh3d(mesh_handle), MeshMaterial3d(material_handle)));
    }
}

fn gizmo_descriptor(kind: &str) -> (Cow<'_, str>, bool, Color) {
    let lower = kind.to_ascii_lowercase();
    if lower.contains("tree") || lower.contains("forest") || lower.contains("shrub") {
        (
            Cow::Borrowed("cat:foliage"),
            false,
            Color::srgb(0.227, 0.541, 0.333),
        )
    } else if lower.contains("base")
        || lower.contains("structure")
        || lower.contains("tower")
        || lower.contains("building")
        || lower.contains("antenna")
    {
        (
            Cow::Borrowed("cat:structure"),
            true,
            Color::srgb(0.824, 0.643, 0.337),
        )
    } else {
        let hash = hash_kind(kind.as_bytes());
        let r = ((hash & 0xFF) as f32) / 255.0;
        let g = (((hash >> 8) & 0xFF) as f32) / 255.0;
        let b = (((hash >> 16) & 0xFF) as f32) / 255.0;
        (
            Cow::Owned(kind.to_string()),
            false,
            Color::srgb(r.max(0.2), g.max(0.2), b.max(0.2)),
        )
    }
}

fn hash_kind(bytes: &[u8]) -> u32 {
    let mut hash = 2166136261u32;
    for &byte in bytes {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}
