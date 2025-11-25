use bevy::ecs::system::SystemParam;
use bevy::{
    asset::{LoadState, RenderAssetUsages},
    log::{debug, warn},
    pbr::MeshMaterial3d,
    prelude::*,
    render::render_resource::{PrimitiveTopology, TextureFormat},
    tasks::AsyncComputeTaskPool,
};
use bevy_mesh::{Indices, Mesh};
use futures_lite::future;
use std::cmp::Ordering;
use std::time::{Duration, Instant};

use super::{
    asset_loader::{self, PatchAssetState},
    lod::{self, PatchDescriptor, PatchKey},
    manager::{PlanetContext, PlanetContextLayer},
    render::{
        PatchCacheEntry, PatchCacheMetrics, PatchGeometry, PatchMaterialLibrary, PatchMaterials,
        PatchPropInstance, PatchRegistry, SurfacePatch,
    },
    stream::{
        GeneratedPatch, PatchLoadMethod, PatchLoadTask, PatchLoadTasks, PatchPropPlaceholder,
        PatchRequestQueue,
    },
};
use crate::app::AppState;
use crate::game::world::{
    planet::{
        mark_skirt_vertex, ClimateModel, PlanetEntity, PlanetParams, PlanetSettings,
        PlanetSurfaceMaterial, PlanetTag,
    },
    terrain::MapSettings,
};

const BASE_VERTS_PER_SIDE: usize = 10;
const PATCHES_PER_FRAME_SURFACE_NEAR: f32 = 18.0;
const PATCHES_PER_FRAME_SURFACE_FAR: f32 = 9.0;
const PATCHES_PER_FRAME_APPROACH_NEAR: f32 = 12.0;
const PATCHES_PER_FRAME_APPROACH_FAR: f32 = 6.0;
const MAX_ACTIVE_APPROACH_NEAR: usize = 420;
const MAX_ACTIVE_APPROACH_FAR: usize = 220;
const MAX_ACTIVE_SURFACE_NEAR: usize = 900;
const MAX_ACTIVE_SURFACE_FAR: usize = 450;
const MAX_CONCURRENT_TASKS: usize = 28;
const LEVEL_PRIORITY_WEIGHT: f32 = 0.08;
const MIN_EVICT_RAD_MULT: f32 = 2.4;
const EVICT_DISTANCE_RATIO: f32 = 1.25;
const EVICT_PRIORITY_MARGIN: f32 = 0.12;
const SURFACE_ALT_CLAMP_KM: f32 = 20.0;
const APPROACH_ALT_CLAMP_KM: f32 = 180.0;
const PATCH_RADIUS_REFERENCE: f32 = 500_000.0;

#[derive(Resource, Default)]
pub struct ClimateProfiler {
    frame_samples: u64,
    frame_time: Duration,
    pub last_samples: u64,
    pub last_time: Duration,
    pub max_samples: u64,
    pub max_time: Duration,
}

impl ClimateProfiler {
    pub fn record(&mut self, samples: u64, time: Duration) {
        self.frame_samples = self.frame_samples.saturating_add(samples);
        self.frame_time += time;
    }

    pub fn finish_frame(&mut self) {
        if self.frame_samples == 0 && self.frame_time.is_zero() {
            self.last_samples = 0;
            self.last_time = Duration::ZERO;
        } else {
            self.last_samples = self.frame_samples;
            self.last_time = self.frame_time;
            if self.last_samples > self.max_samples {
                self.max_samples = self.last_samples;
            }
            if self.last_time > self.max_time {
                self.max_time = self.last_time;
            }
        }
        self.frame_samples = 0;
        self.frame_time = Duration::ZERO;
    }
}

pub fn climate_profiler_finish_frame(mut profiler: ResMut<ClimateProfiler>) {
    profiler.finish_frame();
}

fn patch_priority(key: PatchKey, radius: f32, planet_center: Vec3, camera_pos: Vec3) -> f32 {
    let descriptor = PatchDescriptor::from_key(key, radius);
    let center_world = planet_center + descriptor.center;
    let dist_sq = camera_pos.distance_squared(center_world);
    let level_bias = (key.level as f32) * LEVEL_PRIORITY_WEIGHT * radius * radius;
    dist_sq - level_bias
}

fn select_eviction_candidate(
    registry: &PatchRegistry,
    radius: f32,
    context: &PlanetContext,
) -> Option<(PatchKey, f32, Entity)> {
    let mut candidates: Vec<(PatchKey, f32, Entity)> = registry
        .entries
        .iter()
        .filter_map(|(&key, entry)| {
            let entity = entry.entity?;
            if entry.active_children > 0 {
                return None;
            }
            let descriptor = PatchDescriptor::from_key(key, radius);
            let center_world = context.planet_center + descriptor.center;
            let dist = center_world.distance(context.camera_pos);
            let guard_distance = (descriptor.radius * MIN_EVICT_RAD_MULT).max(radius * 0.01);
            if dist < guard_distance {
                return None;
            }
            let priority = patch_priority(key, radius, context.planet_center, context.camera_pos);
            Some((key, priority, entity))
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));
    candidates.into_iter().next()
}

