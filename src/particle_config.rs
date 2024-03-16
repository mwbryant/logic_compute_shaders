use bevy::{
    prelude::*,
    render::{extract_resource::ExtractResource, render_resource::ShaderType},
};
use rand::{rngs::ThreadRng, Rng};
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Resource, Reflect, Serialize, Deserialize, ExtractResource)]
#[reflect(Resource)]
pub struct ParticleConfig {
    pub n: usize,
    pub dt: f32,
    pub friction_half_life: f32,
    pub r_max: f32,
    pub m: usize,
    pub force_factor: f32,
    pub friction_factor: f32,
    pub attraction_matrix: Vec<f32>,
    pub recreate: bool,
}

impl ParticleConfig {
    pub fn extract_shader_variables(&self) -> (ShaderParticleConfig, Vec<f32>) {
        let shader_config = ShaderParticleConfig {
            n: self.n as u32,
            dt: self.dt,
            friction_half_life: self.friction_half_life,
            r_max: self.r_max,
            m: self.m as u32,
            force_factor: self.force_factor,
            friction_factor: self.friction_factor,
        };
        (shader_config, self.attraction_matrix.clone())
    }
}

#[derive(Debug, Clone, ShaderType)]
pub struct ShaderParticleConfig {
    pub n: u32,
    pub dt: f32,
    pub friction_half_life: f32,
    pub r_max: f32,
    pub m: u32,
    pub force_factor: f32,
    pub friction_factor: f32,
}

impl Default for ParticleConfig {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        let friction_half_life = 0.02;
        let dt = 0.0004;
        let m = rng.gen_range(1..=10);
        let n = 1;

        Self {
            n,
            dt,
            friction_half_life,
            r_max: 50.0,
            m,
            force_factor: 10.0,
            friction_factor: 0.5f32.powf(dt / friction_half_life),
            attraction_matrix: make_random_matrix(m),
            recreate: false,
        }
    }
}

pub fn make_random_matrix(m: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    let mut matrix = vec![0.0; m * m];
    for i in 0..m {
        for j in 0..m {
            matrix[i * m + j] = generate_random_gaussian(&mut rng);
        }
    }
    matrix
}

impl PartialEq for ParticleConfig {
    fn eq(&self, other: &Self) -> bool {
        self.n == other.n
            && self.m == other.m
            && self.recreate == other.recreate
            && float_eq(self.dt, other.dt)
            && float_eq(self.friction_half_life, other.friction_half_life)
            && float_eq(self.r_max, other.r_max)
            && float_eq(self.force_factor, other.force_factor)
            && float_eq(self.friction_factor, other.friction_factor)
    }
}

impl Eq for ParticleConfig {}

impl Hash for ParticleConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.n.hash(state);
        self.m.hash(state);
        hash_float(self.dt, state);
        hash_float(self.friction_half_life, state);
        hash_float(self.r_max, state);
        hash_float(self.force_factor, state);
        hash_float(self.friction_factor, state);
    }
}

fn float_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < f32::EPSILON
}

fn hash_float<F: Hasher>(f: f32, state: &mut F) {
    let bits = f.to_bits();
    bits.hash(state);
}

fn generate_random_gaussian(rng: &mut ThreadRng) -> f32 {
    let normal_dist = Normal::new(0.0, 0.5).unwrap();
    loop {
        let value = normal_dist.sample(rng);
        if value >= -1.0 && value <= 1.0 {
            return value;
        }
    }
}
