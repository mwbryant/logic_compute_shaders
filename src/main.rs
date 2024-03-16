#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::{
    log::LogPlugin,
    prelude::*,
    render::{render_asset::RenderAssetUsages, render_resource::*, texture::ImageSampler},
    sprite::{Material2d, Material2dPlugin, MaterialMesh2dBundle},
    window::WindowResolution,
};

pub const HEIGHT: f32 = 800.0;
pub const WIDTH: f32 = 800.0;
pub const WORKGROUP_SIZE: u32 = 16;

mod compute_utils;
mod particle;
mod particle_config;
mod particle_render;
mod particle_system;
mod particle_ui;
mod particle_update;
mod system_runner;

use particle_config::ParticleConfig;
use particle_system::{ParticlePlugin, ParticleSystem, RecreateParticles};

fn main() {
    let mut app = App::new();

    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(WIDTH, HEIGHT),
                    title: "Logic Particles".to_string(),
                    // resizable: false,
                    ..default()
                }),
                ..default()
            })
            .set(LogPlugin {
                filter: "info,wgpu_core=warn,wgpu_hal=warn,logic_gpu_particles=debug".into(),
                level: bevy::log::Level::DEBUG,
                update_subscriber: None,
            }), //.disable::<bevy::log::LogPlugin>(),
    )
    .insert_resource(ClearColor(Color::BLACK))
    .init_resource::<ParticleTexture>()
    //.add_plugins(WorldInspectorPlugin::new())
    .add_plugins(ParticlePlugin)
    .add_plugins(Material2dPlugin::<GrayscaleMaterial>::default())
    .add_systems(Update, bevy::window::close_on_esc)
    .add_systems(Startup, setup)
    .add_systems(Update, recreate_particles);

    // #[cfg(debug_assertions)]
    // {
    //     use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};
    //     app.add_plugins((FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin::default()));
    // }

    // bevy_mod_debugdump::print_schedule_graph(&mut app, PostUpdate);

    app.run();
}

fn create_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new_fill(
        Extent3d {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::default(),
    );
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    image.sampler = ImageSampler::nearest();
    images.add(image)
}

#[derive(Resource, Default)]
struct ParticleTexture(Option<Handle<Image>>);

#[derive(Component)]
struct ParticleImage;

fn recreate_particles(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut event_writer: EventWriter<RecreateParticles>,
    mut particle_config: ResMut<ParticleConfig>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        *particle_config = ParticleConfig::default();
        event_writer.send(RecreateParticles);
    }
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut image_texture: ResMut<ParticleTexture>,
    mut materials: ResMut<Assets<GrayscaleMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    image_texture.0 = Some(create_texture(&mut images));

    let image = image_texture.0.as_ref().unwrap().clone();

    let offsets = [
        (-WIDTH, HEIGHT),
        (0.0, HEIGHT),
        (WIDTH, HEIGHT),
        (-WIDTH, 0.0),
        (0.0, 0.0),
        (WIDTH, 0.0),
        (-WIDTH, -HEIGHT),
        (0.0, -HEIGHT),
        (WIDTH, -HEIGHT),
    ];

    commands
        .spawn(SpatialBundle::default())
        .with_children(|parent| {
            for (dx, dy) in offsets.iter() {
                if *dx == 0.0 && *dy == 0.0 {
                    parent.spawn(SpriteBundle {
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(WIDTH, HEIGHT)),
                            ..default()
                        },
                        transform: Transform::from_translation(Vec3::new(*dx, *dy, 0.0)),
                        texture: image.clone(),
                        ..default()
                    });
                } else {
                    parent.spawn(MaterialMesh2dBundle {
                        mesh: meshes.add(Rectangle::new(WIDTH, HEIGHT)).into(),
                        transform: Transform::from_translation(Vec3::new(*dx, *dy, 0.0)),
                        material: materials.add(GrayscaleMaterial {
                            texture: Some(image.clone()),
                        }),
                        ..default()
                    });
                };
            }
        })
        .insert(ParticleSystem {
            rendered_texture: image,
        });

    commands.spawn(Camera2dBundle::default());
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct GrayscaleMaterial {
    #[texture(1)]
    #[sampler(2)]
    texture: Option<Handle<Image>>,
}

impl Material2d for GrayscaleMaterial {
    fn fragment_shader() -> ShaderRef {
        "grayscale.wgsl".into()
    }
}
