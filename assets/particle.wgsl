struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    particle_type: u32,
}

struct Particles {
    position: vec2<f32>,
    velocity: vec2<f32>,
    particle_type: u32,
}

struct ParticleConfig {
    n: u32, 
    dt: f32,
    friction_half_life: f32,
    r_max: f32,
    m: u32,
    force_factor: f32,
    friction_factor: f32,
};