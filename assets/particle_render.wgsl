//XXX this is double booked, how to share between shaders
struct Particles {
    position: vec2<i32>,
}

@group(0) @binding(0)
var<storage, read_write> particles: array<Particles>;
@group(0) @binding(1)
var texture: texture_storage_2d<rgba8unorm, read_write>;

fn id(invocation_id: vec3<u32>, num_workgroups: vec3<u32>) -> u32{
    return invocation_id.y*u32(32)+ invocation_id.x;
}

// TODO get from const in code
@compute @workgroup_size(16, 16, 1)
fn clear(@builtin(global_invocation_id) invocation_id: vec3<u32>,@builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let location = vec2<i32>(i32(invocation_id.x), i32(invocation_id.y));
    textureStore(texture, location, vec4<f32>(0.0,0.0,0.0,0.0));
}

// TODO get from const in code
@compute @workgroup_size(16, 1, 1)
fn render(@builtin(global_invocation_id) invocation_id: vec3<u32>,@builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let id = id(invocation_id, num_workgroups);

    let location = particles[id].position;
    textureStore(texture, location, vec4<f32>(1.0,0.0,0.0,1.0));

    //Bounds check!
    let location = particles[id].position + vec2<i32>(0,1);
    textureStore(texture, location, vec4<f32>(0.0,0.0,1.0,1.0));
    let location = particles[id].position + vec2<i32>(0,-1);
    textureStore(texture, location, vec4<f32>(0.0,0.0,1.0,1.0));

    let location = particles[id].position + vec2<i32>(1,0);
    textureStore(texture, location, vec4<f32>(1.0,0.0,1.0,1.0));
    let location = particles[id].position + vec2<i32>(-1,0);
    textureStore(texture, location, vec4<f32>(1.0,0.0,1.0,1.0));
}