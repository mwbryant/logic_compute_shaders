#![allow(clippy::too_many_arguments, clippy::type_complexity)]
//TODO time

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

const PARTICLE_COUNT: u32 = 1024 * 1000;
#[derive(ShaderType, Default, Clone, Copy)]
struct Particle {
    position: Vec2,
}

// XXX when changing this also change it in the shader... TODO figure out how to avoid that...
const WORKGROUP_SIZE: u32 = 16;

use wgpu::Maintain;

fn main() {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins.set(WindowPlugin {
            window: WindowDescriptor {
                width: WIDTH,
                height: HEIGHT,
                title: "Logic Particles".to_string(),
                resizable: false,
                ..default()
            },
            ..default()
        }), //.disable::<LogPlugin>(),
    )
    .add_plugin(WorldInspectorPlugin::new())
    .add_plugin(ParticlePlugin)
    .add_startup_system(setup)
    .add_system(clear_texture);
    //bevy_mod_debugdump::print_render_graph(&mut app);
    app.run();
}

#[derive(Component, Default, Clone)]
pub struct ParticleSystem {
    image: Handle<Image>,
    update_bind_group: Option<ParticleUpdateBindGroup>,
    render_bind_group: Option<ParticleRenderBindGroup>,
}

impl ExtractComponent for ParticleSystem {
    type Query = &'static ParticleSystem;
    type Filter = ();

    fn extract_component(item: bevy::ecs::query::QueryItem<'_, Self::Query>) -> Self {
        // XXX this clone might be expensive, bindgroups are made with an arc, should always be in render world anyway
        item.clone()
    }
}

//There is probably a much better way to clear a texture
fn clear_texture(
    mut images: ResMut<Assets<Image>>,
    mut sprite: Query<(&mut Handle<Image>, &mut ParticleSystem)>,
) {
    let mut image = Image::new_fill(
        Extent3d {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 0],
        TextureFormat::Rgba8Unorm,
    );
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    let image = images.add(image);

    let (mut sprite, mut system) = sprite.single_mut();
    *sprite = image.clone();
    // wish i wasn't double booking this
    system.image = image;
    //commands.insert_resource(ParticleImage(image));
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: WIDTH as u32,
            height: HEIGHT as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 0, 0, 255],
        TextureFormat::Rgba8Unorm,
    );
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING;
    let image = images.add(image);

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                custom_size: Some(Vec2::new(WIDTH * 3.0, HEIGHT * 3.0)),
                ..default()
            },
            texture: image,
            ..default()
        })
        .insert(ParticleSystem::default());
    commands.spawn(Camera2dBundle::default());
}

pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ExtractComponentPlugin::<ParticleSystem>::default());
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<ParticleUpdatePipeline>()
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

//#[derive(Resource, Clone, Deref, ExtractResource)]
//struct ParticleImage(Handle<Image>);

#[derive(Clone)]
struct ParticleUpdateBindGroup(BindGroup);

#[derive(Clone)]
struct ParticleRenderBindGroup(BindGroup);

// Helper function to print out gpu data for debugging
pub fn read_buffer(buffer: &Buffer, device: &RenderDevice, queue: &RenderQueue) {
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    //FIXME this could come from buffer.size
    let scratch = [0; (Particle::SHADER_SIZE.get() * PARTICLE_COUNT as u64) as usize];
    let dest = device.create_buffer_with_data(&BufferInitDescriptor {
        label: None,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        contents: scratch.as_ref(),
    });
    encoder.copy_buffer_to_buffer(buffer, 0, &dest, 0, buffer.size());
    queue.submit([encoder.finish()]);
    let slice = dest.slice(..);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let err = result.err();
        if err.is_some() {
            panic!("{}", err.unwrap().to_string());
        }
    });
    device.poll(Maintain::Wait);
    let data = slice.get_mapped_range();
    let result = Vec::from(data.deref());
    println!("{:?}", result);
}

fn queue_bind_group(
    update_pipeline: Res<ParticleUpdatePipeline>,
    render_pipeline: Res<ParticleRenderPipeline>,
    gpu_images: Res<RenderAssets<Image>>,
    mut particle_systems: Query<&mut ParticleSystem>,
    render_device: Res<RenderDevice>,
) {
    for mut system in &mut particle_systems {
        let view = &gpu_images[&system.image];

        //read_buffer(&pipeline.storage, &render_device, &render_queue);
        let update_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &update_pipeline.bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(
                    update_pipeline.storage.as_entire_buffer_binding(),
                ),
            }],
        });
        system.update_bind_group = Some(ParticleUpdateBindGroup(update_group));

        let render_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &render_pipeline.render_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(
                        update_pipeline.storage.as_entire_buffer_binding(),
                    ),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&view.texture_view),
                },
            ],
        });
        system.render_bind_group = Some(ParticleRenderBindGroup(render_group));
    }
}

#[derive(Resource, Clone)]
pub struct ParticleUpdatePipeline {
    bind_group_layout: BindGroupLayout,
    init_pipeline: CachedComputePipelineId,
    update_pipeline: CachedComputePipelineId,
    storage: Buffer,
}

#[derive(Resource)]
pub struct ParticleSystemStateTracker {}

