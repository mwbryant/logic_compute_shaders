use crate::compute_utils::{compute_pipeline_descriptor, run_compute_pass, run_compute_pass_2d};
use crate::particle_system::ParticleSystemRender;

use crate::ParticleSystem;
use bevy::render::texture::GpuImage;
use bevy::{
    prelude::*,
    render::{
        render_graph::{self},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
    },
    utils::HashMap,
};

#[derive(Resource, Clone)]
pub struct ParticleRenderPipeline {
    bind_group_layout: BindGroupLayout,
    clear_pipeline: CachedComputePipelineId,
    render_pipeline: CachedComputePipelineId,
}

pub struct RenderParticlesNode {
    particle_systems: QueryState<Entity, With<ParticleSystem>>,
    render_state: HashMap<Entity, ParticleRenderState>,
}

#[derive(Default, Clone, Debug)]
enum ParticleRenderState {
    #[default]
    Loading,
    Render,
}

fn bind_group_layout() -> BindGroupLayoutDescriptor<'static> {
    BindGroupLayoutDescriptor {
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
    }
}

pub fn render_bind_group(
    entity: Entity,
    render_device: &RenderDevice,
    render_pipeline: &ParticleRenderPipeline,
    particle_system_render: &ParticleSystemRender,
    view: &GpuImage,
) -> BindGroup {
    render_device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &render_pipeline.bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(
                    particle_system_render.particle_buffers[&entity].as_entire_buffer_binding(),
                ),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&view.texture_view),
            },
        ],
    })
}

impl FromWorld for ParticleRenderPipeline {
    fn from_world(world: &mut World) -> Self {
        let bind_group_layout = world
            .resource::<RenderDevice>()
            .create_bind_group_layout(&bind_group_layout());
        let shader = world.resource::<AssetServer>().load("particle_render.wgsl");
        let mut pipeline_cache = world.resource_mut::<PipelineCache>();

        let render_pipeline = pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
            shader.clone(),
            "render",
            &bind_group_layout,
        ));

        let clear_pipeline = pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
            shader,
            "clear",
            &bind_group_layout,
        ));

        ParticleRenderPipeline {
            bind_group_layout,
            clear_pipeline,
            render_pipeline,
        }
    }
}

impl render_graph::Node for RenderParticlesNode {
    fn update(&mut self, world: &mut World) {
        let mut systems = world.query_filtered::<Entity, With<ParticleSystem>>();
        let pipeline = world.resource::<ParticleRenderPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        for entity in systems.iter(world) {
            self.update_state(entity, pipeline_cache, pipeline);
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

        for entity in self.particle_systems.iter_manual(world) {
            if let Some((clear_pipeline, render_pipeline)) = match self.render_state[&entity] {
                ParticleRenderState::Loading => None,
                ParticleRenderState::Render => {
                    Some((pipeline.clear_pipeline, pipeline.render_pipeline))
                }
            } {
                run_compute_pass_2d(
                    render_context,
                    &particle_systems_render.render_bind_group[&entity],
                    pipeline_cache,
                    clear_pipeline,
                );
                run_compute_pass(
                    render_context,
                    &particle_systems_render.render_bind_group[&entity],
                    pipeline_cache,
                    render_pipeline,
                );
            }
        }

        Ok(())
    }
}

impl RenderParticlesNode {
    pub fn new(world: &mut World) -> Self {
        Self {
            particle_systems: QueryState::new(world),
            render_state: HashMap::default(),
        }
    }
    fn update_state(
        &mut self,
        entity: Entity,
        pipeline_cache: &PipelineCache,
        pipeline: &ParticleRenderPipeline,
    ) {
        let render_state = match self.render_state.get(&entity) {
            Some(state) => state,
            None => {
                self.render_state
                    .insert(entity, ParticleRenderState::Loading);
                &ParticleRenderState::Loading
            }
        };
        match render_state {
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
}
