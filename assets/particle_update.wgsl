#import "particle.wgsl"::{Particles,ParticleConfig}
#import "utils.wgsl"::{force, getCell2D, keyFromHash, hashCell2D, offsets_2d}

@group(0) @binding(0)
var<storage, read_write> particles: array<Particles>;

@group(0) @binding(1)
var<uniform> particle_config: ParticleConfig;

@group(0) @binding(2)
var<storage, read> attraction_matrix: array<f32>;

@group(0) @binding(3)
var<uniform> delta_time: f32;

@group(0) @binding(4)
var<storage, read_write> spatial_indices: array<vec3<u32>>;

@group(0) @binding(5)
var<storage, read_write> spatial_offsets: array<u32>;

const WIDTH: f32 = #{WIDTH}.0;
const HEIGHT: f32 = #{HEIGHT}.0;
const WORKGROUP_SIZE: u32 = #{WORKGROUP_SIZE};

const grid_cell_size = 10;
const table_size = 10;

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

fn randomFloat3(value: u32) -> f32 {
    return randomFloat(value) * 2.0 - 1.0;
}

fn randomFloat4(value: u32) -> f32 {
    return randomFloat2(value) * 2.0 - 1.0;
}

fn id(invocation_id: vec3<u32>, num_workgroups: vec3<u32>) -> u32 {
    return invocation_id.y * u32(32) + invocation_id.x;
}

@compute @workgroup_size(WORKGROUP_SIZE, 1, 1)
fn init(@builtin(global_invocation_id) invocation_id: vec3<u32>, @builtin(num_workgroups) num_workgroups: vec3<u32>) {
    // let location = vec2<f32>(WIDTH * randomFloat(u32(invocation_id.x)), HEIGHT * randomFloat2(u32(invocation_id.x)));
    // let velocity = vec2<f32>(randomFloat3(u32(invocation_id.x)), randomFloat4(u32(invocation_id.x)));

    // let id = id(invocation_id, num_workgroups);
    // particles[id] = Particles(
    //     location,
    //     velocity,
    //     particles[id].particle_type,
    // );
}

// @compute @workgroup_size(64, 1, 1)
// fn update_spatial_hash(@builtin(global_invocation_id) id: vec3<u32>) {
//     if id.x >= particle_config.n {
//         return;
//     }

//     // Reset offsets
//     spatial_offsets[id.x] = particle_config.n;
//     // Update index buffer
//     let index: u32 = id.x;
//     let cell: vec2<i32> = getCell2d(particles[index].position, smoothing_radius);
//     let hash: u32 = hashCell2d(cell);
//     let key: u32 = keyFromHash(hash, particle_config.n);
//     spatial_indices[id.x] = vec3<u32>(index, hash, key);
// }

@compute @workgroup_size(WORKGROUP_SIZE, 1, 1)
fn update_velocities(
    @builtin(global_invocation_id) invocation_id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>
) {
    let id = id(invocation_id, num_workgroups);

    var total_force: vec2<f32> = vec2<f32>(0.0, 0.0);
    let origin_cell = getCell2D(particles[id].position, grid_cell_size);
    let sqr_radius = particle_config.n * particle_config.n;
    let velocity = particles[id].velocity;

    for (var i: i32 = 0; i < 9; i++) {
        let hash = hashCell2D(origin_cell + offsets_2d[i]);
        let key = keyFromHash(hash, table_size);
        var curr_index = spatial_offsets[key];

        while curr_index < particle_config.n {
            let index_data = spatial_indices[curr_index];
            curr_index++;
            if index_data.z != key {
                break;
            } // Exit if no longer looking at the correct bin
            if index_data.y != hash {
                continue;
            } // Skip if hash does not match

            let neighbour_index = index_data.x;
            if neighbour_index == id {
                continue;
            } // Skip if looking at self

            let neighbour_pos = particles[neighbour_index].position;
            let offset_to_neighbour = neighbour_pos - particles[id].position;
            let sqr_dst_to_neighbour = dot(offset_to_neighbour, offset_to_neighbour);

            if sqr_dst_to_neighbour > sqr_radius {
                continue;
            } // Skip if not within radius

            let dst = sqrt(sqr_dst_to_neighbour);
            let neighbour_velocity = particles[neighbour_index].velocity;
            total_force += neighbour_velocity - velocity;
        }
    }

    particles[id].velocity = (particles[id].velocity + total_force * particle_config.dt) * particle_config.friction_factor;

    
    // for (var i: u32 = 0; i < particle_config.n; i++) {
    //     if i == id {
    //         continue; // Skip self
    //     }
    //     let other = particles[i];
    //     let rx = other.position.x - particles[id].position.x;
    //     let ry = other.position.y - particles[id].position.y;
    //     let r = length(vec2<f32>(rx, ry));

    //     let a = attraction_matrix[particles[id].particle_type * particle_config.m + other.particle_type];

    //     if r > 0.0 && r < particle_config.r_max {
    //         let f = force(r / particle_config.r_max, a);
    //         total_force += vec2<f32>(rx / r * f, ry / r * f) * particle_config.r_max * particle_config.force_factor;
    //     }
    // }

    // particles[id].velocity = (particles[id].velocity + total_force * particle_config.dt) * particle_config.friction_factor;
}