impl FromWorld for ParticleUpdatePipeline {
    fn from_world(world: &mut World) -> Self {
        let texture_bind_group_layout =
            world
                .resource::<RenderDevice>()
                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });
        let shader = world.resource::<AssetServer>().load("particle_update.wgsl");
        let mut pipeline_cache = world.resource_mut::<PipelineCache>();
        let init_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: Some(vec![texture_bind_group_layout.clone()]),
            shader: shader.clone(),
            shader_defs: vec![],
            entry_point: Cow::from("init"),
        });
        let update_pipeline = pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: None,
            layout: Some(vec![texture_bind_group_layout.clone()]),
            shader,
            shader_defs: vec![],
            entry_point: Cow::from("update"),
        });

        let particle = [Particle::default(); PARTICLE_COUNT as usize];
        //ugh
        let mut byte_buffer = Vec::new();
        let mut buffer = encase::StorageBuffer::new(&mut byte_buffer);
        buffer.write(&particle).unwrap();
        let storage =
            world
                .resource::<RenderDevice>()
                .create_buffer_with_data(&BufferInitDescriptor {
                    label: None,
                    usage: BufferUsages::COPY_DST | BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                    contents: buffer.into_inner(),
                });
        ParticleUpdatePipeline {
            bind_group_layout: texture_bind_group_layout,
            init_pipeline,
            update_pipeline,
            storage,
        }
    }
}

#[derive(Default, Clone)]
enum ParticleUpdateState {
    #[default]
    Loading,
    Init,
    Update,
}

struct UpdateParticlesNode {
    particle_systems: QueryState<(Entity, &'static ParticleSystem)>,
    //FIXME flush this when the entities no long exists, grows without bound if constantly creating and destroying spawners
    update_state: HashMap<Entity, ParticleUpdateState>,
}

impl UpdateParticlesNode {
    fn new(world: &mut World) -> Self {
        Self {
            particle_systems: QueryState::new(world),
            update_state: HashMap::default(),
        }
    }
}

impl render_graph::Node for UpdateParticlesNode {
    fn update(&mut self, world: &mut World) {
        let mut systems = world.query_filtered::<Entity, With<ParticleSystem>>();
        let pipeline = world.resource::<ParticleUpdatePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        for entity in systems.iter(world) {
            // if the corresponding pipeline has loaded, transition to the next stage
            let update_state = match self.update_state.get(&entity) {
                Some(state) => state,
                None => {
                    self.update_state
                        .insert(entity, ParticleUpdateState::Loading);
                    &ParticleUpdateState::Loading
                }
            };
            match update_state {
                ParticleUpdateState::Loading => {
                    if let CachedPipelineState::Ok(_) =
                        pipeline_cache.get_compute_pipeline_state(pipeline.init_pipeline)
                    {
                        self.update_state.insert(entity, ParticleUpdateState::Init);
                    }
                }
                ParticleUpdateState::Init => {
                    if let CachedPipelineState::Ok(_) =
                        pipeline_cache.get_compute_pipeline_state(pipeline.update_pipeline)
                    {
                        self.update_state
                            .insert(entity, ParticleUpdateState::Update);
                    }
                }
                ParticleUpdateState::Update => {}
            }
        }
        //Update the query for the run step
        self.particle_systems.update_archetypes(world);
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ParticleUpdatePipeline>();

        //Am I using iter manual correctly?
        for (entity, system) in self.particle_systems.iter_manual(world) {
            let mut pass = render_context
                .command_encoder
                .begin_compute_pass(&ComputePassDescriptor::default());

            pass.set_bind_group(0, &system.update_bind_group.as_ref().unwrap().0, &[]);

            // select the pipeline based on the current state
            match self.update_state[&entity] {
                ParticleUpdateState::Loading => {}
                ParticleUpdateState::Init => {
                    let init_pipeline = pipeline_cache
                        .get_compute_pipeline(pipeline.init_pipeline)
                        .unwrap();
                    pass.set_pipeline(init_pipeline);
                    pass.dispatch_workgroups(PARTICLE_COUNT / WORKGROUP_SIZE, 1, 1);
                }
                ParticleUpdateState::Update => {
                    let update_pipeline = pipeline_cache
                        .get_compute_pipeline(pipeline.update_pipeline)
                        .unwrap();
                    pass.set_pipeline(update_pipeline);
                    pass.dispatch_workgroups(PARTICLE_COUNT / WORKGROUP_SIZE, 1, 1);
                }
            }
        }

        Ok(())
    }
}

#[derive(Resource, Clone)]
pub struct ParticleRenderPipeline {
    render_group_layout: BindGroupLayout,
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
            shader,
            shader_defs: vec![],
            entry_point: Cow::from("render"),
        });

        ParticleRenderPipeline {
            render_group_layout: texture_bind_group_layout,
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

        for (entity, system) in self.particle_systems.iter_manual(world) {
            let mut pass = render_context
                .command_encoder
                .begin_compute_pass(&ComputePassDescriptor::default());

            pass.set_bind_group(0, &system.render_bind_group.as_ref().unwrap().0, &[]);

            // select the pipeline based on the current state
            match self.render_state[&entity] {
                ParticleRenderState::Loading => {}
                ParticleRenderState::Render => {
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
