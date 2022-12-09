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

use crate::{
    compute_utils::{compute_pipeline_descriptor, run_compute_pass},
    ParticleSystem, ParticleSystemRender, PARTICLE_COUNT, WORKGROUP_SIZE,
};

#[derive(Resource, Clone)]
pub struct ParticleUpdatePipeline {
    pub bind_group_layout: BindGroupLayout,
    init_pipeline: CachedComputePipelineId,
    update_pipeline: CachedComputePipelineId,
}

pub struct UpdateParticlesNode {
    particle_systems: QueryState<(Entity, &'static ParticleSystem)>,
    //FIXME flush this when the entities no long exists, grows without bound if constantly creating and destroying spawners
    update_state: HashMap<Entity, ParticleUpdateState>,
}

#[derive(Default, Clone)]
enum ParticleUpdateState {
    #[default]
    Loading,
    Init,
    Update,
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
            /*
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            */
        ],
    }
}

impl FromWorld for ParticleUpdatePipeline {
    fn from_world(world: &mut World) -> Self {
        let bind_group_layout = world
            .resource::<RenderDevice>()
            .create_bind_group_layout(&bind_group_layout());

        let shader = world.resource::<AssetServer>().load("particle_update.wgsl");

        let mut pipeline_cache = world.resource_mut::<PipelineCache>();

        let init_pipeline = pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
            shader.clone(),
            "init",
            &bind_group_layout,
        ));

        let update_pipeline = pipeline_cache.queue_compute_pipeline(compute_pipeline_descriptor(
            shader,
            "update",
            &bind_group_layout,
        ));

        ParticleUpdatePipeline {
            bind_group_layout,
            init_pipeline,
            update_pipeline,
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
            self.update_state(entity, pipeline_cache, pipeline);
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
        let particle_systems_render = world.resource::<ParticleSystemRender>();

        //Am I using iter manual correctly?
        for (entity, system) in self.particle_systems.iter_manual(world) {
            // select the pipeline based on the current state
            if let Some(pipeline) = match self.update_state[&entity] {
                ParticleUpdateState::Loading => None,
                ParticleUpdateState::Init => Some(pipeline.init_pipeline),
                ParticleUpdateState::Update => Some(pipeline.update_pipeline),
            } {
                run_compute_pass(
                    render_context,
                    &particle_systems_render.update_bind_group[&entity],
                    pipeline_cache,
                    pipeline,
                );
            }
        }

        Ok(())
    }
}

impl UpdateParticlesNode {
    pub fn new(world: &mut World) -> Self {
        Self {
            particle_systems: QueryState::new(world),
            update_state: HashMap::default(),
        }
    }

    fn update_state(
        &mut self,
        entity: Entity,
        pipeline_cache: &PipelineCache,
        pipeline: &ParticleUpdatePipeline,
    ) {
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
}
