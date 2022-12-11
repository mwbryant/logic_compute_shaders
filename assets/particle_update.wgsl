struct Particles {
    position: vec2<i32>,
}
struct Spawner {
    time: f32,
}

@group(0) @binding(0)
var<storage, read_write> particles: array<Particles>;
//@group(0) @binding(1)
//var<uniform> spawner: Spawner;

fn hash(value: u32) -> u32 {
    var state = value;
    state = state ^ 2747636419u;
    state = state * 2654435769u;
    state = state ^ state >> 16u;
    state = state * 2654435769u;
    state = state ^ state >> 16u;
    state = state * 2654435769u;
    return state;
}

fn randomFloat(value: u32) -> f32 {
    return f32(hash(value)) / 4294967295.0;
}
fn hash2(value: u32) -> u32 {
    var state = value;
    state = state ^ 3104587013u;
    state = state * 1654435769u;
    state = state ^ state >> 16u;
    state = state * 2301324115u;
    state = state ^ state >> 16u;
    state = state * 2351435769u;
    return state;
}

fn randomFloat2(value: u32) -> f32 {
    return f32(hash2(value)) / 4294967295.0;
}

fn id(invocation_id: vec3<u32>, num_workgroups: vec3<u32>) -> u32{
    return invocation_id.y*u32(32)+ invocation_id.x;
}

// TODO get from const in code
@compute @workgroup_size(16, 1, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let location = vec2<i32>(i32(640.0*randomFloat(u32(invocation_id.x))), i32(480.0*randomFloat2(u32(invocation_id.x))));

    let id = id(invocation_id, num_workgroups);
    particles[id] = Particles(
         location
    );
}


@compute @workgroup_size(16, 1, 1)
fn update(@builtin(global_invocation_id) invocation_id: vec3<u32>,@builtin(num_workgroups) num_workgroups: vec3<u32>) {
    let id = id(invocation_id, num_workgroups);
    particles[id] = Particles(
         vec2<i32>(particles[id].position.x, particles[id].position.y + 1)
    );
}