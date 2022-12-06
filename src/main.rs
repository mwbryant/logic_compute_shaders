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

const SIZE: (u32, u32) = (1280, 720);
const WORKGROUP_SIZE: u32 = 8;

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
    .add_startup_system(setup);
    //bevy_mod_debugdump::print_render_graph(&mut app);
    app.run();
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let mut image = Image::new_fill(
        Extent3d {
            width: SIZE.0,
            height: SIZE.1,
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
            custom_size: Some(Vec2::new(SIZE.0 as f32, SIZE.1 as f32)),
            ..default()
        },
        texture: image.clone(),
        ..default()
    });
    commands.spawn(Camera2dBundle::default());

    commands.insert_resource(GameOfLifeImage(image));
}

pub struct GameOfLifeComputePlugin;

impl Plugin for GameOfLifeComputePlugin {
    fn build(&self, app: &mut App) {
        // Extract the game of life image resource from the main world into the render world
        // for operation on by the compute shader and display on the sprite.
        app.add_plugin(ExtractResourcePlugin::<GameOfLifeImage>::default());
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<GameOfLifePipeline>()
            .add_system_to_stage(RenderStage::Queue, queue_bind_group);

        let mut render_graph = render_app.world.resource_mut::<RenderGraph>();
        render_graph.add_node("game_of_life", GameOfLifeNode::default());
        render_graph
            .add_node_edge(
                "game_of_life",
                bevy::render::main_graph::node::CAMERA_DRIVER,
            )
            .unwrap();
    }
}

#[derive(Resource, Clone, Deref, ExtractResource)]
struct GameOfLifeImage(Handle<Image>);

#[derive(Resource)]
struct GameOfLifeImageBindGroup(BindGroup);

pub fn read_buffer(buffer: &Buffer, len: usize, device: &RenderDevice, queue: &RenderQueue) {
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: None });

    let scratch = StorageBufferWrapper::new(vec![0, 0, 0, 0, 0, 0, 0, 0]);
    let dest = device.create_buffer_with_data(&BufferInitDescriptor {
        label: None,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        contents: scratch.as_ref(),
    });
    encoder.copy_buffer_to_buffer(buffer, 0, &dest, 0, 8);
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
use encase::{
    internal::WriteInto, DynamicStorageBuffer as DynamicStorageBufferWrapper, ShaderType,
    StorageBuffer as StorageBufferWrapper,
};

pub struct MyStorageBuffer<T: ShaderType> {
    value: T,
    scratch: StorageBufferWrapper<Vec<u8>>,
    buffer: Option<Buffer>,
    capacity: usize,
    label: Option<String>,
    label_changed: bool,
}

impl<T: ShaderType + WriteInto> MyStorageBuffer<T> {
    #[inline]
    pub fn buffer(&self) -> Option<&Buffer> {
        self.buffer.as_ref()
    }

    #[inline]
    pub fn binding(&self) -> Option<BindingResource> {
        Some(BindingResource::Buffer(
            self.buffer()?.as_entire_buffer_binding(),
        ))
    }