/// Consume queued patch requests, drive background mesh generation, and spawn
/// patch entities once assets are ready.
#[derive(SystemParam)]
pub struct ProcessPatchQueueParams<'w, 's> {
    pub state: Res<'w, State<AppState>>,
    pub queue: ResMut<'w, PatchRequestQueue>,
    pub load_tasks: ResMut<'w, PatchLoadTasks>,
    pub current_planet: Res<'w, PlanetEntity>,
    pub params: Res<'w, PlanetParams>,
    pub settings: Res<'w, PlanetSettings>,
    pub map: Res<'w, MapSettings>,
    pub registry: ResMut<'w, PatchRegistry>,
    pub asset_state: ResMut<'w, PatchAssetState>,
    pub metrics: ResMut<'w, PatchCacheMetrics>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub planet_materials: ResMut<'w, Assets<PlanetSurfaceMaterial>>,
    pub images: ResMut<'w, Assets<Image>>,
    pub context: Res<'w, PlanetContext>,
    pub q_planet_material:
        Query<'w, 's, &'static MeshMaterial3d<PlanetSurfaceMaterial>, With<PlanetTag>>,
    pub material_library: ResMut<'w, PatchMaterialLibrary>,
    pub asset_server: Res<'w, AssetServer>,
    pub profiler: ResMut<'w, ClimateProfiler>,
}

