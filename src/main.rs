#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::{borrow::Cow, ops::Deref};

use bevy::{
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_asset::RenderAssets,
        render_graph::{self, RenderGraph},
        render_resource::*,
        renderer::{RenderContext, RenderDevice, RenderQueue},
        RenderApp, RenderStage,
    },
    utils::HashMap,
};
use bevy_inspector_egui::WorldInspectorPlugin;
use compute_utils::read_buffer;
use particle_update::{ParticleUpdatePipeline, UpdateParticlesNode};
use wgpu::Maintain;

pub const HEIGHT: f32 = 480.0;
pub const WIDTH: f32 = 640.0;

pub const PARTICLE_COUNT: u32 = 1000;
// XXX when changing this also change it in the shader... TODO figure out how to avoid that...
pub const WORKGROUP_SIZE: u32 = 16;

#[derive(ShaderType, Default, Clone, Copy)]
struct Particle {
    position: Vec2,
}

mod compute_utils;
mod particle_render;
mod particle_system;
mod particle_update;

use particle_system::{ParticlePlugin, ParticleSystemRender};

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                width: WIDTH,
                height: HEIGHT,
                title: "Logic Particles".to_string(),
                //present_mode: bevy::window::PresentMode::Immediate,
                resizable: false,
                ..default()
            },
            ..default()
        }), //.disable::<LogPlugin>(),
    )
    .add_plugin(WorldInspectorPlugin::new())
    .add_plugin(ParticlePlugin)
    .add_startup_system(setup)
    //.add_system(clear_texture)
    .add_system(spawn);
    //bevy_mod_debugdump::print_render_graph(&mut app);
    app.run();
}

#[derive(Component, Default, Clone)]
pub struct ParticleSystem {
    pub image: Handle<Image>,
}

//There is probably a much better way to clear a texture
fn create_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new_fill(
        Extent3d {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 255, 127],
        TextureFormat::Rgba8Unorm,
    );
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    images.add(image)
}

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>, keyboard: Res<Input<KeyCode>>) {
    if keyboard.pressed(KeyCode::Space) {
        let image = create_texture(&mut images);
        commands
            .spawn(SpriteBundle {
                sprite: Sprite {
                    custom_size: Some(Vec2::new(WIDTH * 3.0, HEIGHT * 3.0)),
                    ..default()
                },
                texture: image.clone(),
                ..default()
            })
            .insert(ParticleSystem { image });
    }
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = create_texture(&mut images);
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                custom_size: Some(Vec2::new(WIDTH * 3.0, HEIGHT * 3.0)),
                ..default()
            },
            texture: image.clone(),
            ..default()
        })
        .insert(ParticleSystem { image });

    commands.spawn(Camera2dBundle::default());
}
