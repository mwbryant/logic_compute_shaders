use crate::compute_utils::{compute_pipeline_descriptor, run_compute_pass, run_compute_pass_2d};
use crate::particle_config::ParticleConfig;
use crate::particle_system::ParticleSystemRender;

use crate::{ParticleSystem, WORKGROUP_SIZE};
use bevy::render::texture::GpuImage;
use bevy::{
    prelude::*,
    render::{
        render_graph::{self},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
    },
};

#[derive(Resource, Clone)]
pub struct ParticleRenderPipeline {
    bind_group_layout: BindGroupLayout,
    clear_pipeline: CachedComputePipelineId,
    render_pipeline: CachedComputePipelineId,
}

pub struct RenderParticlesNode {
    particle_system: QueryState<Entity, With<ParticleSystem>>,
    render_state: ParticleRenderState,
}

#[derive(Default, Clone, Debug)]
enum ParticleRenderState {
    #[default]
    Loading,
    Render,
}

pub fn create_render_bind_group(
    render_device: &RenderDevice,
    render_pipeline: &ParticleRenderPipeline,
    particle_system_render: &ParticleSystemRender,
    view: &GpuImage,
) -> BindGroup {
    render_device.create_bind_group(
        None,
        &render_pipeline.bind_group_layout,
        &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::Buffer(
                    particle_system_render
                        .particle_buffer
                        .as_ref()
                        .unwrap()
                        .as_entire_buffer_binding(),
                ),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Buffer(
                    particle_system_render
                        .particle_config_buffer
                        .as_ref()
                        .unwrap()
                        .as_entire_buffer_binding(),
                ),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::TextureView(&view.texture_view),
            },
        ],
    )
}

impl FromWorld for ParticleRenderPipeline {
    fn from_world(world: &mut World) -> Self {
        let bind_group_layout = world.resource::<RenderDevice>().create_bind_group_layout(
            "render_bind_group_layout",
            &[
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
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform {},
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::ReadWrite,
                        format: TextureFormat::Rgba8Unorm,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        );

        let shader = world.resource::<AssetServer>().load("particle_render.wgsl");
        let pipeline_cache = world.resource_mut::<PipelineCache>();
        let shader_defs = vec![ShaderDefVal::UInt("WORKGROUP_SIZE".into(), WORKGROUP_SIZE)];

        let render_pipeline = pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
            shader.clone(),
            "render",
            &bind_group_layout,
            shader_defs.clone(),
        ));

        let clear_pipeline = pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
            shader,
            "clear",
            &bind_group_layout,
            shader_defs,
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

        self.particle_system.update_archetypes(world);
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
        let particle_config = world.resource::<ParticleConfig>();

        for _ in self.particle_system.iter_manual(world) {
            if let Some((clear_pipeline, render_pipeline)) = match self.render_state {
                ParticleRenderState::Loading => None,
                ParticleRenderState::Render => {
                    Some((pipeline.clear_pipeline, pipeline.render_pipeline))
                }
            } {
                run_compute_pass_2d(
                    render_context,
                    &particle_systems_render.render_bind_group.as_ref().unwrap(),
                    pipeline_cache,
                    clear_pipeline,
                );

                run_compute_pass(
                    render_context,
                    &particle_systems_render.render_bind_group.as_ref().unwrap(),
                    pipeline_cache,
                    render_pipeline,
                    particle_config.n as u32,
                );
            }
        }

        Ok(())
    }
}

impl RenderParticlesNode {
    pub fn new(world: &mut World) -> Self {
        Self {
            particle_system: QueryState::new(world),
            render_state: ParticleRenderState::default(),
        }
    }
    fn update_state(
        &mut self,
        entity: Entity,
        pipeline_cache: &PipelineCache,
        pipeline: &ParticleRenderPipeline,
    ) {
        match self.render_state {
            ParticleRenderState::Loading => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.render_pipeline)
                {
                    self.render_state = ParticleRenderState::Render;
                }
            }

            ParticleRenderState::Render => {}
        }
    }
}