pub fn process_patch_queue(mut commands: Commands, params: ProcessPatchQueueParams) {
    if params.state.get() != &AppState::InGame {
        return;
    }

    let ProcessPatchQueueParams {
        state: _,
        mut queue,
        mut load_tasks,
        current_planet,
        params,
        settings,
        map,
        mut registry,
        mut asset_state,
        mut metrics,
        mut meshes,
        mut planet_materials,
        mut images,
        context,
        q_planet_material,
        mut material_library,
        asset_server,
        mut profiler,
    } = params;

    let planet_entity = match current_planet.0 {
        Some(entity) => entity,
        None => return,
    };

    if !matches!(
        context.layer,
        PlanetContextLayer::Approach | PlanetContextLayer::Surface
    ) {
        return;
    }

    let settings_value = *settings;

    let planet_material = match q_planet_material.iter().next() {
        Some(handle) => handle.0.clone(),
        None => return,
    };
    let base_changed =
        super::render::update_material_palette_base(&mut material_library, planet_material.clone());
    if base_changed {
        for entry in registry.entries.values_mut() {
            entry.resolved_material = None;
            if let Some(entity) = entry.entity {
                let handle = resolve_patch_material(
                    &mut material_library,
                    &mut planet_materials,
                    &planet_material,
                    &settings_value,
                    asset_server.as_ref(),
                    images.as_mut(),
                    entry,
                );
                commands.entity(entity).insert(MeshMaterial3d(handle));
            }
        }
    }

    for entry in registry.entries.values_mut() {
        let entity = match entry.entity {
            Some(entity) => entity,
            None => continue,
        };
        if entry.albedo_texture.is_none()
            && entry.normal_texture.is_none()
            && entry.roughness_texture.is_none()
        {
            continue;
        }
        let had_material = entry.resolved_material.is_some();
        let handle = resolve_patch_material(
            &mut material_library,
            &mut planet_materials,
            &planet_material,
            &settings_value,
            asset_server.as_ref(),
            images.as_mut(),
            entry,
        );
        if !had_material {
            commands.entity(entity).insert(MeshMaterial3d(handle));
        }
    }

    let params_value = *params;
    let seed_value = map.seed;
    let planet_radius = params_value.radius.max(1.0);

    load_tasks
        .tasks
        .retain(|key, _| context.desired.contains(key));

    // Reuse cached meshes: completed tasks only update existing handles.
    let mut ready: Vec<(PatchKey, PatchLoadMethod, Option<GeneratedPatch>)> = Vec::new();
    load_tasks.tasks.retain(|key, entry| {
        if let Some(result) = future::block_on(future::poll_once(&mut entry.task)) {
            ready.push((*key, entry.method, result));
            false
        } else {
            true
        }
    });

    let mut fallback_keys: Vec<PatchKey> = Vec::new();
    for (key, method, result) in ready {
        match result {
            Some(generated) => {
                match method {
                    PatchLoadMethod::Disk => {
                        metrics.disk_hits = metrics.disk_hits.saturating_add(1);
                    }
                    PatchLoadMethod::Procedural => {
                        metrics.procedural_builds = metrics.procedural_builds.saturating_add(1);
                    }
                }
                let GeneratedPatch {
                    mesh,
                    samples,
                    build_time,
                    sphere_positions,
                    displaced_positions,
                    sphere_normals,
                    displaced_normals,
                    materials,
                    props,
                    albedo_path,
                    normal_path,
                    roughness_path,
                } = generated;
                let mut patch_materials = materials;
                let mut albedo_handle = albedo_path
                    .as_ref()
                    .map(|path| asset_server.load(path.clone()));
                let mut normal_handle = normal_path
                    .as_ref()
                    .map(|path| asset_server.load(path.clone()));
                let mut roughness_handle = roughness_path
                    .as_ref()
                    .map(|path| asset_server.load(path.clone()));

                if patch_materials.is_empty()
                    || albedo_handle.is_none()
                    || normal_handle.is_none()
                    || roughness_handle.is_none()
                {
                    if let Some(parent_key) = key.parent() {
                        if let Some(parent_entry) = registry.entries.get(&parent_key) {
                            if patch_materials.is_empty() && !parent_entry.materials.is_empty() {
                                patch_materials = parent_entry.materials.clone();
                            }
                            if albedo_handle.is_none() {
                                albedo_handle = parent_entry.albedo_texture.clone();
                            }
                            if normal_handle.is_none() {
                                normal_handle = parent_entry.normal_texture.clone();
                            }
                            if roughness_handle.is_none() {
                                roughness_handle = parent_entry.roughness_texture.clone();
                            }
                        }
                    }
                }

                profiler.record(samples as u64, build_time);
                let geometry = PatchGeometry {
                    sphere_positions,
                    displaced_positions,
                    sphere_normals,
                    displaced_normals,
                };
                if let Some(entry) = registry.entries.get_mut(&key) {
                    let _ = meshes.insert(entry.mesh.id(), mesh);
                    entry.geometry = Some(geometry);
                    entry.current_morph = f32::NAN;
                    entry.materials = patch_materials.clone();
                    entry.props = props;
                    entry.resolved_material = None;
                    entry.albedo_texture = albedo_handle.clone();
                    entry.normal_texture = normal_handle.clone();
                    entry.roughness_texture = roughness_handle.clone();
                    if let Some(entity) = entry.entity {
                        let resolved = resolve_patch_material(
                            &mut material_library,
                            &mut planet_materials,
                            &planet_material,
                            &settings_value,
                            asset_server.as_ref(),
                            images.as_mut(),
                            entry,
                        );
                        commands.entity(entity).insert(MeshMaterial3d(resolved));
                    }
                } else {
                    let handle = meshes.add(mesh);
                    registry.entries.insert(
                        key,
                        PatchCacheEntry {
                            mesh: handle,
                            entity: None,
                            active_children: 0,
                            geometry: Some(geometry),
                            current_morph: f32::NAN,
                            materials: patch_materials,
                            props,
                            resolved_material: None,
                            albedo_texture: albedo_handle,
                            normal_texture: normal_handle,
                            roughness_texture: roughness_handle,
                        },
                    );
                }
            }
            None => match method {
                PatchLoadMethod::Disk => {
                    metrics.disk_misses = metrics.disk_misses.saturating_add(1);
                    asset_state.mark_missing(key);
                    fallback_keys.push(key);
                }
                PatchLoadMethod::Procedural => {
                    metrics.procedural_failures = metrics.procedural_failures.saturating_add(1);
                    fallback_keys.push(key);
                }
            },
        }
    }

    for key in fallback_keys {
        if context.desired.contains(&key) {
            queue.enqueue(key);
        }
    }

    let altitude_km = context.altitude_km;
    let spawn_budget = patch_spawn_budget(context.layer, altitude_km, planet_radius);
    let mut spawned_this_frame = 0u32;
    let mut deferred: Vec<PatchKey> = Vec::new();
    let mut active_count = registry
        .entries
        .values()
        .filter(|entry| entry.entity.is_some() && entry.active_children == 0)
        .count();
    let max_active = match context.layer {
        PlanetContextLayer::Surface => {
            let t = (altitude_km / SURFACE_ALT_CLAMP_KM).clamp(0.0, 1.0);
            let density_scale = (planet_radius / PATCH_RADIUS_REFERENCE).clamp(0.4, 2.0);
            let near = MAX_ACTIVE_SURFACE_NEAR as f32 * density_scale;
            let far = MAX_ACTIVE_SURFACE_FAR as f32 * density_scale;
            (near + (far - near) * t).round() as usize
        }
        PlanetContextLayer::Approach => {
            let t = (altitude_km / APPROACH_ALT_CLAMP_KM).clamp(0.0, 1.0);
            let density_scale = (planet_radius / PATCH_RADIUS_REFERENCE).clamp(0.4, 2.0);
            let near = MAX_ACTIVE_APPROACH_NEAR as f32 * density_scale;
            let far = MAX_ACTIVE_APPROACH_FAR as f32 * density_scale;
            (near + (far - near) * t).round() as usize
        }
        PlanetContextLayer::Orbit => 0,
    };

    while let Some((key, candidate_priority)) = queue
        .pop_best(|k| patch_priority(k, planet_radius, context.planet_center, context.camera_pos))
    {
        if !context.desired.contains(&key) {
            continue;
        }

        if spawn_budget == 0 {
            deferred.push(key);
            break;
        }

        if let Some(parent_key) = key.parent() {
            if !registry.entries.contains_key(&parent_key) {
                deferred.push(key);
                continue;
            }
        }

        if spawned_this_frame >= spawn_budget {
            deferred.push(key);
            break;
        }

        let candidate_descriptor = PatchDescriptor::from_key(key, planet_radius);
        let candidate_center_world = context.planet_center + candidate_descriptor.center;
        let candidate_dist = context.camera_pos.distance(candidate_center_world).max(1.0);

        while active_count >= max_active {
            let Some((evict_key, evict_priority, evict_entity)) =
                select_eviction_candidate(&registry, planet_radius, &context)
            else {
                break;
            };

            if context.desired.contains(&evict_key) {
                let priority_margin = candidate_priority.abs().max(1.0) * EVICT_PRIORITY_MARGIN;
                if evict_priority <= candidate_priority + priority_margin {
                    break;
                }

                let evict_descriptor = PatchDescriptor::from_key(evict_key, planet_radius);
                let evict_center_world = context.planet_center + evict_descriptor.center;
                let evict_dist = context.camera_pos.distance(evict_center_world).max(1.0);
                if evict_dist <= candidate_dist * EVICT_DISTANCE_RATIO {
                    break;
                }
            }

            if context.desired.contains(&evict_key) {
                queue.enqueue(evict_key);
            }
            despawn_surface_patch(&mut commands, &mut registry, evict_key, evict_entity);
            metrics.evictions = metrics.evictions.saturating_add(1);
            active_count = active_count.saturating_sub(1);
        }

        if active_count >= max_active {
            deferred.push(key);
            continue;
        }

        if let Some(entry) = registry.entries.get_mut(&key) {
            if entry.entity.is_none() {
                if spawned_this_frame < spawn_budget && active_count < max_active {
                    let materials = entry.materials.clone();
                    let props = entry.props.clone();
                    let material_handle = resolve_patch_material(
                        &mut material_library,
                        &mut planet_materials,
                        &planet_material,
                        &settings_value,
                        asset_server.as_ref(),
                        images.as_mut(),
                        entry,
                    );
                    let entity = spawn_surface_patch(
                        &mut commands,
                        planet_entity,
                        key,
                        entry.mesh.clone(),
                        material_handle,
                        materials,
                        props,
                    );
                    entry.entity = Some(entity);
                    if entry.active_children >= 4 {
                        commands.entity(entity).insert(Visibility::Hidden);
                    }
                    if let Some(parent_key) = key.parent() {
                        if let Some(parent_entry) = registry.entries.get_mut(&parent_key) {
                            parent_entry.active_children =
                                parent_entry.active_children.saturating_add(1);
                            if parent_entry.active_children >= 4 {
                                if let Some(parent_entity) = parent_entry.entity {
                                    commands.entity(parent_entity).insert(Visibility::Hidden);
                                }
                            }
                        }
                    }
                    spawned_this_frame += 1;
                    active_count += 1;
                } else {
                    deferred.push(key);
                }
            }
            continue;
        }

        if !load_tasks.tasks.contains_key(&key) {
            if !asset_state.is_marked_missing(&key) {
                if load_tasks.tasks.len() >= MAX_CONCURRENT_TASKS {
                    deferred.push(key);
                } else {
                    let base_path = asset_state.base_path().to_path_buf();
                    let task = AsyncComputeTaskPool::get().spawn(async move {
                        asset_loader::load_patch_bundle_from_disk(base_path, key, planet_radius)
                    });
                    load_tasks.tasks.insert(
                        key,
                        PatchLoadTask {
                            method: PatchLoadMethod::Disk,
                            task,
                        },
                    );
                    deferred.push(key);
                }
                continue;
            }
            if load_tasks.tasks.len() >= MAX_CONCURRENT_TASKS {
                deferred.push(key);
                continue;
            }
            let params_clone = params_value;
            let settings_clone = settings_value;
            let task = AsyncComputeTaskPool::get().spawn(async move {
                let climate = ClimateModel::new(seed_value, params_clone, settings_clone);
                build_patch_mesh(key, &climate)
            });
            load_tasks.tasks.insert(
                key,
                PatchLoadTask {
                    method: PatchLoadMethod::Procedural,
                    task,
                },
            );
            deferred.push(key);
            continue;
        }
        deferred.push(key);
    }

    if metrics.history_capacity == 0 {
        metrics.history_capacity = 240;
    }

    let morph_start = Instant::now();
    apply_surface_morph(&mut registry, &mut meshes, context.surface_morph);
    material_library.update_morph(&mut planet_materials, context.surface_morph);
    let morph_ms = morph_start.elapsed().as_secs_f32() * 1000.0;
    metrics.morph_time_last_ms = morph_ms;
    let alpha = 0.1;
    metrics.morph_time_avg_ms = if metrics.morph_time_avg_ms <= 0.0 {
        morph_ms
    } else {
        metrics.morph_time_avg_ms * (1.0 - alpha) + morph_ms * alpha
    };
    let now = Instant::now();
    let should_log = metrics
        .last_morph_log
        .map(|last| now.duration_since(last) >= Duration::from_secs(1))
        .unwrap_or(true);
    if should_log {
        debug!(
            "surface morph {:.1}% | morph {:.3} ms (avg {:.3} ms) | active patches {}",
            context.surface_morph * 100.0,
            morph_ms,
            metrics.morph_time_avg_ms,
            active_count
        );
        metrics.last_morph_log = Some(now);
    }

    metrics.active_patches = active_count;
    metrics.max_active_budget = max_active;
    metrics.spawn_budget_last = spawn_budget;

    metrics.active_history.push_back(active_count);
    if metrics.active_history.len() > metrics.history_capacity {
        metrics.active_history.pop_front();
    }
    metrics.spawn_history.push_back(spawn_budget);
    if metrics.spawn_history.len() > metrics.history_capacity {
        metrics.spawn_history.pop_front();
    }

    for key in deferred {
        queue.enqueue(key);
    }
}