    pub fn set(&mut self, value: T) {
        self.value = value;
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    pub fn set_label(&mut self, label: Option<&str>) {
        let label = label.map(str::to_string);

        if label != self.label {
            self.label_changed = true;
        }

        self.label = label;
    }

    pub fn get_label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Queues writing of data from system RAM to VRAM using the [`RenderDevice`](crate::renderer::RenderDevice)
    /// and the provided [`RenderQueue`](crate::renderer::RenderQueue).
    ///
    /// If there is no GPU-side buffer allocated to hold the data currently stored, or if a GPU-side buffer previously
    /// allocated does not have enough capacity, a new GPU-side buffer is created.
    pub fn write_buffer(&mut self, device: &RenderDevice, queue: &RenderQueue) {
        self.scratch.write(&self.value).unwrap();

        let size = self.scratch.as_ref().len();

        if self.capacity < size || self.label_changed {
            self.buffer = Some(device.create_buffer_with_data(&BufferInitDescriptor {
                label: self.label.as_deref(),
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                contents: self.scratch.as_ref(),
            }));
            self.capacity = size;
            self.label_changed = false;
        } else if let Some(buffer) = &self.buffer {
            queue.write_buffer(buffer, 0, self.scratch.as_ref());
        }
    }
}
impl<T: ShaderType + Default> Default for MyStorageBuffer<T> {
    fn default() -> Self {
        Self {
            value: T::default(),
            scratch: StorageBufferWrapper::new(Vec::new()),
            buffer: None,
            capacity: 0,
            label: None,
            label_changed: false,
        }
    }
}

fn queue_bind_group(
    mut commands: Commands,
    pipeline: Res<GameOfLifePipeline>,
    gpu_images: Res<RenderAssets<Image>>,
    game_of_life_image: Res<GameOfLifeImage>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let view = &gpu_images[&game_of_life_image.0];
    let test = pipeline.storage.get();
    read_buffer(
        pipeline.storage.buffer().unwrap(),
        0,
        &render_device,
        &render_queue,
    );

    //println!("{:?}", test);
    let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &pipeline.texture_bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: pipeline.storage.binding().unwrap(),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&view.texture_view),
            },
        ],
    });
    commands.insert_resource(GameOfLifeImageBindGroup(bind_group));
}

#[derive(Resource)]
pub struct GameOfLifePipeline {
    texture_bind_group_layout: BindGroupLayout,
    init_pipeline: CachedComputePipelineId,
    update_pipeline: CachedComputePipelineId,
    storage: MyStorageBuffer<IVec2>,
}

impl FromWorld for GameOfLifePipeline {
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
        let shader = world.resource::<AssetServer>().load("game_of_life.wgsl");
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

        let mut storage = MyStorageBuffer::default();
        *storage.get_mut() = IVec2::new(7, 9);
        storage.write_buffer(
            world.resource::<RenderDevice>(),
            world.resource::<RenderQueue>(),
        );
        GameOfLifePipeline {
            texture_bind_group_layout,
            init_pipeline,
            update_pipeline,
            storage,
        }
    }
}

enum GameOfLifeState {
    Loading,
    Init,
    Update,
}

struct GameOfLifeNode {
    state: GameOfLifeState,
}

impl Default for GameOfLifeNode {
    fn default() -> Self {
        Self {
            state: GameOfLifeState::Loading,
        }
    }
}

impl render_graph::Node for GameOfLifeNode {
    fn update(&mut self, world: &mut World) {
        let pipeline = world.resource::<GameOfLifePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        // if the corresponding pipeline has loaded, transition to the next stage
        match self.state {
            GameOfLifeState::Loading => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.init_pipeline)
                {
                    self.state = GameOfLifeState::Init;
                }
            }
            GameOfLifeState::Init => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update_pipeline)
                {
                    self.state = GameOfLifeState::Update;
                }
            }
            GameOfLifeState::Update => {}
        }
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let texture_bind_group = &world.resource::<GameOfLifeImageBindGroup>().0;
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<GameOfLifePipeline>();

        let mut pass = render_context
            .command_encoder
            .begin_compute_pass(&ComputePassDescriptor::default());

        pass.set_bind_group(0, texture_bind_group, &[]);

        // select the pipeline based on the current state
        match self.state {
            GameOfLifeState::Loading => {}
            GameOfLifeState::Init => {
                let init_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.init_pipeline)
                    .unwrap();
                pass.set_pipeline(init_pipeline);
                pass.dispatch_workgroups(SIZE.0 / WORKGROUP_SIZE, SIZE.1 / WORKGROUP_SIZE, 1);
            }
            GameOfLifeState::Update => {
                let update_pipeline = pipeline_cache
                    .get_compute_pipeline(pipeline.update_pipeline)
                    .unwrap();
                pass.set_pipeline(update_pipeline);
                pass.dispatch_workgroups(SIZE.0 / WORKGROUP_SIZE, SIZE.1 / WORKGROUP_SIZE, 1);
            }
        }

        Ok(())
    }
}
