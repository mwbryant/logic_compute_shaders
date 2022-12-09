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
mod particle_update;

use compute_utils::read_buffer;
use particle_update::{ParticleUpdatePipeline, UpdateParticlesNode};
use wgpu::Maintain;

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

impl ExtractComponent for ParticleSystem {
    type Query = &'static ParticleSystem;
    type Filter = ();

    fn extract_component(item: bevy::ecs::query::QueryItem<'_, Self::Query>) -> Self {
        // XXX this clone might be expensive, bindgroups are made with an arc, should always be in render world anyway
        item.clone()
    }
}

// Must maintain all our own data because render world flushes between frames :,(
#[derive(Resource, Default)]
pub struct ParticleSystemRender {
    pub update_bind_group: HashMap<Entity, BindGroup>,
    pub render_bind_group: HashMap<Entity, BindGroup>,
    pub particle_buffers: HashMap<Entity, Buffer>,
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

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ExtractComponentPlugin::<ParticleSystem>::default());
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<ParticleUpdatePipeline>()
            .init_resource::<ParticleSystemRender>()
            .init_resource::<ParticleRenderPipeline>()
            .add_system_to_stage(RenderStage::Queue, queue_bind_group);

        let update_node = UpdateParticlesNode::new(&mut render_app.world);
        let render_node = RenderParticlesNode::new(&mut render_app.world);
        let mut render_graph = render_app.world.resource_mut::<RenderGraph>();
        render_graph.add_node("update_particles", update_node);
        render_graph.add_node("render_particles", render_node);

        render_graph
            .add_node_edge("update_particles", "render_particles")
            .unwrap();
        render_graph
            .add_node_edge(
                "render_particles",
                bevy::render::main_graph::node::CAMERA_DRIVER,
            )
            .unwrap();
    }
}

fn queue_bind_group(
    render_device: Res<RenderDevice>,
    //render_queue: Res<RenderQueue>,
    render_pipeline: Res<ParticleRenderPipeline>,
    gpu_images: Res<RenderAssets<Image>>,
    mut particle_systems_render: ResMut<ParticleSystemRender>,
    update_pipeline: Res<ParticleUpdatePipeline>,
    //Getting mutable queries in the render world is an antipattern?
    particle_systems: Query<(Entity, &ParticleSystem)>,
) {
    // Everything here is done lazily and should only happen on the first call here.
    for (entity, system) in &particle_systems {
        let view = &gpu_images[&system.image];

        if !particle_systems_render
            .particle_buffers
            .contains_key(&entity)
        {
            let particle = [Particle::default(); PARTICLE_COUNT as usize];
            //ugh
            let mut byte_buffer = Vec::new();
            let mut buffer = encase::StorageBuffer::new(&mut byte_buffer);
            buffer.write(&particle).unwrap();
            let storage = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: None,
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                contents: buffer.into_inner(),
            });
            particle_systems_render
                .particle_buffers
                .insert(entity, storage);
        }

        /*
        read_buffer(
            &particle_systems_render.particle_buffers[&entity],
            &render_device,
            &render_queue,
        );
        */

        if !particle_systems_render
            .update_bind_group
            .contains_key(&entity)
        {
            info!("Creating");
            let update_group = render_device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &update_pipeline.bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(
                        particle_systems_render.particle_buffers[&entity]
                            .as_entire_buffer_binding(),
                    ),
                }],
            });
            particle_systems_render
                .update_bind_group
                .insert(entity, update_group);
        }

        if !particle_systems_render
            .render_bind_group
            .contains_key(&entity)
        {
            let render_group = render_device.create_bind_group(&BindGroupDescriptor {
                label: None,
                layout: &render_pipeline.render_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(
                            particle_systems_render.particle_buffers[&entity]
                                .as_entire_buffer_binding(),
                        ),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&view.texture_view),
                    },
                ],
            });
            particle_systems_render
                .render_bind_group
                .insert(entity, render_group);
        }
    }
}

#[derive(Resource, Clone)]
pub struct ParticleRenderPipeline {
    render_group_layout: BindGroupLayout,
    clear_pipeline: CachedComputePipelineId,
    render_pipeline: CachedComputePipelineId,
}