fn apply_surface_morph(registry: &mut PatchRegistry, _meshes: &mut Assets<Mesh>, morph: f32) {
    for entry in registry.entries.values_mut() {
        entry.current_morph = morph;
    }
}

fn patch_spawn_budget(layer: PlanetContextLayer, altitude_km: f32, planet_radius: f32) -> u32 {
    let radius_scale = (planet_radius / PATCH_RADIUS_REFERENCE).clamp(0.4, 2.0);
    let value = match layer {
        PlanetContextLayer::Surface => {
            let t = (altitude_km / SURFACE_ALT_CLAMP_KM).clamp(0.0, 1.0);
            let near = PATCHES_PER_FRAME_SURFACE_NEAR * radius_scale;
            let far = PATCHES_PER_FRAME_SURFACE_FAR * radius_scale;
            near + (far - near) * t
        }
        PlanetContextLayer::Approach => {
            let t = (altitude_km / APPROACH_ALT_CLAMP_KM).clamp(0.0, 1.0);
            let near = PATCHES_PER_FRAME_APPROACH_NEAR * radius_scale;
            let far = PATCHES_PER_FRAME_APPROACH_FAR * radius_scale;
            near + (far - near) * t
        }
        PlanetContextLayer::Orbit => 0.0,
    };
    if value <= 0.0 {
        0
    } else {
        value.round().max(1.0) as u32
    }
}

