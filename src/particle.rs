use bevy::{prelude::*, render::render_resource::ShaderType};

#[derive(ShaderType, Default, Clone, Copy)]
pub struct Particle {
    pub position: Vec2,
    pub velocity: Vec2,
    pub particle_type: u32,
}
