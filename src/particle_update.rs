use bevy::{
    prelude::*,
    render::{
        render_graph::{self},
        render_resource::*,
        renderer::{RenderContext, RenderDevice},
    },
};

use crate::{
    compute_utils::{compute_pipeline_descriptor, run_compute_pass},
    particle_config::ParticleConfig,
    particle_system::ParticleSystemRender,
    ParticleSystem, HEIGHT, WIDTH, WORKGROUP_SIZE,
};

#[derive(Resource, Clone)]
pub struct ParticleUpdatePipeline {
    bind_group_layout: BindGroupLayout,
    update_positions_pipeline: CachedComputePipelineId,
    update_velocities_pipeline: CachedComputePipelineId,
    update_spatial_hash_grid_pipeline: CachedComputePipelineId,
}

pub struct UpdateParticlesNode {
    particle_system: QueryState<Entity, With<ParticleSystem>>,
    update_state: ParticleUpdateState,
}

#[derive(Default, Clone)]
enum ParticleUpdateState {
    #[default]
    Loading,
    UpdateVelocities,
    UpdatePositions,
    UpdateSpatialHashGrid,
}

pub fn create_update_bind_group(
    render_device: &RenderDevice,
    update_pipeline: &ParticleUpdatePipeline,
    particle_system_render: &ParticleSystemRender,
) -> BindGroup {
    render_device.create_bind_group(
        None,
        &update_pipeline.bind_group_layout,
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
                resource: BindingResource::Buffer(
                    particle_system_render
                        .attraction_matrix_buffer
                        .as_ref()
                        .unwrap()
                        .as_entire_buffer_binding(),
                ),
            },
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::Buffer(
                    particle_system_render
                        .delta_time_buffer
                        .as_ref()
                        .unwrap()
                        .as_entire_buffer_binding(),
                ),
            },
            BindGroupEntry {
                binding: 4,
                resource: BindingResource::Buffer(
                    particle_system_render
                        .spatial_indices_buffer
                        .as_ref()
                        .unwrap()
                        .as_entire_buffer_binding(),
                ),
            },
            BindGroupEntry {
                binding: 5,
                resource: BindingResource::Buffer(
                    particle_system_render
                        .spatial_offsets_buffer
                        .as_ref()
                        .unwrap()
                        .as_entire_buffer_binding(),
                ),
            },
        ],
    )
}

impl FromWorld for ParticleUpdatePipeline {
    fn from_world(world: &mut World) -> Self {
        let bind_group_layout = world.resource::<RenderDevice>().create_bind_group_layout(
            "update_bind_group_layout",
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
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform {},
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        );

        let shader = world.resource::<AssetServer>().load("particle_update.wgsl");

        let pipeline_cache = world.resource_mut::<PipelineCache>();

        let shader_defs = vec![
            ShaderDefVal::UInt("WIDTH".into(), (WIDTH as u32).into()),
            ShaderDefVal::UInt("HEIGHT".into(), (HEIGHT as u32).into()),
            ShaderDefVal::UInt("WORKGROUP_SIZE".into(), WORKGROUP_SIZE),
        ];

        let update_velocities_pipeline =
            pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
                shader.clone(),
                "update_velocities",
                &bind_group_layout,
                shader_defs.clone(),
            ));

        let update_positions_pipeline =
            pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
                shader.clone(),
                "update_positions",
                &bind_group_layout,
                shader_defs.clone(),
            ));

        let update_spatial_hash_grid_pipeline =
            pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
                shader,
                "update_spatial_hash_grid",
                &bind_group_layout,
                shader_defs,
            ));

        ParticleUpdatePipeline {
            bind_group_layout,
            update_velocities_pipeline,
            update_positions_pipeline,
            update_spatial_hash_grid_pipeline,
        }
    }
}

impl render_graph::Node for UpdateParticlesNode {
    fn update(&mut self, world: &mut World) {
        let mut systems = world.query_filtered::<Entity, With<ParticleSystem>>();
        let pipeline = world.resource::<ParticleUpdatePipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();

        if systems.get_single(world).is_ok() {
            // if the corresponding pipeline has loaded, transition to the next stage
            self.update_state(pipeline_cache, pipeline);
        }
        // Update the query for the run step
        self.particle_system.update_archetypes(world);
    }

    fn run(
        &self,
        _graph: &mut render_graph::RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let pipeline_cache = world.resource::<PipelineCache>();
        let pipeline = world.resource::<ParticleUpdatePipeline>();
        let particle_system_render = world.resource::<ParticleSystemRender>();
        let particle_config = world.resource::<ParticleConfig>();

        for _ in self.particle_system.iter_manual(world) {
            if let Some(pipeline) = match self.update_state {
                ParticleUpdateState::Loading => None,
                ParticleUpdateState::UpdateVelocities => Some(pipeline.update_velocities_pipeline),
                ParticleUpdateState::UpdatePositions => Some(pipeline.update_positions_pipeline),
                ParticleUpdateState::UpdateSpatialHashGrid => {
                    Some(pipeline.update_spatial_hash_grid_pipeline)
                }
            } {
                run_compute_pass(
                    render_context,
                    &particle_system_render.update_bind_group.as_ref().unwrap(),
                    pipeline_cache,
                    pipeline,
                    particle_config.n as u32,
                );
            }
        }

        Ok(())
    }
}

impl UpdateParticlesNode {
    pub fn new(world: &mut World) -> Self {
        Self {
            particle_system: QueryState::new(world),
            update_state: ParticleUpdateState::default(),
        }
    }

    fn update_state(&mut self, pipeline_cache: &PipelineCache, pipeline: &ParticleUpdatePipeline) {
        match self.update_state {
            ParticleUpdateState::Loading => {
                if let CachedPipelineState::Ok(_) = pipeline_cache
                    .get_compute_pipeline_state(pipeline.update_spatial_hash_grid_pipeline)
                {
                    self.update_state = ParticleUpdateState::UpdateSpatialHashGrid;
                }
            }
            ParticleUpdateState::UpdateSpatialHashGrid => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update_velocities_pipeline)
                {
                    self.update_state = ParticleUpdateState::UpdateVelocities;
                }
            }
            ParticleUpdateState::UpdateVelocities => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update_positions_pipeline)
                {
                    self.update_state = ParticleUpdateState::UpdatePositions;
                }
            }
            ParticleUpdateState::UpdatePositions => {
                if let CachedPipelineState::Ok(_) =
                    pipeline_cache.get_compute_pipeline_state(pipeline.update_velocities_pipeline)
                {
                    self.update_state = ParticleUpdateState::UpdateVelocities;
                }
            }
        }
    }
}