fn build_patch_mesh(key: PatchKey, climate: &ClimateModel) -> Option<GeneratedPatch> {
    let start = Instant::now();
    let (u0, v0, u1, v1) = lod::patch_uv_bounds(key);
    let verts = verts_per_side(key.level).max(2);
    let sample_count = (verts * verts) as u32;

    let mut positions: Vec<Vec3> = Vec::with_capacity(verts * verts);
    let mut detail_normals: Vec<Vec3> = Vec::with_capacity(verts * verts);
    let mut uv0s: Vec<[f32; 2]> = Vec::with_capacity(verts * verts);
    let mut uv1s: Vec<[f32; 2]> = Vec::with_capacity(verts * verts);
    let mut packed_attributes: Vec<[f32; 4]> = Vec::with_capacity(verts * verts);
    let base_radius = climate.radius().max(1.0);
    let mut min_height = f32::MAX;
    let mut max_height = f32::MIN;
    let mut sphere_positions: Vec<Vec3> = Vec::with_capacity(verts * verts);
    let mut sphere_normals: Vec<Vec3> = Vec::with_capacity(verts * verts);

    for j in 0..verts {
        let v = lerp(v0, v1, j as f32 / (verts - 1) as f32);
        for i in 0..verts {
            let u = lerp(u0, u1, i as f32 / (verts - 1) as f32);
            let dir = lod::cube_uv_to_dir(key.face, u, v);
            let sample = climate.sample(dir);
            positions.push(sample.position);
            detail_normals.push(sample.normal.normalize_or_zero());
            uv0s.push(sample.uv0);
            uv1s.push(sample.uv1);
            packed_attributes.push(sample.packed);
            let height = sample.position.length() - base_radius;
            min_height = min_height.min(height);
            max_height = max_height.max(height);
            let sphere_pos = dir * base_radius;
            sphere_positions.push(sphere_pos);
            sphere_normals.push(dir.normalize_or_zero());
        }
    }

    let mut indices: Vec<u32> = Vec::with_capacity((verts - 1) * (verts - 1) * 6);
    for j in 0..(verts - 1) {
        for i in 0..(verts - 1) {
            let a = (j * verts + i) as u32;
            let b = a + 1;
            let c = a + verts as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }

    let smooth_normals = compute_smooth_normals(&positions, &indices);
    let mut normals: Vec<Vec3> = smooth_normals
        .into_iter()
        .zip(detail_normals.into_iter())
        .map(|(smooth, detail)| (smooth * 0.55 + detail * 0.45).normalize_or_zero())
        .collect();

    let base_vertex_count = positions.len();
    let mut skirt_map: Vec<Option<u32>> = vec![None; base_vertex_count];
    if !min_height.is_finite() || !max_height.is_finite() {
        min_height = 0.0;
        max_height = 0.0;
    }
    let local_range = (max_height - min_height).max(0.0);
    let height_amp = climate.height_amp().max(1.0);
    let min_depth = 20.0;
    let fallback_depth = (height_amp * 0.05).max(min_depth);
    let dynamic_depth = local_range * 1.5;
    let mut target_depth = dynamic_depth.max(fallback_depth);
    let max_depth_amp = height_amp * 1.25;
    target_depth = target_depth.min(max_depth_amp);
    let radius_cap = (base_radius * 0.002).max(fallback_depth);
    let base_depth = target_depth.min(radius_cap).max(min_depth);
    let level_norm = (key.level as f32).min(6.0) / 6.0;
    let lod_scale = 0.35 + 0.65 * level_norm;
    let skirt_depth = base_depth * lod_scale;

    for j in 0..verts {
        for i in 0..verts {
            if i == 0 || i == verts - 1 || j == 0 || j == verts - 1 {
                let idx = j * verts + i;
                if skirt_map[idx].is_some() {
                    continue;
                }
                let base_pos = positions[idx];
                let dir = base_pos.normalize_or_zero();
                let skirt_pos = base_pos - dir * skirt_depth;
                positions.push(skirt_pos);
                let skirt_normal = normals[idx];
                normals.push(skirt_normal);

                let uv0 = uv0s[idx];
                let uv1 = uv1s[idx];
                let mut packed = packed_attributes[idx];
                packed[3] = mark_skirt_vertex(packed[3]);
                uv0s.push(uv0);
                uv1s.push(uv1);
                packed_attributes.push(packed);

                skirt_map[idx] = Some((positions.len() - 1) as u32);

                if let Some(base_sphere) = sphere_positions.get(idx).copied() {
                    sphere_positions.push(base_sphere);
                    sphere_normals.push(base_sphere.normalize_or_zero());
                } else {
                    sphere_positions.push(base_pos.normalize_or_zero() * base_radius);
                    sphere_normals.push(base_pos.normalize_or_zero());
                }
            }
        }
    }

    let mut add_skirt_quad = |a: usize, b: usize| {
        if let (Some(sa), Some(sb)) = (skirt_map[a], skirt_map[b]) {
            let a = a as u32;
            let b = b as u32;
            indices.extend_from_slice(&[a, b, sa]);
            indices.extend_from_slice(&[sa, b, sb]);
        }
    };

    let last_row = verts - 1;
    for i in 0..last_row {
        add_skirt_quad(i, i + 1);

        let bottom_a = last_row * verts + i;
        add_skirt_quad(bottom_a, bottom_a + 1);

        let left_a = i * verts;
        add_skirt_quad(left_a, left_a + verts);

        let right_col = last_row;
        let right_a = i * verts + right_col;
        add_skirt_quad(right_a, right_a + verts);
    }

    let positions_array: Vec<[f32; 3]> = positions.iter().map(|p| p.to_array()).collect();
    let normals_array: Vec<[f32; 3]> = normals.iter().map(|n| n.to_array()).collect();

    let displaced_positions = positions.clone();
    let displaced_normals = normals.clone();

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions_array);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals_array);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv0s);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uv1s);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, packed_attributes);
    let tangent_data: Vec<[f32; 4]> = sphere_positions
        .iter()
        .map(|pos| [pos.x, pos.y, pos.z, 1.0])
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangent_data);
    mesh.insert_indices(Indices::U32(indices));
    let build_time = start.elapsed();
    Some(GeneratedPatch {
        mesh,
        samples: sample_count,
        build_time,
        sphere_positions,
        displaced_positions,
        sphere_normals,
        displaced_normals,
        materials: Vec::new(),
        props: Vec::new(),
        albedo_path: None,
        normal_path: None,
        roughness_path: None,
    })
}

