#import bevy_pbr::{
    forward_io::{FragmentOutput, Vertex, VertexOutput},
    mesh_bindings::mesh,
    mesh_functions,
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types,
    view_transformations::position_world_to_clip,
}

struct PlanetSurfaceUniform {
    seed: u32,
    debug_mode: u32,
    _pad0: u32,
<<<<<<< HEAD
    _pad1: u32,
    sea_level: f32,
    base_freq: f32,
    detail_freq: f32,
    warp_freq: f32,
    warp_amp: f32,
    coast_width: f32,
    mountain_strength: f32,
    mountain_scale: f32,
    axial_tilt: f32,
    temp_shift: f32,
    moisture_bias_global: f32,
    dryness_bias_global: f32,
    normal_strength: f32,
    perceptual_roughness: f32,
    metallic: f32,
    reflectance: f32,
    sand_height: f32,
    rock_start: f32,
    snow_start: f32,
    sun_dir: vec4<f32>,
    water_deep: vec4<f32>,
    water_shallow: vec4<f32>,
    land_sand: vec4<f32>,
    land_grass: vec4<f32>,
=======
    _pad1: u32,
    sea_level: f32,
    base_freq: f32,
    detail_freq: f32,
    warp_freq: f32,
    warp_amp: f32,
    coast_width: f32,
    mountain_strength: f32,
    mountain_scale: f32,
    axial_tilt: f32,
    temp_shift: f32,
    moisture_bias_global: f32,
    dryness_bias_global: f32,
    normal_strength: f32,
    perceptual_roughness: f32,
    metallic: f32,
    reflectance: f32,
    rock_start: f32,
    snow_start: f32,
    sun_dir: vec4<f32>,
    water_deep: vec4<f32>,
    water_shallow: vec4<f32>,
    land_sand: vec4<f32>,
    land_grass: vec4<f32>,
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
    land_rock: vec4<f32>,
    land_snow: vec4<f32>,
    surface_detail_amp: f32,
    surface_detail_scale: f32,
    surface_morph: f32,
    _pad_surface: vec3<f32>,
};

@group(3) @binding(31)
var<uniform> material: PlanetSurfaceUniform;

const SKIRT_FLAG_BIT: u32 = 0x8000u;
const SKIRT_LAND_MASK_BITS: u32 = 0x7FFFu;
const SKIRT_LAND_MASK_SCALE: f32 = 32767.0;

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let sphere_local = vec4<f32>(vertex.tangent.xyz, 1.0);
    let displaced_local = vec4<f32>(vertex.position, 1.0);
    let morph = clamp(material.surface_morph, 0.0, 1.0);
    let local_position = mix(sphere_local, displaced_local, morph);

    let world_position = mesh_functions::mesh_position_local_to_world(world_from_local, local_position);
    out.world_position = world_position;
    out.position = position_world_to_clip(world_position.xyz);

    let sphere_normal = normalize(vertex.tangent.xyz);
    let displaced_normal = vertex.normal;
    let local_normal = normalize(mix(sphere_normal, displaced_normal, morph));
    out.world_normal = mesh_functions::mesh_normal_local_to_world(local_normal, vertex.instance_index);

#ifdef VERTEX_UVS_A
    out.uv = vertex.uv;
#endif
#ifdef VERTEX_UVS_B
    out.uv_b = vertex.uv_b;
#endif
#ifdef VERTEX_TANGENTS
    out.world_tangent = vec4<f32>(0.0, 0.0, 0.0, 0.0);
#endif
#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    out.instance_index = vertex.instance_index;
#endif
#ifdef VISIBILITY_RANGE_DITHER
    out.visibility_range_dither =
        mesh_functions::get_visibility_range_dither_level(vertex.instance_index, world_position);
#endif

    return out;
}

fn smooth3(t: f32) -> f32 {
    return t * t * (3.0 - 2.0 * t);
}

