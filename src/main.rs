#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use std::{borrow::Cow, ops::Deref};

use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_graph::{self, RenderGraph},
        render_resource::*,
        renderer::{RenderContext, RenderDevice, RenderQueue},
        RenderApp, RenderStage,
    },
};
use bevy_inspector_egui::WorldInspectorPlugin;

pub const HEIGHT: f32 = 480.0;
pub const WIDTH: f32 = 640.0;

const PARTICLE_COUNT: u32 = 1024;
// XXX when changing this also change it in the shader... TODO figure out how to avoid that...
const WORKGROUP_SIZE: u32 = 16;

use bevy::log::LogPlugin;
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
    .add_plugin(GameOfLifeComputePlugin)
    .add_startup_system(setup)
    .add_system(clear_texture);
    //bevy_mod_debugdump::print_render_graph(&mut app);
    app.run();
}

//There is probably a much better way to clear a texture
fn clear_texture(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut sprite: Query<&mut Handle<Image>>,
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

    let mut sprite = sprite.single_mut();
    *sprite = image.clone();
    commands.insert_resource(ParticleImage(image));
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

    commands.spawn(SpriteBundle {
        sprite: Sprite {
            custom_size: Some(Vec2::new(WIDTH * 3.0, HEIGHT * 3.0)),
            ..default()
        },
        texture: image.clone(),
        ..default()
    });
    commands.spawn(Camera2dBundle::default());

    commands.insert_resource(ParticleImage(image));
}

pub struct GameOfLifeComputePlugin;

impl Plugin for GameOfLifeComputePlugin {
    fn build(&self, app: &mut App) {
        // Extract the game of life image resource from the main world into the render world
        // for operation on by the compute shader and display on the sprite.
        app.add_plugin(ExtractResourcePlugin::<ParticleImage>::default());
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<ParticleUpdatePipeline>()
            .init_resource::<ParticleRenderPipeline>()
            .add_system_to_stage(RenderStage::Queue, queue_bind_group);

        let mut render_graph = render_app.world.resource_mut::<RenderGraph>();
        render_graph.add_node("update_particles", UpdateParticlesNode::default());
        render_graph.add_node("render_particles", RenderParticleNode::default());

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

#[derive(Resource, Clone, Deref, ExtractResource)]
struct ParticleImage(Handle<Image>);

#[derive(Resource)]
struct ParticleUpdateBindGroup(BindGroup);

#[derive(Resource)]
struct ParticleRenderBindGroup(BindGroup);

// Helper function to print out gpu data for debugging
pub fn read_buffer(buffer: &Buffer, device: &RenderDevice, queue: &RenderQueue) {
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    //FIXME this could come from buffer.size
    let scratch = [0; PARTICLE_COUNT as usize];
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
    mut commands: Commands,
    update_pipeline: Res<ParticleUpdatePipeline>,
    render_pipeline: Res<ParticleRenderPipeline>,
    gpu_images: Res<RenderAssets<Image>>,
    particle_image: Res<ParticleImage>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let view = &gpu_images[&particle_image.0];

    //read_buffer(&pipeline.storage, &render_device, &render_queue);
    let update_group = render_device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &update_pipeline.bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: BindingResource::Buffer(update_pipeline.storage.as_entire_buffer_binding()),
        }],
    });
    commands.insert_resource(ParticleUpdateBindGroup(update_group));

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
    commands.insert_resource(ParticleRenderBindGroup(render_group));
}

#[derive(Resource)]
pub struct ParticleUpdatePipeline {
    bind_group_layout: BindGroupLayout,
    init_pipeline: CachedComputePipelineId,
    update_pipeline: CachedComputePipelineId,
    storage: Buffer,
}

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

        let storage =
            world
                .resource::<RenderDevice>()
                .create_buffer_with_data(&BufferInitDescriptor {
                    label: None,
                    usage: BufferUsages::COPY_DST | BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                    contents: &[0; PARTICLE_COUNT as usize],
                });
        ParticleUpdatePipeline {
            bind_group_layout: texture_bind_group_layout,
            init_pipeline,
            update_pipeline,
            storage,
        }
    }
}

enum ParticleUpdateState {
    Loading,
    Init,
    Update,
}

struct UpdateParticlesNode {
    state: ParticleUpdateState,
}

impl Default for UpdateParticlesNode {
    fn default() -> Self {
        Self {
            state: ParticleUpdateState::Loading,
        }
    }
}

impl render_graph::Node for UpdateParticlesNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<ParticleUpdatePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        // if the corresponding pipeline has loaded, transition to the next stage
        match self.state {
            ParticleUpdateState::Loading => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.init_pipeline)
                {
                    self.state = ParticleUpdateState::Init;
                }
            }
            ParticleUpdateState::Init => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update_pipeline)
                {
                    self.state = ParticleUpdateState::Update;
                }
            }
            ParticleUpdateState::Update => {}
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let bind_group = &world.resource::<ParticleUpdateBindGroup>().0;
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ParticleUpdatePipeline>();

        let mut pass = render_context
            .command_encoder
            .begin_compute_pass(&ComputePassDescriptor::default());

        pass.set_bind_group(0, bind_group, &[]);

        // select the pipeline based on the current state
        match self.state {
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

        Ok(())
    }
}

#[derive(Resource)]
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

enum ParticleRenderState {
    Loading,
    Render,
}

struct RenderParticleNode {
    state: ParticleRenderState,
}

impl Default for RenderParticleNode {
    fn default() -> Self {
        Self {
            state: ParticleRenderState::Loading,
        }
    }
}

impl render_graph::Node for RenderParticleNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<ParticleRenderPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        // if the corresponding pipeline has loaded, transition to the next stage
        match self.state {
            ParticleRenderState::Loading => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.render_pipeline)
                {
                    self.state = ParticleRenderState::Render;
                }
            }
            ParticleRenderState::Render => {}
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let bind_group = &world.resource::<ParticleRenderBindGroup>().0;
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ParticleRenderPipeline>();

        let mut pass = render_context
            .command_encoder
            .begin_compute_pass(&ComputePassDescriptor::default());

        pass.set_bind_group(0, bind_group, &[]);

        // select the pipeline based on the current state
        match self.state {
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

        Ok(())
    }
}