fn compute_smooth_normals(positions: &[Vec3], indices: &[u32]) -> Vec<Vec3> {
    let mut accum = vec![Vec3::ZERO; positions.len()];

    for tri in indices.chunks_exact(3) {
        let ia = tri[0] as usize;
        let ib = tri[1] as usize;
        let ic = tri[2] as usize;

        let a = positions[ia];
        let b = positions[ib];
        let c = positions[ic];

        let n = (b - a).cross(c - a);
        if n.length_squared() > 0.0 {
            accum[ia] += n;
            accum[ib] += n;
            accum[ic] += n;
        }
    }

    accum
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            if n.length_squared() > 0.0 {
                n.normalize()
            } else {
                positions[i].normalize_or_zero()
            }
        })
        .collect()
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn verts_per_side(level: u8) -> usize {
    match level {
        0 => 10,
        1 => 13,
        2 => 16,
        3 => 19,
        4 => 22,
        5 => 24,
        _ => 24,
    }
}

fn resolve_patch_material(
    material_library: &mut PatchMaterialLibrary,
    planet_materials: &mut Assets<PlanetSurfaceMaterial>,
    base_handle: &Handle<PlanetSurfaceMaterial>,
    settings: &PlanetSettings,
    asset_server: &AssetServer,
    images: &mut Assets<Image>,
    entry: &mut PatchCacheEntry,
) -> Handle<PlanetSurfaceMaterial> {
    if entry.materials.is_empty() {
        entry.materials.push(String::from("palette:default"));
    } else if !entry.materials.iter().any(|m| m.starts_with("palette:")) {
        entry.materials.push(String::from("palette:default"));
    }
    let handle = match entry.resolved_material.clone() {
        Some(handle) => handle,
        None => {
            let base_variant = material_library
                .base
                .as_ref()
                .cloned()
                .unwrap_or_else(|| base_handle.clone());
            let handle = material_library.resolve(
                &entry.materials,
                &base_variant,
                planet_materials,
                settings,
            );
            entry.resolved_material = Some(handle.clone());
            handle
        }
    };
    if let Some(material) = planet_materials.get_mut(&handle) {
        if let Some(albedo) = entry.albedo_texture.clone() {
            match asset_server.get_load_state(albedo.id()) {
                Some(LoadState::Loaded) => {
                    match ensure_filterable_texture(&albedo, images, "albedo") {
                        FilterableTextureStatus::Ready => {
                            material.base.base_color_texture = Some(albedo);
                            material.base.base_color = Color::WHITE;
                        }
                        FilterableTextureStatus::Pending => {
                            material.base.base_color_texture = None;
                        }
                        FilterableTextureStatus::Unsupported => {
                            entry.albedo_texture = None;
                            material.base.base_color_texture = None;
                        }
                    }
                }
                Some(LoadState::Failed(_)) => {
                    entry.albedo_texture = None;
                    material.base.base_color_texture = None;
                }
                _ => {
                    material.base.base_color_texture = None;
                }
            }
        } else {
            material.base.base_color_texture = None;
        }
        if let Some(normal) = entry.normal_texture.clone() {
            match asset_server.get_load_state(normal.id()) {
                Some(LoadState::Loaded) => {
                    match ensure_filterable_texture(&normal, images, "normal") {
                        FilterableTextureStatus::Ready => {
                            material.base.normal_map_texture = Some(normal);
                        }
                        FilterableTextureStatus::Pending => {
                            material.base.normal_map_texture = None;
                        }
                        FilterableTextureStatus::Unsupported => {
                            entry.normal_texture = None;
                            material.base.normal_map_texture = None;
                        }
                    }
                }
                Some(LoadState::Failed(_)) => {
                    entry.normal_texture = None;
                    material.base.normal_map_texture = None;
                }
                _ => {
                    material.base.normal_map_texture = None;
                }
            }
        } else {
            material.base.normal_map_texture = None;
        }
        if let Some(roughness) = entry.roughness_texture.clone() {
            match asset_server.get_load_state(roughness.id()) {
                Some(LoadState::Loaded) => {
                    match ensure_filterable_texture(&roughness, images, "roughness") {
                        FilterableTextureStatus::Ready => {
                            material.base.metallic_roughness_texture = Some(roughness);
                        }
                        FilterableTextureStatus::Pending => {
                            material.base.metallic_roughness_texture = None;
                        }
                        FilterableTextureStatus::Unsupported => {
                            entry.roughness_texture = None;
                            material.base.metallic_roughness_texture = None;
                        }
                    }
                }
                Some(LoadState::Failed(_)) => {
                    entry.roughness_texture = None;
                    material.base.metallic_roughness_texture = None;
                }
                _ => {
                    material.base.metallic_roughness_texture = None;
                }
            }
        } else {
            material.base.metallic_roughness_texture = None;
        }
    }
    handle
}

