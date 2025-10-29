#import bevy_pbr::{
    forward_io::FragmentOutput,
    mesh_bindings::mesh,
    mesh_functions::{
        get_world_from_local, mesh_normal_local_to_world, mesh_position_local_to_clip,
        mesh_position_local_to_world,
    },
    mesh_view_bindings::view,
}

struct AtmosphereVertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct AtmosphereVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
}

struct AtmosphereParams {
    color: vec4<f32>,
    intensity: f32,
    falloff: f32,
    _pad: f32,
}

@group(3) @binding(31)
var<uniform> atmosphere: AtmosphereParams;

@vertex
fn vertex(input: AtmosphereVertexInput, @builtin(instance_index) instance_index: u32) -> AtmosphereVertexOutput {
    let world_from_local = get_world_from_local(instance_index);
    let world_position = mesh_position_local_to_world(world_from_local, vec4(input.position, 1.0)).xyz;
    let world_normal = normalize(mesh_normal_local_to_world(input.normal, instance_index));

    var out: AtmosphereVertexOutput;
    out.clip_position = mesh_position_local_to_clip(world_from_local, vec4(input.position, 1.0));
    out.world_position = world_position;
    out.world_normal = world_normal;
    return out;
}

@fragment
fn fragment(input: AtmosphereVertexOutput) -> FragmentOutput {
    let view_dir = normalize(view.world_position.xyz - input.world_position);
    let rim = clamp(1.0 - max(dot(view_dir, input.world_normal), 0.0), 0.0, 1.0);
    let fresnel = pow(rim, atmosphere.falloff) * atmosphere.intensity;
    let color = atmosphere.color.rgb * fresnel;
    let alpha = atmosphere.color.a * fresnel;

    var out: FragmentOutput;
    out.color = vec4(color, alpha);
    return out;
}