impl FromWorld for ParticleRenderPipeline {
    fn from_world(world: &mut World) -> Self {
        let texture_bind_group_layout =
            world
                .resource::<RenderDevice>()
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[
                        BindGroupLayoutEntry {
                            binding: 0,
                            visibility: ShaderStages::COMPUTE,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        BindGroupLayoutEntry {
                            binding: 1,
                            visibility: ShaderStages::COMPUTE,
                            ty: BindingType::StorageTexture {
                                access: StorageTextureAccess::ReadWrite,
                                format: TextureFormat::Rgba8Unorm,
                                view_dimension: TextureViewDimension::D2,
                            },
                            count: None,
                        },
                    ],
                });
        let shader = world.resource::<AssetServer>().load("particle_render.wgsl");
        let mut pipeline_cache = world.resource_mut::<PipelineCache>();
        let render_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: Some(vec![texture_bind_group_layout.clone()]),
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Cow::from("render"),
        });

        let clear_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: Some(vec![texture_bind_group_layout.clone()]),
            shader,
            shader_defs: vec![],
            entry_point: Cow::from("clear"),
        });

        ParticleRenderPipeline {
            render_group_layout: texture_bind_group_layout,
            clear_pipeline,
            render_pipeline,
        }
    }
}

#[derive(Default, Clone, Debug)]
enum ParticleRenderState {
    #[default]
    Loading,
    Render,
}

struct RenderParticlesNode {
    particle_systems: QueryState<(Entity, &'static ParticleSystem)>,
    render_state: HashMap<Entity, ParticleRenderState>,
}

impl RenderParticlesNode {
    fn new(world: &mut World) -> Self {
        Self {
            particle_systems: QueryState::new(world),
            render_state: HashMap::default(),
        }
    }
}

impl render_graph::Node for RenderParticlesNode {
    fn update(&mut self, world: &mut World) {
        let mut systems = world.query_filtered::<Entity, With<ParticleSystem>>();
        let pipeline = world.resource::<ParticleRenderPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        for entity in systems.iter(world) {
            // if the corresponding pipeline has loaded, transition to the next stage
            let render_state = match self.render_state.get(&entity) {
                Some(state) => state,
                None => {
                    self.render_state
                        .insert(entity, ParticleRenderState::Loading);
                    &ParticleRenderState::Loading
                }
            };
            // if the corresponding pipeline has loaded, transition to the next stage
            match render_state {
                // if the corresponding pipeline has loaded, transition to the next stage
                ParticleRenderState::Loading => {
                    if let CachedPipelineState::Ok(_) =
                        pipeline_cache.get_compute_pipeline_state(pipeline.render_pipeline)
                    {
                        self.render_state
                            .insert(entity, ParticleRenderState::Render);
                    }
                }
                ParticleRenderState::Render => {}
            }
        }
        self.particle_systems.update_archetypes(world);
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ParticleRenderPipeline>();
        let particle_systems_render = world.resource::<ParticleSystemRender>();

        for (entity, system) in self.particle_systems.iter_manual(world) {
            let mut pass = render_context
                .command_encoder
                .begin_compute_pass(&ComputePassDescriptor::default());

            pass.set_bind_group(0, &particle_systems_render.render_bind_group[&entity], &[]);

            // select the pipeline based on the current state
            match self.render_state[&entity] {
                ParticleRenderState::Loading => {}
                ParticleRenderState::Render => {
                    let clear_pipeline = pipeline_cache
                        .get_compute_pipeline(pipeline.clear_pipeline)
                        .unwrap();
                    pass.set_pipeline(clear_pipeline);
                    pass.dispatch_workgroups(
                        WIDTH as u32 / WORKGROUP_SIZE,
                        HEIGHT as u32 / WORKGROUP_SIZE,
                        1,
                    );

                    let render_pipeline = pipeline_cache
                        .get_compute_pipeline(pipeline.render_pipeline)
                        .unwrap();
                    pass.set_pipeline(render_pipeline);
                    //FIXME
                    pass.dispatch_workgroups(PARTICLE_COUNT / WORKGROUP_SIZE, 1, 1);
                }
            }
        }

        Ok(())
    }
}