enum FilterableTextureStatus {
    Pending,
    Ready,
    Unsupported,
}

fn ensure_filterable_texture(
    handle: &Handle<Image>,
    images: &mut Assets<Image>,
    label: &str,
) -> FilterableTextureStatus {
    let Some(image) = images.get_mut(handle) else {
        return FilterableTextureStatus::Pending;
    };
    match image.texture_descriptor.format {
        TextureFormat::R8Uint => {
            reinterpret_image_format(image, TextureFormat::R8Unorm);
            FilterableTextureStatus::Ready
        }
        TextureFormat::R16Uint => {
            reinterpret_image_format(image, TextureFormat::R16Unorm);
            FilterableTextureStatus::Ready
        }
        TextureFormat::Rg8Uint => {
            reinterpret_image_format(image, TextureFormat::Rg8Unorm);
            FilterableTextureStatus::Ready
        }
        TextureFormat::Rg16Uint => {
            reinterpret_image_format(image, TextureFormat::Rg16Unorm);
            FilterableTextureStatus::Ready
        }
        TextureFormat::Rgba8Uint => {
            reinterpret_image_format(image, TextureFormat::Rgba8Unorm);
            FilterableTextureStatus::Ready
        }
        TextureFormat::Rgba16Uint => {
            reinterpret_image_format(image, TextureFormat::Rgba16Unorm);
            FilterableTextureStatus::Ready
        }
        TextureFormat::R32Uint
        | TextureFormat::Rg32Uint
        | TextureFormat::Rgba32Uint
        | TextureFormat::R8Sint
        | TextureFormat::R16Sint
        | TextureFormat::Rg8Sint
        | TextureFormat::Rg16Sint
        | TextureFormat::Rgba8Sint
        | TextureFormat::Rgba16Sint
        | TextureFormat::R32Sint
        | TextureFormat::Rg32Sint
        | TextureFormat::Rgba32Sint => {
            warn!(
                "Texture with format {:?} is incompatible with filterable sampling (used for {label}); skipping.",
                image.texture_descriptor.format
            );
            FilterableTextureStatus::Unsupported
        }
        _ => FilterableTextureStatus::Ready,
    }
}

