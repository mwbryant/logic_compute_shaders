use crate::particle_render::{render_bind_group, ParticleRenderPipeline, RenderParticlesNode};
use crate::particle_update::{update_bind_group, ParticleUpdatePipeline, UpdateParticlesNode};
use crate::{Particle, ParticleSystem, PARTICLE_COUNT};
use bevy::render::render_graph::RenderLabel;
use bevy::render::{graph, Render, RenderSet};
use bevy::{
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_asset::RenderAssets,
        render_graph::RenderGraph,
        render_resource::*,
        renderer::RenderDevice,
        RenderApp,
    },
    utils::HashMap,
};

pub struct ParticlePlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct UpdateParticlesRenderLabel;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct RenderParticlesRenderLabel;

// Must maintain all our own data because render world flushes between frames :,(
#[derive(Resource, Default)]
pub struct ParticleSystemRender {
    pub update_bind_group: HashMap<Entity, BindGroup>,
    pub render_bind_group: HashMap<Entity, BindGroup>,
    pub particle_buffers: HashMap<Entity, Buffer>,
}

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<ParticleSystem>::default());

        let render_app = app.sub_app_mut(RenderApp);
        render_app.add_systems(Render, queue_bind_group.in_set(RenderSet::Queue));

        let update_node = UpdateParticlesNode::new(&mut render_app.world);
        let render_node = RenderParticlesNode::new(&mut render_app.world);

        let mut render_graph = render_app.world.resource_mut::<RenderGraph>();

        render_graph.add_node(UpdateParticlesRenderLabel, update_node);
        render_graph.add_node(RenderParticlesRenderLabel, render_node);

        render_graph.add_node_edge(UpdateParticlesRenderLabel, RenderParticlesRenderLabel);
        render_graph.add_node_edge(RenderParticlesRenderLabel, graph::CameraDriverLabel);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<ParticleUpdatePipeline>()
            .init_resource::<ParticleSystemRender>()
            .init_resource::<ParticleRenderPipeline>();
    }
}

fn queue_bind_group(
    render_device: Res<RenderDevice>,
    //render_queue: Res<RenderQueue>,
    render_pipeline: Res<ParticleRenderPipeline>,
    gpu_images: Res<RenderAssets<Image>>,
    mut particle_system_render: ResMut<ParticleSystemRender>,
    update_pipeline: Res<ParticleUpdatePipeline>,
    //Getting mutable queries in the render world is an antipattern?
    particle_systems: Query<(Entity, &ParticleSystem)>,
) {
    // Everything here is done lazily and should only happen on the first call here.
    for (entity, system) in &particle_systems {
        if !particle_system_render
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

            particle_system_render
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

        if !particle_system_render
            .update_bind_group
            .contains_key(&entity)
        {
            let update_group = update_bind_group(
                entity,
                &render_device,
                &update_pipeline,
                &particle_system_render,
            );
            particle_system_render
                .update_bind_group
                .insert(entity, update_group);
        }

        if !particle_system_render
            .render_bind_group
            .contains_key(&entity)
        {
            let Some(view) = &gpu_images.get(&system.rendered_texture) else {
                continue;
            };
            let render_group = render_bind_group(
                entity,
                &render_device,
                &render_pipeline,
                &particle_system_render,
                view,
            );

            particle_system_render
                .render_bind_group
                .insert(entity, render_group);
        }
    }
}

impl ExtractComponent for ParticleSystem {
    type QueryData = &'static ParticleSystem;
    type QueryFilter = ();
    type Out = Self;

    fn extract_component(item: bevy::ecs::query::QueryItem<'_, Self::QueryData>) -> Option<Self> {
        Some(item.clone())
    }
}
