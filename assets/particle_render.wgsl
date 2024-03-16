#import "particle.wgsl"::{Particles,ParticleConfig}
#import "utils.wgsl"::{hslToRgb}


@group(0) @binding(0)
var<storage, read_write> particles: array<Particles>;

@group(0) @binding(1)
var<uniform> particle_config: ParticleConfig;

@group(0) @binding(2)
var texture: texture_storage_2d<rgba8unorm, read_write>;

const WORKGROUP_SIZE: u32 = #{WORKGROUP_SIZE};

fn id(invocation_id: vec3<u32>, num_workgroups: vec3<u32>) -> u32 {
    return invocation_id.y * u32(32) + invocation_id.x;
}

@compute @workgroup_size(WORKGROUP_SIZE, WORKGROUP_SIZE, 1)
fn clear(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let location = vec2<i32>(i32(invocation_id.x), i32(invocation_id.y));
    textureStore(texture, location, vec4<f32>(0.0, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(WORKGROUP_SIZE, 1, 1)
fn render(
    @builtin(global_invocation_id) invocation_id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>
) {
    let id = id(invocation_id, num_workgroups);

    var location = vec2<i32>(particles[id].position);

    let hue: f32 = 360.0 * (f32(particles[id].particle_type) / f32(particle_config.m));
    let saturation: f32 = 1.0;
    let lightness: f32 = 0.5;
    let color: vec3<f32> = hslToRgb(hue, saturation, lightness);

    textureStore(texture, location, vec4<f32>(color, 1.0));
}