fn reinterpret_image_format(image: &mut Image, new_format: TextureFormat) {
    debug!(
        "Reinterpreting texture from {:?} to {:?}",
        image.texture_descriptor.format, new_format
    );
    image.texture_descriptor.format = new_format;
    if let Some(view_descriptor) = &mut image.texture_view_descriptor {
        view_descriptor.format = Some(new_format);
    }
}

fn spawn_surface_patch(
    commands: &mut Commands,
    planet_entity: Entity,
    key: PatchKey,
    mesh: Handle<Mesh>,
    material: Handle<PlanetSurfaceMaterial>,
    materials: Vec<String>,
    props: Vec<PatchPropPlaceholder>,
) -> Entity {
    let mut entity_commands = commands.spawn((
        SurfacePatch { key },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::IDENTITY,
        GlobalTransform::IDENTITY,
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        Name::new(format!(
            "SurfacePatch face{}-lvl{}-({},{})",
            key.face, key.level, key.ix, key.iy
        )),
        PatchMaterials {
            identifiers: materials.clone(),
        },
    ));
    let entity = entity_commands.id();
    if !props.is_empty() {
        entity_commands.with_children(|children| {
            for (index, prop) in props.iter().enumerate() {
                let rotation = Quat::from_euler(
                    EulerRot::XYZ,
                    prop.rotation_euler.x,
                    prop.rotation_euler.y,
                    prop.rotation_euler.z,
                );
                let transform = Transform {
                    translation: prop.position,
                    rotation,
                    scale: prop.scale,
                };
                children.spawn((
                    PatchPropInstance {
                        kind: prop.kind.clone(),
                    },
                    transform,
                    GlobalTransform::IDENTITY,
                    Visibility::Visible,
                    Name::new(format!("PropPlaceholder {} #{}", prop.kind, index)),
                ));
            }
        });
    }
    commands.entity(planet_entity).add_child(entity);
    entity
}

fn despawn_surface_patch(
    commands: &mut Commands,
    registry: &mut PatchRegistry,
    key: PatchKey,
    entity: Entity,
) {
    if let Some(entry) = registry.entries.get_mut(&key) {
        entry.entity = None;
    }
    commands.entity(entity).despawn();
    if let Some(parent) = key.parent() {
        if let Some(parent_entry) = registry.entries.get_mut(&parent) {
            parent_entry.active_children = parent_entry.active_children.saturating_sub(1);
            if parent_entry.active_children < 4 {
                if let Some(parent_entity) = parent_entry.entity {
                    commands.entity(parent_entity).insert(Visibility::Visible);
                }
            }
        }
    }
}

pub fn prune_surface_patches(
    mut commands: Commands,
    context: Res<PlanetContext>,
    patches: Query<(Entity, &SurfacePatch)>,
    mut registry: ResMut<PatchRegistry>,
) {
    match context.layer {
        PlanetContextLayer::Approach | PlanetContextLayer::Surface => {
            for (entity, patch) in patches.iter() {
                if context.desired.contains(&patch.key) {
                    continue;
                }
                if let Some(entry) = registry.entries.get(&patch.key) {
                    if entry.active_children > 0 {
                        continue;
                    }
                }
                despawn_surface_patch(&mut commands, &mut registry, patch.key, entity);
            }
        }
        PlanetContextLayer::Orbit => {
            for (entity, patch) in patches.iter() {
                despawn_surface_patch(&mut commands, &mut registry, patch.key, entity);
            }
        }
    }
}
