#import bevy_pbr::{
    forward_io::{FragmentOutput, VertexOutput},
    mesh_bindings::mesh,
    mesh_view_bindings::view,
    pbr_fragment::pbr_input_from_vertex_output,
    pbr_functions,
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    pbr_types,
}

struct PlanetSurfaceUniform {
    seed: u32,
    debug_mode: u32,
    _pad0: u32,
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
    water_deep: vec4<f32>,
    water_shallow: vec4<f32>,
    land_sand: vec4<f32>,
    land_grass: vec4<f32>,
    land_rock: vec4<f32>,
    land_snow: vec4<f32>,
};

@group(2) @binding(0)
var<uniform> material: PlanetSurfaceUniform;

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

    let unit = decode_unit_octa(vertex_output.uv);
    let pack_md = decode_pair(vertex_output.uv_b.x);
    let pack_cs = decode_pair(vertex_output.uv_b.y);
    let pack_ht = decode_pair(vertex_output.color.x);
    let pack_sl = decode_pair(vertex_output.color.y);
    let pack_mr = decode_pair(vertex_output.color.z);
    let pack_lv = decode_pair(vertex_output.color.w);

    let moisture = clamp(pack_md.x, 0.0, 1.0);
    let dryness = clamp(pack_md.y, 0.0, 1.0);
    let coast_band = clamp(pack_cs.x, 0.0, 1.0);
    let snow_score = clamp(pack_cs.y, 0.0, 1.0);
    let height01 = clamp(pack_ht.x, 0.0, 1.0);
    let temperature = clamp(pack_ht.y, 0.0, 1.0);
    let slope = clamp(pack_sl.x, 0.0, 1.0);
    let continent_value = clamp(pack_sl.y, 0.0, 1.0) * 2.0 - 1.0;
    let micro_relief = clamp(pack_mr.x, 0.0, 1.0) * 2.0 - 1.0;
    let ridge_light = clamp(pack_mr.y, 0.0, 1.0);
    let valley_shadow = clamp(pack_lv.x, 0.0, 1.0);
    let land_mask = clamp(pack_lv.y, 0.0, 1.0);
    let lat_abs = abs(unit.y);

    let detail = fbm3_with_derivative(
        material.seed ^ 0xE1u,
        unit.x * 8.4,
        unit.y * 8.1,
        unit.z * 8.3,
        material.detail_freq * 2.4,
    );
    let micro = detail.value * 2.0 - 1.0;

    let depth = clamp((material.sea_level - height01) / max(material.sea_level, 1e-3), 0.0, 1.0);
    let elev01 = clamp((height01 - material.sea_level) / max(1.0 - material.sea_level, 1e-3), 0.0, 1.0);
    let shore_mix = pow(max(1.0 - depth, 0.0), 0.6);
    let polar_cap = smoothstep(0.88, 1.0, lat_abs);

    if material.debug_mode != 0u {
        let debug_rgb = debug_color(
            material.debug_mode,
            continent_value,
            land_mask,
            height01,
            moisture,
            temperature,
            slope,
        );
        var debug_out: FragmentOutput;
        debug_out.color = vec4(debug_rgb, 1.0);
        return debug_out;
    }

    var normal = normalize(pbr_input.N);
    var grad = detail.grad * (material.normal_strength * 0.9);
    grad = grad - normal * dot(normal, grad);
    normal = normalize(normal - grad);

    let sun_dir = normalize(vec3(0.32, 0.78, 0.54));
    let night_tint = vec3(0.08, 0.09, 0.12);
    let dawn_tint = vec3(0.14, 0.15, 0.18);
    let sun_ndotl = clamp(dot(normal, sun_dir), 0.0, 1.0);
    let hemi = 0.45 + 0.55 * max(normal.y, 0.0);
    let night_mix = pow(1.0 - sun_ndotl, 1.8);
    let ambient_tint = mix(night_tint, dawn_tint, hemi);
    let rim = pow(1.0 - clamp(dot(normal, unit), 0.0, 1.0), 1.9);

    var albedo = vec3(0.0);
    if height01 < material.sea_level {
        var water = mix(material.water_deep.xyz, material.water_shallow.xyz, pow(shore_mix, 0.75));
        let depth_strength = pow(depth, 0.65);
        let turbidity = clamp(moisture * 0.45 + coast_band * 0.65, 0.0, 1.0);
        water = mix(water, vec3(0.62, 0.76, 0.88), turbidity * 0.25);
        let fresnel = pow(1.0 - abs(dot(normal, unit)), 2.2);
        let foam = smoothstep(0.72, 1.0, shore_mix);
        let sun_glint = pow(sun_ndotl, 14.0) * (0.15 + 0.25 * (1.0 - depth_strength));
        water = water + vec3(foam * 0.06 + fresnel * 0.08 + sun_glint);
        let light_factor = clamp(0.48 + hemi * 0.28 + sun_ndotl * 0.32, 0.35, 1.3);
        water = water * light_factor + ambient_tint * (0.35 * night_mix + rim * 0.18);
        albedo = clamp(water, vec3(0.0), vec3(1.0));
    } else {
        var color = biome_color(temperature, moisture);

        let beach_mix = clamp(
            smoothstep(0.0, max(material.coast_width, 1e-3), elev01) * 0.65 + moisture * 0.25,
            0.0,
            1.0,
        );
        color = mix(material.land_sand.xyz, color, beach_mix);

        let coast_emphasis = clamp(pow(shore_mix, 1.3) + coast_band, 0.0, 1.0);
        color = mix(color, vec3(0.95, 0.88, 0.7), coast_emphasis * 0.35);

        let slope_to_rock = pow(slope, 0.8);
        let rock_span = max(material.snow_start - material.rock_start, 1e-3);
        let rock_k = clamp((elev01 - material.rock_start) / rock_span, 0.0, 1.0);
        let to_rock = clamp(rock_k * 0.75 + slope_to_rock * 0.55, 0.0, 1.0);
        color = mix(color, material.land_rock.xyz, to_rock);

        let high_altitude = smoothstep(material.rock_start + 0.12, 0.98, elev01);
        let cold_factor = clamp((0.26 - temperature) / 0.26, 0.0, 1.0);
        let moisture_factor = clamp((moisture - 0.55) / 0.45, 0.0, 1.0);
        let allow_alpine = (high_altitude > 0.6) && (cold_factor > 0.6) && (moisture_factor > 0.4);

        if snow_score > 0.62 && (lat_abs > 0.88 || allow_alpine) {
            let snow_k = clamp((snow_score - 0.62) / 0.38, 0.0, 1.0);
            let snow_variation =
                fbm3_with_derivative(
                    material.seed ^ 0xC5u,
                    unit.x * 6.2,
                    unit.y * 6.0,
                    unit.z * 6.3,
                    material.detail_freq * 1.7,
                ).value;
            let cold_bleach =
                mix(vec3(0.93, 0.96, 1.0), vec3(0.82, 0.86, 0.91), snow_variation);
            let polar_snow = mix(
                material.land_snow.xyz,
                cold_bleach,
                polar_cap * 0.6 + snow_variation * 0.4,
            );
            color = mix(color, polar_snow, snow_k);
        }

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

        let shade = clamp(
            0.62 + hemi * 0.3 + sun_ndotl * 0.42 + ridge_light * 0.5
                - valley_shadow * 0.35
                + micro_relief * 0.25
                - slope * 0.08,
            0.35,
            1.6,
        );
        color = color * shade;
        let spec = pow(sun_ndotl, 18.0) * pow(1.0 - slope, 1.5) * 0.18;
        color = color + vec3(spec);
        color = color + ambient_tint * (0.3 * night_mix + rim * 0.25);
        color = clamp(color, vec3(0.0), vec3(1.0));
        albedo = clamp(color, vec3(0.0), vec3(1.0));
    }

    pbr_input.material.base_color = vec4(albedo, 1.0);
    pbr_input.N = normal;
    pbr_input.world_normal = normal;

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