fn smooth3_derivative(t: f32) -> f32 {
    return 6.0 * t * (1.0 - t);
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn hash3(seed: u32, ix: i32, iy: i32, iz: i32) -> f32 {
    var v = seed;
    v ^= bitcast<u32>(ix) * 0x9E3779B9u;
    v ^= bitcast<u32>(iy) * 0xBB67AE85u;
    v ^= bitcast<u32>(iz) * 0xC2B2AE3Du;
    v ^= v >> 16u;
    v *= 0x7FEB352Du;
    v ^= v >> 15u;
    v *= 0x846CA68Bu;
    v ^= v >> 16u;
    return f32(v) / 4294967295.0;
}

fn decode_unit_octa(octa: vec2<f32>) -> vec3<f32> {
    var normal = vec3<f32>(
        octa.x * 2.0 - 1.0,
        octa.y * 2.0 - 1.0,
        1.0 - abs(octa.x * 2.0 - 1.0) - abs(octa.y * 2.0 - 1.0),
    );
    if normal.z < 0.0 {
        let x = normal.x;
        let y = normal.y;
        normal.x = (1.0 - abs(y)) * sign(x);
        normal.y = (1.0 - abs(x)) * sign(y);
        normal.z = -normal.z;
    }
    return normalize(normal);
}

fn decode_pair(value: f32) -> vec2<f32> {
    let bits = bitcast<u32>(value);
    let hi = f32(bits >> 16u) / 65535.0;
    let lo = f32(bits & 0xFFFFu) / 65535.0;
    return vec2(hi, lo);
}

struct NoiseSample {
    value: f32,
    grad: vec3<f32>,
}

struct ClimateFields {
    continent_value: f32,
    land_mask: f32,
    macro_relief: f32,
    mountain_seed: f32,
    erosion: f32,
    detail: f32,
    moisture_noise: f32,
}

struct ClimateDebugSample {
    continent_value: f32,
    land_mask: f32,
    height01: f32,
    moisture: f32,
    temperature: f32,
    slope: f32,
}

fn value3_with_derivative(seed: u32, coord: vec3<f32>) -> NoiseSample {
    let x0 = i32(floor(coord.x));
    let y0 = i32(floor(coord.y));
    let z0 = i32(floor(coord.z));
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;

    let fx = coord.x - floor(coord.x);
    let fy = coord.y - floor(coord.y);
    let fz = coord.z - floor(coord.z);

    let tx = smooth3(fx);
    let ty = smooth3(fy);
    let tz = smooth3(fz);
    let dtx = smooth3_derivative(fx);
    let dty = smooth3_derivative(fy);
    let dtz = smooth3_derivative(fz);

    let c000 = hash3(seed, x0, y0, z0);
    let c100 = hash3(seed, x1, y0, z0);
    let c010 = hash3(seed, x0, y1, z0);
    let c110 = hash3(seed, x1, y1, z0);
    let c001 = hash3(seed, x0, y0, z1);
    let c101 = hash3(seed, x1, y0, z1);
    let c011 = hash3(seed, x0, y1, z1);
    let c111 = hash3(seed, x1, y1, z1);

    let x00 = mix(c000, c100, tx);
    let x10 = mix(c010, c110, tx);
    let x01 = mix(c001, c101, tx);
    let x11 = mix(c011, c111, tx);

    let y_mix0 = mix(x00, x10, ty);
    let y_mix1 = mix(x01, x11, ty);
    let value = mix(y_mix0, y_mix1, tz);

    let dx00 = (c100 - c000) * dtx;
    let dx10 = (c110 - c010) * dtx;
    let dx01 = (c101 - c001) * dtx;
    let dx11 = (c111 - c011) * dtx;
    let dy0_dx = mix(dx00, dx10, ty);
    let dy1_dx = mix(dx01, dx11, ty);
    let grad_x = mix(dy0_dx, dy1_dx, tz);

    let dy0 = (x10 - x00) * dty;
    let dy1 = (x11 - x01) * dty;
    let grad_y = mix(dy0, dy1, tz);

    let grad_z = (y_mix1 - y_mix0) * dtz;

    return NoiseSample(value, vec3(grad_x, grad_y, grad_z));
}

fn fbm3_with_derivative(seed: u32, x: f32, y: f32, z: f32, base_freq: f32) -> NoiseSample {
    var frequency = max(base_freq, 1e-4);
    var amplitude = 1.0;
    var total = 0.0;
    var grad = vec3(0.0);
    var norm = 0.0;

    for (var octave: u32 = 0u; octave < 5u; octave = octave + 1u) {
        let sample = value3_with_derivative(seed ^ octave, vec3(x, y, z) * frequency);
        total = total + sample.value * amplitude;
        grad = grad + sample.grad * (amplitude * frequency);
        norm = norm + amplitude;
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return NoiseSample(clamp(total / norm, 0.0, 1.0), grad / norm);
}

fn fbm3_value(seed: u32, coord: vec3<f32>, base_freq: f32) -> f32 {
    return fbm3_with_derivative(seed, coord.x, coord.y, coord.z, base_freq).value;
}

fn to_signed(value: f32) -> f32 {
    return value * 2.0 - 1.0;
}

fn climate_sample_fields(coord: vec3<f32>) -> ClimateFields {
    let base_f = max(material.base_freq, 1e-5);
    let detail_f = max(material.detail_freq, 0.35);

    let plate_a = to_signed(fbm3_value(
        material.seed,
        vec3(coord.x * 0.55, coord.y * 0.53, coord.z * 0.57),
        base_f * 0.55,
    ));
    let plate_b = to_signed(fbm3_value(
        material.seed ^ 0x11u,
        vec3(coord.z * 0.48, coord.x * 0.52, coord.y * 0.51),
        base_f * 0.45,
    ));
    let coast_noise = to_signed(fbm3_value(
        material.seed ^ 0x21u,
        vec3(coord.x * 1.2, coord.y * 1.1, coord.z * 1.15),
        base_f * 1.05,
    ));

    let continent_value = plate_a * 0.75 + plate_b * 0.45 + coast_noise * 0.28 - 0.05;
    let land_mask = smoothstep(-0.18, 0.22, continent_value);

    let macro_relief = pow(
        fbm3_value(
            material.seed ^ 0x33u,
            vec3(coord.x * 0.95, coord.y * 0.85, coord.z * 0.9),
            base_f * 0.9,
        ),
        1.6,
    );
    let mountain_seed = fbm3_value(
        material.seed ^ 0x43u,
        vec3(coord.y * 1.8, coord.z * 1.6, coord.x * 1.7),
        detail_f * 0.7,
    );
    let erosion = pow(
        fbm3_value(
            material.seed ^ 0x53u,
            vec3(coord.z * 1.4, coord.x * 1.3, coord.y * 1.35),
            detail_f * 0.8,
        ),
        1.8,
    );
    var detail = fbm3_value(
        material.seed ^ 0x63u,
        vec3(coord.x * 3.3, coord.y * 3.1, coord.z * 3.2),
        detail_f * 2.1,
    );
    let moisture_noise = fbm3_value(
        material.seed ^ 0x73u,
        vec3(coord.x * 1.05, coord.y * 1.02, coord.z * 1.03),
        base_f * 1.3,
    );

    let detail_hi = fbm3_value(
        material.seed ^ 0x93u,
        vec3(coord.x * 6.0, coord.y * 5.8, coord.z * 5.9),
        max(detail_f, 0.6) * 3.5,
    );
    detail = mix(detail, detail_hi, 0.35);

    return ClimateFields(
        continent_value,
        land_mask,
        macro_relief,
        mountain_seed,
        erosion,
        detail,
        moisture_noise,
    );
}

fn compose_height_from_fields(fields: ClimateFields) -> f32 {
    var height =
        fields.continent_value * 0.48 + (fields.land_mask - 0.5) * 0.65;
    height = height + (fields.macro_relief - 0.5) * 0.42;
    height = height + (fields.erosion - 0.5) * 0.22;
    height = height + pow(max(fields.mountain_seed, 1e-6), 0.9) * material.mountain_scale * 1.12;
    height = height + (fields.detail - 0.5) * 0.09;
    height = height + pow(fields.land_mask, 2.2) * 0.08;
    return clamp(height * 0.52 + 0.48, 0.0, 1.0);
}

fn climate_debug_evaluate(unit: vec3<f32>) -> ClimateDebugSample {
    let warp_f = max(material.warp_freq, 1e-5);
    let seed = material.seed;

    let warp_offset = vec3(
        fbm3_value(seed ^ 0xA1u, vec3(unit.x, unit.y, unit.z), warp_f) - 0.5,
        fbm3_value(seed ^ 0xB2u, vec3(unit.z, unit.x, unit.y), warp_f) - 0.5,
        fbm3_value(seed ^ 0xC3u, vec3(unit.y, unit.z, unit.x), warp_f) - 0.5,
    );
    let warped = unit + warp_offset * material.warp_amp;

    let center = climate_sample_fields(warped);
    let height01 = compose_height_from_fields(center);

    let eps = 0.012;
    let sample_px = climate_sample_fields(warped + vec3(eps, 0.0, 0.0));
    let sample_mx = climate_sample_fields(warped - vec3(eps, 0.0, 0.0));
    let sample_py = climate_sample_fields(warped + vec3(0.0, eps, 0.0));
    let sample_my = climate_sample_fields(warped - vec3(0.0, eps, 0.0));
    let sample_pz = climate_sample_fields(warped + vec3(0.0, 0.0, eps));
    let sample_mz = climate_sample_fields(warped - vec3(0.0, 0.0, eps));

    let hx1 = compose_height_from_fields(sample_px);
    let hx0 = compose_height_from_fields(sample_mx);
    let hy1 = compose_height_from_fields(sample_py);
    let hy0 = compose_height_from_fields(sample_my);
    let hz1 = compose_height_from_fields(sample_pz);
    let hz0 = compose_height_from_fields(sample_mz);

    let grad_x = (hx1 - hx0) / (2.0 * eps);
    let grad_y = (hy1 - hy0) / (2.0 * eps);
    let grad_z = (hz1 - hz0) / (2.0 * eps);

    let slope = min(
        clamp(sqrt(grad_x * grad_x + grad_y * grad_y + grad_z * grad_z), 0.0, 1.4),
        1.0,
    );

    let lat_abs = abs(unit.y);

    let land_grad_x = abs(sample_px.land_mask - sample_mx.land_mask);
    let land_grad_y = abs(sample_py.land_mask - sample_my.land_mask);
    let land_grad_z = abs(sample_pz.land_mask - sample_mz.land_mask);
    let coast_gradient = clamp(
        sqrt(
            (land_grad_x * land_grad_x + land_grad_y * land_grad_y + land_grad_z * land_grad_z)
                / 3.0,
        ),
        0.0,
        1.0,
    );

    let coast_band = clamp(center.land_mask * (1.0 - center.land_mask) * 4.0, 0.0, 1.0);
    let interior = pow(center.land_mask, 3.0);
    let dryness_distance =
        max(1.0 - clamp(coast_band + coast_gradient * 0.6, 0.0, 1.2), 0.0);
    let dryness_base =
        clamp(pow(interior, 1.1) * pow(dryness_distance, 1.2), 0.0, 1.0);
    let dryness_noise = (center.detail - 0.5) * 0.2;
    let dryness = clamp(
        dryness_base + material.dryness_bias_global + dryness_noise,
        0.0,
        1.0,
    );

    let rain_shadow = pow(center.mountain_seed * dryness_distance, 1.3);
    var moisture = clamp(
        center.moisture_noise * 0.55 + coast_band * 1.05 - interior * 0.35,
        0.0,
        1.0,
    );
    moisture = clamp(moisture - rain_shadow * 0.45, 0.0, 1.0);
    moisture = moisture + (1.0 - interior) * 0.04;
    moisture = clamp(moisture + material.moisture_bias_global, 0.0, 1.0);
    moisture = clamp(moisture - dryness * 0.25, 0.0, 1.0);

    let elev01 =
        clamp((height01 - material.sea_level) / max(1.0 - material.sea_level, 1e-3), 0.0, 1.0);
    let tilt_cooling = (material.axial_tilt * 1.15) * pow(lat_abs, 1.1);
    let base_temp = clamp(
        1.0 - pow(lat_abs, 1.45) - tilt_cooling + material.temp_shift,
        0.0,
        1.0,
    );
    let altitude_cooling = pow(elev01, 0.65) * 0.7 + pow(slope, 0.75) * 0.15;
    let temp_variation = (center.detail - 0.5) * 0.18;
    let moisture_cooling = moisture * 0.1;
    var temperature = clamp(
        base_temp + temp_variation - altitude_cooling - moisture_cooling + dryness * 0.05,
        0.0,
        1.0,
    );
    temperature = temperature - smoothstep(0.72, 0.98, lat_abs) * 0.18;
    temperature = clamp(temperature, 0.0, 1.0);

    return ClimateDebugSample(
        center.continent_value,
        center.land_mask,
        height01,
        moisture,
        temperature,
        slope,
    );
}

fn biome_color(temperature: f32, moisture: f32) -> vec3<f32> {
    let desert = vec3(0.86, 0.75, 0.46);
    let savanna = vec3(0.74, 0.66, 0.36);
    let shrubland = vec3(0.58, 0.63, 0.37);
    let grassland = vec3(0.38, 0.56, 0.28);
    let temperate_forest = vec3(0.26, 0.48, 0.26);
    let rainforest = vec3(0.16, 0.42, 0.23);
    let boreal = vec3(0.24, 0.4, 0.31);
    let tundra = vec3(0.72, 0.75, 0.74);

    if temperature < 0.2 {
        return mix(tundra, boreal, pow(moisture, 1.1));
    } else if temperature < 0.4 {
        let cool = mix(boreal, temperate_forest, pow(moisture, 0.9));
        return mix(cool, tundra, pow(1.0 - moisture, 1.4) * 0.35);
    } else if temperature < 0.6 {
        let dry = shrubland;
        let wet = mix(grassland, temperate_forest, 0.6);
        return mix(dry, wet, pow(moisture, 0.85));
    } else if temperature < 0.8 {
        let dry = savanna;
        let wet = rainforest;
        return mix(dry, wet, pow(moisture, 0.9));
    } else {
        let dry = desert;
        let wet = mix(savanna, rainforest, 0.5);
        return mix(dry, wet, pow(moisture, 1.1));
    }
}

fn debug_color(
    mode: u32,
    continent_value: f32,
    land_mask: f32,
    height01: f32,
    moisture: f32,
    temperature: f32,
    slope: f32,
) -> vec3<f32> {
    switch mode {
        case 1u: {
            let t = clamp((continent_value + 1.0) * 0.5, 0.0, 1.0);
            return mix(vec3(0.1, 0.25, 0.65), vec3(0.85, 0.72, 0.42), t);
        }
        case 2u: {
            return mix(vec3(0.08, 0.16, 0.45), vec3(0.92, 0.9, 0.45), clamp(land_mask, 0.0, 1.0));
        }
        case 3u: {
            return vec3(clamp(height01, 0.0, 1.0));
        }
        case 4u: {
            return mix(vec3(0.85, 0.7, 0.35), vec3(0.18, 0.48, 0.25), clamp(moisture, 0.0, 1.0));
        }
        case 5u: {
            return mix(vec3(0.15, 0.28, 0.68), vec3(0.95, 0.42, 0.12), clamp(temperature, 0.0, 1.0));
        }
        case 6u: {
            return mix(vec3(0.12, 0.12, 0.12), vec3(0.98, 0.98, 0.98), clamp(slope, 0.0, 1.0));
        }
        default: {
            return vec3(0.0);
        }
    }
}

@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(vertex_output.position, vertex_output.visibility_range_dither);
#endif

    var pbr_input = pbr_input_from_standard_material(vertex_output, is_front);

    let sampled_base_color = pbr_input.material.base_color;
    let sampled_perceptual_roughness = pbr_input.material.perceptual_roughness;
    let sampled_reflectance = pbr_input.material.reflectance;
    let sampled_metallic = pbr_input.material.metallic;
    let sampled_flags = pbr_input.material.flags;
    let sampled_normal = pbr_input.N;

<<<<<<< HEAD
fn hash3(seed: u32, ix: i32, iy: i32, iz: i32) -> f32 {
    var v = seed;
    v ^= bitcast<u32>(ix) * 0x9E3779B9u;
    v ^= bitcast<u32>(iy) * 0xBB67AE85u;
    v ^= bitcast<u32>(iz) * 0xC2B2AE3Du;
    v ^= v >> 16u;
    v *= 0x7FEB352Du;
    v ^= v >> 15u;
    v *= 0x846CA68Bu;
    v ^= v >> 16u;
    return f32(v) / 4294967295.0;
}

fn decode_pair(value: f32) -> vec2<f32> {
    let bits = bitcast<u32>(value);
    let hi = f32(bits >> 16u) / 65535.0;
    let lo = f32(bits & 0xFFFFu) / 65535.0;
    return vec2(hi, lo);
}

struct NoiseSample {
    value: f32,
    grad: vec3<f32>,
}

struct ClimateFields {
    continent_value: f32,
    land_mask: f32,
    macro_relief: f32,
    mountain_seed: f32,
    erosion: f32,
    detail: f32,
    moisture_noise: f32,
}

struct ClimateDebugSample {
    continent_value: f32,
    land_mask: f32,
    height01: f32,
    moisture: f32,
    temperature: f32,
    slope: f32,
}

fn value3_with_derivative(seed: u32, coord: vec3<f32>) -> NoiseSample {
    let x0 = i32(floor(coord.x));
    let y0 = i32(floor(coord.y));
    let z0 = i32(floor(coord.z));
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let z1 = z0 + 1;

    let fx = coord.x - floor(coord.x);
    let fy = coord.y - floor(coord.y);
    let fz = coord.z - floor(coord.z);

    let tx = smooth3(fx);
    let ty = smooth3(fy);
    let tz = smooth3(fz);
    let dtx = smooth3_derivative(fx);
    let dty = smooth3_derivative(fy);
    let dtz = smooth3_derivative(fz);

    let c000 = hash3(seed, x0, y0, z0);
    let c100 = hash3(seed, x1, y0, z0);
    let c010 = hash3(seed, x0, y1, z0);
    let c110 = hash3(seed, x1, y1, z0);
    let c001 = hash3(seed, x0, y0, z1);
    let c101 = hash3(seed, x1, y0, z1);
    let c011 = hash3(seed, x0, y1, z1);
    let c111 = hash3(seed, x1, y1, z1);

    let x00 = mix(c000, c100, tx);
    let x10 = mix(c010, c110, tx);
    let x01 = mix(c001, c101, tx);
    let x11 = mix(c011, c111, tx);

    let y_mix0 = mix(x00, x10, ty);
    let y_mix1 = mix(x01, x11, ty);
    let value = mix(y_mix0, y_mix1, tz);

    let dx00 = (c100 - c000) * dtx;
    let dx10 = (c110 - c010) * dtx;
    let dx01 = (c101 - c001) * dtx;
    let dx11 = (c111 - c011) * dtx;
    let dy0_dx = mix(dx00, dx10, ty);
    let dy1_dx = mix(dx01, dx11, ty);
    let grad_x = mix(dy0_dx, dy1_dx, tz);

    let dy0 = (x10 - x00) * dty;
    let dy1 = (x11 - x01) * dty;
    let grad_y = mix(dy0, dy1, tz);

    let grad_z = (y_mix1 - y_mix0) * dtz;

    return NoiseSample(value, vec3(grad_x, grad_y, grad_z));
}

fn fbm3_with_derivative(seed: u32, x: f32, y: f32, z: f32, base_freq: f32) -> NoiseSample {
    var frequency = max(base_freq, 1e-4);
    var amplitude = 1.0;
    var total = 0.0;
    var grad = vec3(0.0);
    var norm = 0.0;

    for (var octave: u32 = 0u; octave < 5u; octave = octave + 1u) {
        let sample = value3_with_derivative(seed ^ octave, vec3(x, y, z) * frequency);
        total = total + sample.value * amplitude;
        grad = grad + sample.grad * (amplitude * frequency);
        norm = norm + amplitude;
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return NoiseSample(clamp(total / norm, 0.0, 1.0), grad / norm);
}

fn fbm3_value(seed: u32, coord: vec3<f32>, base_freq: f32) -> f32 {
    return fbm3_with_derivative(seed, coord.x, coord.y, coord.z, base_freq).value;
}

fn to_signed(value: f32) -> f32 {
    return value * 2.0 - 1.0;
}

fn climate_sample_fields(coord: vec3<f32>) -> ClimateFields {
    let base_f = max(material.base_freq, 1e-5);
    let detail_f = max(material.detail_freq, 0.35);

    let plate_a = to_signed(fbm3_value(
        material.seed,
        vec3(coord.x * 0.55, coord.y * 0.53, coord.z * 0.57),
        base_f * 0.55,
    ));
    let plate_b = to_signed(fbm3_value(
        material.seed ^ 0x11u,
        vec3(coord.z * 0.48, coord.x * 0.52, coord.y * 0.51),
        base_f * 0.45,
    ));
    let coast_noise = to_signed(fbm3_value(
        material.seed ^ 0x21u,
        vec3(coord.x * 1.2, coord.y * 1.1, coord.z * 1.15),
        base_f * 1.05,
    ));

    let continent_value = plate_a * 0.75 + plate_b * 0.45 + coast_noise * 0.28 - 0.05;
    let land_mask = smoothstep(-0.18, 0.22, continent_value);

    let macro_relief = pow(
        fbm3_value(
            material.seed ^ 0x33u,
            vec3(coord.x * 0.95, coord.y * 0.85, coord.z * 0.9),
            base_f * 0.9,
        ),
        1.6,
    );
    let mountain_seed = fbm3_value(
        material.seed ^ 0x43u,
        vec3(coord.y * 1.8, coord.z * 1.6, coord.x * 1.7),
        detail_f * 0.7,
    );
    let erosion = pow(
        fbm3_value(
            material.seed ^ 0x53u,
            vec3(coord.z * 1.4, coord.x * 1.3, coord.y * 1.35),
            detail_f * 0.8,
        ),
        1.8,
    );
    var detail = fbm3_value(
        material.seed ^ 0x63u,
        vec3(coord.x * 3.3, coord.y * 3.1, coord.z * 3.2),
        detail_f * 2.1,
    );
    let moisture_noise = fbm3_value(
        material.seed ^ 0x73u,
        vec3(coord.x * 1.05, coord.y * 1.02, coord.z * 1.03),
        base_f * 1.3,
    );

    let detail_hi = fbm3_value(
        material.seed ^ 0x93u,
        vec3(coord.x * 6.0, coord.y * 5.8, coord.z * 5.9),
        max(detail_f, 0.6) * 3.5,
    );
    detail = mix(detail, detail_hi, 0.35);

    return ClimateFields(
        continent_value,
        land_mask,
        macro_relief,
        mountain_seed,
        erosion,
        detail,
        moisture_noise,
    );
}

fn compose_height_from_fields(fields: ClimateFields) -> f32 {
    var height =
        fields.continent_value * 0.48 + (fields.land_mask - 0.5) * 0.65;
    height = height + (fields.macro_relief - 0.5) * 0.42;
    height = height + (fields.erosion - 0.5) * 0.22;
    height = height + pow(max(fields.mountain_seed, 1e-6), 0.9) * material.mountain_scale * 1.12;
    height = height + (fields.detail - 0.5) * 0.09;
    height = height + pow(fields.land_mask, 2.2) * 0.08;
    return clamp(height * 0.52 + 0.48, 0.0, 1.0);
}

fn climate_debug_evaluate(unit: vec3<f32>) -> ClimateDebugSample {
    let warp_f = max(material.warp_freq, 1e-5);
    let seed = material.seed;

    let warp_offset = vec3(
        fbm3_value(seed ^ 0xA1u, vec3(unit.x, unit.y, unit.z), warp_f) - 0.5,
        fbm3_value(seed ^ 0xB2u, vec3(unit.z, unit.x, unit.y), warp_f) - 0.5,
        fbm3_value(seed ^ 0xC3u, vec3(unit.y, unit.z, unit.x), warp_f) - 0.5,
    );
    let warped = unit + warp_offset * material.warp_amp;

    let center = climate_sample_fields(warped);
    let height01 = compose_height_from_fields(center);

    let eps = 0.012;
    let sample_px = climate_sample_fields(warped + vec3(eps, 0.0, 0.0));
    let sample_mx = climate_sample_fields(warped - vec3(eps, 0.0, 0.0));
    let sample_py = climate_sample_fields(warped + vec3(0.0, eps, 0.0));
    let sample_my = climate_sample_fields(warped - vec3(0.0, eps, 0.0));
    let sample_pz = climate_sample_fields(warped + vec3(0.0, 0.0, eps));
    let sample_mz = climate_sample_fields(warped - vec3(0.0, 0.0, eps));

    let hx1 = compose_height_from_fields(sample_px);
    let hx0 = compose_height_from_fields(sample_mx);
    let hy1 = compose_height_from_fields(sample_py);
    let hy0 = compose_height_from_fields(sample_my);
    let hz1 = compose_height_from_fields(sample_pz);
    let hz0 = compose_height_from_fields(sample_mz);

    let grad_x = (hx1 - hx0) / (2.0 * eps);
    let grad_y = (hy1 - hy0) / (2.0 * eps);
    let grad_z = (hz1 - hz0) / (2.0 * eps);

    let slope = min(
        clamp(sqrt(grad_x * grad_x + grad_y * grad_y + grad_z * grad_z), 0.0, 1.4),
        1.0,
    );

    let lat_abs = abs(unit.y);

    let land_grad_x = abs(sample_px.land_mask - sample_mx.land_mask);
    let land_grad_y = abs(sample_py.land_mask - sample_my.land_mask);
    let land_grad_z = abs(sample_pz.land_mask - sample_mz.land_mask);
    let coast_gradient = clamp(
        sqrt(
            (land_grad_x * land_grad_x + land_grad_y * land_grad_y + land_grad_z * land_grad_z)
                / 3.0,
        ),
        0.0,
        1.0,
    );

    let coast_band = clamp(center.land_mask * (1.0 - center.land_mask) * 4.0, 0.0, 1.0);
    let interior = pow(center.land_mask, 3.0);
    let dryness_distance =
        max(1.0 - clamp(coast_band + coast_gradient * 0.6, 0.0, 1.2), 0.0);
    let dryness_base =
        clamp(pow(interior, 1.1) * pow(dryness_distance, 1.2), 0.0, 1.0);
    let dryness_noise = (center.detail - 0.5) * 0.2;
    let dryness = clamp(
        dryness_base + material.dryness_bias_global + dryness_noise,
        0.0,
        1.0,
    );

    let rain_shadow = pow(center.mountain_seed * dryness_distance, 1.3);
    var moisture = clamp(
        center.moisture_noise * 0.55 + coast_band * 1.05 - interior * 0.35,
        0.0,
        1.0,
    );
    moisture = clamp(moisture - rain_shadow * 0.45, 0.0, 1.0);
    moisture = moisture + (1.0 - interior) * 0.04;
    moisture = clamp(moisture + material.moisture_bias_global, 0.0, 1.0);
    moisture = clamp(moisture - dryness * 0.25, 0.0, 1.0);

    let elev01 =
        clamp((height01 - material.sea_level) / max(1.0 - material.sea_level, 1e-3), 0.0, 1.0);
    let tilt_cooling = (material.axial_tilt * 1.15) * pow(lat_abs, 1.1);
    let base_temp = clamp(
        1.0 - pow(lat_abs, 1.45) - tilt_cooling + material.temp_shift,
        0.0,
        1.0,
    );
    let altitude_cooling = pow(elev01, 0.65) * 0.7 + pow(slope, 0.75) * 0.15;
    let temp_variation = (center.detail - 0.5) * 0.18;
    let moisture_cooling = moisture * 0.1;
    var temperature = clamp(
        base_temp + temp_variation - altitude_cooling - moisture_cooling + dryness * 0.05,
        0.0,
        1.0,
    );
    temperature = temperature - smoothstep(0.72, 0.98, lat_abs) * 0.18;
    temperature = clamp(temperature, 0.0, 1.0);

    return ClimateDebugSample(
        center.continent_value,
        center.land_mask,
        height01,
        moisture,
        temperature,
        slope,
    );
}

fn biome_color(temperature: f32, moisture: f32) -> vec3<f32> {
    let desert = vec3(0.86, 0.75, 0.46);
    let savanna = vec3(0.74, 0.66, 0.36);
    let shrubland = vec3(0.58, 0.63, 0.37);
    let grassland = vec3(0.38, 0.56, 0.28);
    let temperate_forest = vec3(0.26, 0.48, 0.26);
    let rainforest = vec3(0.16, 0.42, 0.23);
    let boreal = vec3(0.24, 0.4, 0.31);
    let tundra = vec3(0.72, 0.75, 0.74);

    if temperature < 0.2 {
        return mix(tundra, boreal, pow(moisture, 1.1));
    } else if temperature < 0.4 {
        let cool = mix(boreal, temperate_forest, pow(moisture, 0.9));
        return mix(cool, tundra, pow(1.0 - moisture, 1.4) * 0.35);
    } else if temperature < 0.6 {
        let dry = shrubland;
        let wet = mix(grassland, temperate_forest, 0.6);
        return mix(dry, wet, pow(moisture, 0.85));
    } else if temperature < 0.8 {
        let dry = savanna;
        let wet = rainforest;
        return mix(dry, wet, pow(moisture, 0.9));
    } else {
        let dry = desert;
        let wet = mix(savanna, rainforest, 0.5);
        return mix(dry, wet, pow(moisture, 1.1));
    }
}

fn debug_color(
    mode: u32,
    continent_value: f32,
    land_mask: f32,
    height01: f32,
    moisture: f32,
    temperature: f32,
    slope: f32,
) -> vec3<f32> {
    switch mode {
        case 1u: {
            let t = clamp((continent_value + 1.0) * 0.5, 0.0, 1.0);
            return mix(vec3(0.1, 0.25, 0.65), vec3(0.85, 0.72, 0.42), t);
        }
        case 2u: {
            return mix(vec3(0.08, 0.16, 0.45), vec3(0.92, 0.9, 0.45), clamp(land_mask, 0.0, 1.0));
        }
        case 3u: {
            return vec3(clamp(height01, 0.0, 1.0));
        }
        case 4u: {
            return mix(vec3(0.85, 0.7, 0.35), vec3(0.18, 0.48, 0.25), clamp(moisture, 0.0, 1.0));
        }
        case 5u: {
            return mix(vec3(0.15, 0.28, 0.68), vec3(0.95, 0.42, 0.12), clamp(temperature, 0.0, 1.0));
        }
        case 6u: {
            return mix(vec3(0.12, 0.12, 0.12), vec3(0.98, 0.98, 0.98), clamp(slope, 0.0, 1.0));
        }
        default: {
            return vec3(0.0);
        }
    }
}

@fragment
fn fragment(vertex_output: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
#ifdef VISIBILITY_RANGE_DITHER
    pbr_functions::visibility_range_dither(vertex_output.position, vertex_output.visibility_range_dither);
#endif

    var pbr_input = pbr_input_from_vertex_output(vertex_output, is_front, false);
    pbr_input.material.base_color = vec4(1.0, 1.0, 1.0, 1.0);
    pbr_input.material.flags = pbr_types::STANDARD_MATERIAL_FLAGS_ALPHA_MODE_OPAQUE;

    let biome_hint = vertex_output.uv;
    let biome_id_hint = clamp(biome_hint.x, 0.0, 1.0);
    let altitude_hint = clamp(biome_hint.y, 0.0, 1.0);
    let unit = normalize(vertex_output.world_position.xyz);

    if material.debug_mode != 0u {
        let debug_sample = climate_debug_evaluate(unit);
        let debug_rgb = debug_color(
            material.debug_mode,
            debug_sample.continent_value,
            debug_sample.land_mask,
            debug_sample.height01,
            debug_sample.moisture,
            debug_sample.temperature,
            debug_sample.slope,
        );
        var debug_out: FragmentOutput;
        debug_out.color = vec4(debug_rgb, 1.0);
        return debug_out;
    }

    let pack_md = decode_pair(vertex_output.uv_b.x);
    let pack_cs = decode_pair(vertex_output.uv_b.y);
    let pack_ht = decode_pair(vertex_output.color.x);
    let pack_sl = decode_pair(vertex_output.color.y);
    let pack_mr = decode_pair(vertex_output.color.z);
    let pack_lv = decode_pair(vertex_output.color.w);

    let moisture = clamp(pack_md.x, 0.0, 1.0);
    var dryness = clamp(pack_md.y + biome_id_hint * 1e-4, 0.0, 1.0);
    let coast_band = clamp(pack_cs.x, 0.0, 1.0);
    let snow_score = clamp(pack_cs.y, 0.0, 1.0);
    let height_hint =
        material.sea_level + altitude_hint * max(1.0 - material.sea_level, 1e-3);
    var height01 = clamp(pack_ht.x, 0.0, 1.0);
    height01 = mix(height_hint, height01, 0.999);
    let temperature = clamp(pack_ht.y, 0.0, 1.0);
    let slope = clamp(pack_sl.x, 0.0, 1.0);
    let continent_value = clamp(pack_sl.y, 0.0, 1.0) * 2.0 - 1.0;
    let mountain_mask = clamp(pack_mr.x, 0.0, 1.0);
    let ridge_light = clamp(pack_mr.y, 0.0, 1.0);
    let depth_or_valley = clamp(pack_lv.x, 0.0, 1.0);
    let land_mask = clamp(pack_lv.y, 0.0, 1.0);
    let lat_abs = abs(unit.y);

    let detail = fbm3_with_derivative(
        material.seed ^ 0xE1u,
        unit.x * 8.4,
        unit.y * 8.1,
        unit.z * 8.3,
        material.detail_freq * 2.4,
    );
    var micro = detail.value * 2.0 - 1.0;

    let is_water = height01 < material.sea_level;
    let depth = select(
        clamp((material.sea_level - height01) / max(material.sea_level, 1e-3), 0.0, 1.0),
        depth_or_valley,
        is_water,
    );
    let elev01 = clamp(
        (height01 - material.sea_level) / max(1.0 - material.sea_level, 1e-3),
        0.0,
        1.0,
    );
    let shore_mix = pow(max(1.0 - depth, 0.0), 0.6);

    var normal = normalize(pbr_input.N);
    if (is_water) {
        micro = 0.0;
    }

    var sun_dir = material.sun_dir.xyz;
    let sun_dir_len = length(sun_dir);
    if sun_dir_len > 1e-4 {
        sun_dir = sun_dir / sun_dir_len;
    } else {
        sun_dir = normalize(vec3(0.32, 0.78, 0.54));
    }
    let night_tint = vec3(0.08, 0.09, 0.12);
    let dawn_tint = vec3(0.14, 0.15, 0.18);
    let sun_ndotl = clamp(dot(normal, sun_dir), 0.0, 1.0);
    let hemi = 0.45 + 0.55 * max(normal.y, 0.0);
    let night_mix = pow(1.0 - sun_ndotl, 1.8);
    let ambient_tint = mix(night_tint, dawn_tint, hemi);
    let rim = pow(1.0 - clamp(dot(normal, unit), 0.0, 1.0), 1.9);

    var albedo = vec3(0.0);
    if (is_water) {
        let deep_color = vec3(0.08, 0.12, 0.2);
        let shallow_color = vec3(0.23, 0.34, 0.48);
        let shelf_mix = smoothstep(0.0, 0.5, depth);
        let water_color = mix(shallow_color, deep_color, shelf_mix);
=======
    let has_base_color_texture =
        (sampled_flags & pbr_types::STANDARD_MATERIAL_FLAGS_BASE_COLOR_TEXTURE_BIT) != 0u;
    let has_metallic_roughness_texture =
        (sampled_flags & pbr_types::STANDARD_MATERIAL_FLAGS_METALLIC_ROUGHNESS_TEXTURE_BIT) != 0u;

    let unit = decode_unit_octa(vertex_output.uv);

    if material.debug_mode != 0u {
        let debug_sample = climate_debug_evaluate(unit);
        let debug_rgb = debug_color(
            material.debug_mode,
            debug_sample.continent_value,
            debug_sample.land_mask,
            debug_sample.height01,
            debug_sample.moisture,
            debug_sample.temperature,
            debug_sample.slope,
        );
        var debug_out: FragmentOutput;
        debug_out.color = vec4(debug_rgb, 1.0);
        return debug_out;
    }

    let pack_md = decode_pair(vertex_output.uv_b.x);
    let pack_cs = decode_pair(vertex_output.uv_b.y);
    let pack_ht = decode_pair(vertex_output.color.x);
    let pack_sl = decode_pair(vertex_output.color.y);
    let pack_mr = decode_pair(vertex_output.color.z);
    let pack_lv_bits = bitcast<u32>(vertex_output.color.w);
    let depth_bits = pack_lv_bits >> 16u;
    let land_bits_raw = pack_lv_bits & 0xFFFFu;
    let is_skirt = (land_bits_raw & SKIRT_FLAG_BIT) != 0u;
    let land_bits = land_bits_raw & SKIRT_LAND_MASK_BITS;

    let moisture = clamp(pack_md.x, 0.0, 1.0);
    let dryness = clamp(pack_md.y, 0.0, 1.0);
    let coast_band = clamp(pack_cs.x, 0.0, 1.0);
    let snow_score = clamp(pack_cs.y, 0.0, 1.0);
    let height01 = clamp(pack_ht.x, 0.0, 1.0);
    let temperature = clamp(pack_ht.y, 0.0, 1.0);
    let slope = clamp(pack_sl.x, 0.0, 1.0);
    let continent_value = clamp(pack_sl.y, 0.0, 1.0) * 2.0 - 1.0;
    let mountain_mask = clamp(pack_mr.x, 0.0, 1.0);
    let ridge_light = clamp(pack_mr.y, 0.0, 1.0);
    let depth_or_valley = clamp(f32(depth_bits) / 65535.0, 0.0, 1.0);
    let land_mask = clamp(f32(land_bits) / SKIRT_LAND_MASK_SCALE, 0.0, 1.0);
    var skirt_factor = 0.0;
    if (is_skirt) {
        skirt_factor = 1.0;
    }
    let lat_abs = abs(unit.y);

    let warp_field = fbm3_with_derivative(
        material.seed ^ 0xABu,
        unit.x * 4.1,
        unit.y * 4.4,
        unit.z * 4.3,
        material.detail_freq * 1.8,
    );
    let warped_unit = normalize(unit + warp_field.grad * (0.06 * material.detail_freq));

    let basis0 = warped_unit;
    let basis1 = vec3(warped_unit.y, warped_unit.z, warped_unit.x);
    let basis2 = vec3(warped_unit.z, warped_unit.x, warped_unit.y);

    let coarse0 =
        fbm3_with_derivative(
            material.seed ^ 0xE1u,
            basis0.x * 8.4,
            basis0.y * 8.1,
            basis0.z * 8.3,
            material.detail_freq * 2.4,
        )
            .value;
    let coarse1 =
        fbm3_with_derivative(
            material.seed ^ 0x9Du,
            basis1.x * 8.4,
            basis1.y * 8.1,
            basis1.z * 8.3,
            material.detail_freq * 2.4,
        )
            .value;
    let coarse2 =
        fbm3_with_derivative(
            material.seed ^ 0xA7u,
            basis2.x * 8.4,
            basis2.y * 8.1,
            basis2.z * 8.3,
            material.detail_freq * 2.4,
        )
            .value;
    let coarse_noise = ((coarse0 + coarse1 + coarse2) / 3.0) * 2.0 - 1.0;

    let fine0 =
        fbm3_with_derivative(
            material.seed ^ 0xC5u,
            basis0.y * 13.6,
            basis0.z * 13.2,
            basis0.x * 13.4,
            material.detail_freq * 5.8,
        )
            .value;
    let fine1 =
        fbm3_with_derivative(
            material.seed ^ 0xB1u,
            basis1.y * 13.6,
            basis1.z * 13.2,
            basis1.x * 13.4,
            material.detail_freq * 5.8,
        )
            .value;
    let fine2 =
        fbm3_with_derivative(
            material.seed ^ 0xD3u,
            basis2.y * 13.6,
            basis2.z * 13.2,
            basis2.x * 13.4,
            material.detail_freq * 5.8,
        )
            .value;
    let fine_noise = ((fine0 + fine1 + fine2) / 3.0) * 2.0 - 1.0;

    var micro = coarse_noise * 0.5 + fine_noise * 0.5;

    let is_water = height01 < material.sea_level;
    let depth = select(
        clamp((material.sea_level - height01) / max(material.sea_level, 1e-3), 0.0, 1.0),
        depth_or_valley,
        is_water,
    );
    let elev01 = clamp(
        (height01 - material.sea_level) / max(1.0 - material.sea_level, 1e-3),
        0.0,
        1.0,
    );
    let shore_mix = pow(max(1.0 - depth, 0.0), 0.6);

    var normal = normalize(pbr_input.N);
    if (is_water) {
        micro = 0.0;
    }

    var sun_dir = material.sun_dir.xyz;
    let sun_dir_len = length(sun_dir);
    if sun_dir_len > 1e-4 {
        sun_dir = sun_dir / sun_dir_len;
    } else {
        sun_dir = normalize(vec3(0.32, 0.78, 0.54));
    }
    let night_tint = vec3(0.08, 0.09, 0.12);
    let dawn_tint = vec3(0.14, 0.15, 0.18);
    let sun_ndotl = clamp(dot(normal, sun_dir), 0.0, 1.0);
    let hemi = 0.45 + 0.55 * max(normal.y, 0.0);
    let night_mix = pow(1.0 - sun_ndotl, 1.8);
    let ambient_tint = mix(night_tint, dawn_tint, hemi);
    let rim = pow(1.0 - clamp(dot(normal, unit), 0.0, 1.0), 1.9);

    var albedo = vec3(0.0);
    if (is_water) {
        let deep_color = vec3(0.08, 0.12, 0.2);
        let shallow_color = vec3(0.23, 0.34, 0.48);
        let shelf_mix = smoothstep(0.0, 0.5, depth);
        let water_color = mix(shallow_color, deep_color, shelf_mix);
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
        albedo = clamp(water_color * 0.64 + ambient_tint * 0.06, vec3(0.0), vec3(1.0));
        if (!has_metallic_roughness_texture) {
            pbr_input.material.perceptual_roughness = 0.22;
            pbr_input.material.reflectance = vec3(0.08);
        }
    } else {
        var color = biome_color(temperature, moisture);

        let slope_to_rock = pow(slope, 0.8);
        let rock_span = max(material.snow_start - material.rock_start, 1e-3);
        let rock_k = clamp((elev01 - material.rock_start) / rock_span, 0.0, 1.0);
        let to_rock = clamp(rock_k * 0.75 + slope_to_rock * 0.55, 0.0, 1.0);
        color = mix(color, material.land_rock.xyz, to_rock);

        let lush_factor = pow(moisture * (1.0 - dryness), 1.4);
        let grass_variation =
            fbm3_with_derivative(
                material.seed ^ 0xD7u,
                unit.z * 4.8,
                unit.x * 4.6,
                unit.y * 5.0,
                material.base_freq * 3.2,
            ).value;
        let grass_alt =
            mix(material.land_grass.xyz, vec3(0.24, 0.5, 0.28), clamp(grass_variation, 0.0, 1.0));
        color = mix(color, grass_alt, lush_factor * 0.22);

        let desert_bleach = pow(dryness, 1.25) * (1.0 - moisture * 0.6);
        color = mix(color, vec3(0.92, 0.82, 0.6), desert_bleach * 0.35);

        let coast_soft = clamp(coast_band * 1.15, 0.0, 1.0);
        let sand_mix = clamp(smoothstep(0.0, 0.25, elev01), 0.0, 1.0);
        let coast_color = mix(material.land_sand.xyz, color, sand_mix);
        color = mix(coast_color, color, clamp(coast_soft * 0.55, 0.0, 1.0));
        color = clamp(color + vec3(micro) * 0.032, vec3(0.0), vec3(1.0));

        let detail_amp = material.surface_detail_amp;
        if detail_amp > 1e-4 {
            let detail_scale = max(material.surface_detail_scale, 0.01);
            let surface_detail = fbm3_with_derivative(
                material.seed ^ 0xF5u,
                unit.x * detail_scale,
                unit.y * detail_scale,
                unit.z * detail_scale,
                detail_scale,
            );
            let detail_value = surface_detail.value * 2.0 - 1.0;
            color = clamp(color + vec3(detail_value) * (detail_amp * 0.38), vec3(0.0), vec3(1.0));
            normal = normalize(normal + surface_detail.grad * (detail_amp * 0.32));
        }

        let cloud_seed = smoothstep(0.65, 1.0, snow_score) * 0.6
            + smoothstep(0.58, 0.92, moisture) * (1.0 - dryness) * 0.4;
        let cloud_intensity = clamp(cloud_seed, 0.0, 1.0);
        color = clamp(color + vec3(cloud_intensity) * 0.07, vec3(0.0), vec3(1.0));

        let mountain_highlight = pow(mountain_mask, 1.4);
        color = mix(color, color + vec3(0.18), mountain_highlight * 0.35);

        let shade_base = clamp(
            0.62 + hemi * 0.3 + sun_ndotl * 0.42 + ridge_light * 0.5
                + micro * 0.22
                - slope * 0.08,
            0.35,
            1.6,
        );
        let shade = mix(1.0, shade_base, 1.0 - coast_soft);
        let mountain_shade = mix(shade, max(shade, 1.05), mountain_highlight * 0.5);
        color = color * mountain_shade + coast_soft * 0.03;
        let spec =
            pow(sun_ndotl, 18.0) * pow(1.0 - slope, 1.5) * 0.16 * (1.0 - coast_soft) * (1.0 + mountain_highlight * 0.5);
        color = color + vec3(spec);
        let ambient_blend = mix(0.12, 0.26, 1.0 - coast_soft);
        color =
            color + ambient_tint * (ambient_blend * night_mix + rim * 0.25 * (1.0 - coast_soft));
        color = clamp(color, vec3(0.0), vec3(1.0));
        albedo = clamp(color, vec3(0.0), vec3(1.0));
        if (!has_metallic_roughness_texture) {
            var roughness = pbr_input.material.perceptual_roughness;
            roughness = roughness + (0.38 - roughness) * (mountain_highlight * 0.6);
            pbr_input.material.perceptual_roughness = clamp(roughness, 0.0, 1.0);

<<<<<<< HEAD
        let lush_factor = pow(moisture * (1.0 - dryness), 1.4);
        let grass_variation =
            fbm3_with_derivative(
                material.seed ^ 0xD7u,
                unit.z * 4.8,
                unit.x * 4.6,
                unit.y * 5.0,
                material.base_freq * 3.2,
            ).value;
        let grass_alt =
            mix(material.land_grass.xyz, vec3(0.24, 0.5, 0.28), clamp(grass_variation, 0.0, 1.0));
        color = mix(color, grass_alt, lush_factor * 0.22);

        let desert_bleach = pow(dryness, 1.25) * (1.0 - moisture * 0.6);
        color = mix(color, vec3(0.92, 0.82, 0.6), desert_bleach * 0.35);

        let coast_soft = clamp(coast_band * 1.15, 0.0, 1.0);
        let sand_start = clamp(material.sand_height * 0.35, 0.0, material.sand_height);
        let sand_end = max(material.sand_height, sand_start + 1e-3);
        let sand_mix = clamp(smoothstep(sand_start, sand_end, elev01), 0.0, 1.0);
        let coast_color = mix(material.land_sand.xyz, color, sand_mix);
        color = mix(coast_color, color, clamp(coast_soft * 0.55, 0.0, 1.0));
        color = clamp(color + vec3(micro) * 0.025, vec3(0.0), vec3(1.0));

        let cloud_seed = smoothstep(0.65, 1.0, snow_score) * 0.6
            + smoothstep(0.58, 0.92, moisture) * (1.0 - dryness) * 0.4;
        let cloud_intensity = clamp(cloud_seed, 0.0, 1.0);
        color = clamp(color + vec3(cloud_intensity) * 0.07, vec3(0.0), vec3(1.0));

        let mountain_highlight = pow(mountain_mask, 1.4);
        color = mix(color, color + vec3(0.18), mountain_highlight * 0.35);
        let snow_mix = smoothstep(material.snow_start - 0.04, material.snow_start + 0.06, elev01);
        color = mix(color, material.land_snow.xyz, snow_mix);

        let shade_base = clamp(
            0.62 + hemi * 0.3 + sun_ndotl * 0.42 + ridge_light * 0.5
                + micro * 0.18
                - slope * 0.08,
            0.35,
            1.6,
        );
        let shade = mix(1.0, shade_base, 1.0 - coast_soft);
        let mountain_shade = mix(shade, max(shade, 1.05), mountain_highlight * 0.5);
        color = color * mountain_shade + coast_soft * 0.03;
        let spec =
            pow(sun_ndotl, 18.0) * pow(1.0 - slope, 1.5) * 0.16 * (1.0 - coast_soft) * (1.0 + mountain_highlight * 0.5);
        color = color + vec3(spec);
        let ambient_blend = mix(0.12, 0.26, 1.0 - coast_soft);
        color =
            color + ambient_tint * (ambient_blend * night_mix + rim * 0.25 * (1.0 - coast_soft));
        color = clamp(color, vec3(0.0), vec3(1.0));
        albedo = clamp(color, vec3(0.0), vec3(1.0));
        var roughness = pbr_input.material.perceptual_roughness;
        roughness = roughness + (0.38 - roughness) * (mountain_highlight * 0.6);
        pbr_input.material.perceptual_roughness = clamp(roughness, 0.0, 1.0);

        var reflectance = pbr_input.material.reflectance;
        reflectance = reflectance + (vec3(0.06) - reflectance) * (mountain_highlight * 0.5);
        pbr_input.material.reflectance = clamp(reflectance, vec3(0.0), vec3(1.0));
=======
            var reflectance = pbr_input.material.reflectance;
            reflectance =
                reflectance + (vec3(0.06) - reflectance) * (mountain_highlight * 0.5);
            pbr_input.material.reflectance = clamp(reflectance, vec3(0.0), vec3(1.0));
        }
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
    }

    var final_color = albedo;
    if (has_base_color_texture) {
        final_color = clamp(sampled_base_color.rgb, vec3(0.0), vec3(1.0));
    }

    let limb_weight = clamp(rim * 0.85 + 0.1, 0.0, 1.0);
    let normal_mix = min(1.0, skirt_factor * (0.82 + limb_weight * 0.18));
    normal = normalize(mix(normal, unit, normal_mix));
    if (skirt_factor > 0.0) {
        let skirt_mix = clamp(skirt_factor * (0.55 + rim * 0.45), 0.0, 1.0);
        let base_surface = mix(material.water_shallow.xyz, material.land_grass.xyz, land_mask);
        let coastal_surface =
            mix(material.land_sand.xyz, base_surface, clamp(land_mask + shore_mix * 0.5, 0.0, 1.0));
        let tonal_target = mix(coastal_surface, ambient_tint, 0.42);
        final_color = mix(final_color, tonal_target, skirt_mix);
        if (!has_metallic_roughness_texture) {
            pbr_input.material.perceptual_roughness =
                mix(pbr_input.material.perceptual_roughness, mix(0.88, 0.97, rim), skirt_mix);
            pbr_input.material.reflectance =
                mix(pbr_input.material.reflectance, vec3(0.015), skirt_mix);
            pbr_input.material.metallic = mix(pbr_input.material.metallic, 0.0, skirt_mix);
        }
        if (material.sun_dir.w > 0.5) {
            final_color = mix(final_color, vec3(0.95, 0.28, 0.78), 0.8);
        }
    }

    if (has_metallic_roughness_texture) {
        pbr_input.material.perceptual_roughness = sampled_perceptual_roughness;
        pbr_input.material.reflectance = sampled_reflectance;
        pbr_input.material.metallic = sampled_metallic;
    }

    pbr_input.material.base_color = vec4(final_color, sampled_base_color.a);
    var final_normal = normal;
#ifdef STANDARD_MATERIAL_NORMAL_MAP
    final_normal = normalize(mix(normal, sampled_normal, 0.7));
#endif
    pbr_input.N = final_normal;
    pbr_input.world_normal = final_normal;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