@compute @workgroup_size(WORKGROUP_SIZE, 1, 1)
fn update_positions(
    @builtin(global_invocation_id) invocation_id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>
) {
    let id = id(invocation_id, num_workgroups);

    particles[id].position += particles[id].velocity * delta_time;

    // Wrap position if it goes out of the window bounds
    if particles[id].position.x < 0.0 {
        particles[id].position.x += WIDTH;
    } else if particles[id].position.x > WIDTH {
        particles[id].position.x -= WIDTH;
    }

    if particles[id].position.y < 0.0 {
        particles[id].position.y += HEIGHT;
    } else if particles[id].position.y > HEIGHT {
        particles[id].position.y -= HEIGHT;
    }
}

@compute @workgroup_size(WORKGROUP_SIZE, 1, 1)
fn update_spatial_hash_grid(
    @builtin(global_invocation_id) invocation_id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>
) {
    let id = id(invocation_id, num_workgroups);

    // Reset offsets
    spatial_offsets[id] = particle_config.n;
    // Update index buffer
    let index = id;
    let cell = getCell2D(particles[index].position, particle_config.r_max);
    let hash = hashCell2D(cell);
    let key = keyFromHash(hash, particle_config.n);
    spatial_indices[id] = vec3u(index, hash, key);
}


// // @compute @workgroup_size(WORKGROUP_SIZE, 1, 1)
// fn update2(
//     @builtin(global_invocation_id) invocation_id: vec3<u32>,
//     @builtin(num_workgroups) num_workgroups: vec3<u32>
// ) {
//     let id = id(invocation_id, num_workgroups);

//     var viscosity_force: vec2<f32> = vec2<f32>(0.0, 0.0);
//     let origin_cell = getCell2D(particles[id].position, grid_cell_size);
//     let sqr_radius = particle_config.n * particle_config.n;
//     let velocity = particles[id].velocity;

//     for (var i: i32 = 0; i < 9; i++) {
//         let hash = hashCell2D(origin_cell + offsets_2d[i]);
//         let key = keyFromHash(hash, table_size);
//         var curr_index = spatial_offsets[key];

//         while curr_index < particles.length {
//             let index_data = spatial_indices[curr_index];
//             curr_index++;
//             if index_data.z != key {
//                 break;
//             } // Exit if no longer looking at the correct bin
//             if index_data.y != hash {
//                 continue;
//             } // Skip if hash does not match

//             let neighbour_index = index_data.x;
//             if neighbour_index == id {
//                 continue;
//             } // Skip if looking at self

//             let neighbour_pos = particles[neighbour_index].position;
//             let offset_to_neighbour = neighbour_pos - particles[id].position;
//             let sqr_dst_to_neighbour = dot(offset_to_neighbour, offset_to_neighbour);

//             if sqr_dst_to_neighbour > sqr_radius {
//                 continue;
//             } // Skip if not within radius

//             let dst = sqrt(sqr_dst_to_neighbour);
//             let neighbour_velocity = particles[neighbour_index].velocity;
//             // Assume ViscosityKernel is defined elsewhere
//           //  viscosity_force += (neighbour_velocity - velocity) * ViscosityKernel(dst, smoothingRadius);
//         }
//     }

//     // particles[id].velocity += viscosity_force * viscosityStrength * deltaTime;
// }

// // @compute @workgroup_size(WORKGROUP_SIZE, 1, 1)
// fn updateSpatialHash(
//     @builtin(global_invocation_id) invocation_id: vec3<u32>,
//     @builtin(num_workgroups) num_workgroups: vec3<u32>
// ) {
//     let id = id(invocation_id, num_workgroups);

//     // Reset offsets
//     spatial_offsets[id] = particle_config.n;
//     // Update index buffer
//     let index = id;
//     let cell = getCell2D(particles[index].position, particle_config.r_max);
//     let hash = hashCell2D(cell);
//     let key = keyFromHash(hash, particle_config.n);
//     spatial_indices[id] = vec3u(index, hash, key);
// }