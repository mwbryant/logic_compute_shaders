use crate::particle::Particle;
use crate::particle_config::ParticleConfig;
use crate::particle_render::{
    create_render_bind_group, ParticleRenderPipeline, RenderParticlesNode,
};
use crate::particle_ui::ParticleUiPlugin;
use crate::particle_update::{
    create_update_bind_group, ParticleUpdatePipeline, UpdateParticlesNode,
};
use crate::{HEIGHT, WIDTH};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_graph::RenderLabel;
use bevy::render::{graph, Extract, Render, RenderSet};
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
};
use rand::Rng;

#[derive(ExtractComponent, Component, Default, Clone)]
pub struct ParticleSystem {
    pub rendered_texture: Handle<Image>,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct UpdateParticlesRenderLabel;

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct RenderParticlesRenderLabel;

// Must maintain all our own data because render world flushes between frames :,(
#[derive(Resource, Default, Clone, ExtractResource)]
pub struct ParticleSystemRender {
    pub update_bind_group: Option<BindGroup>,
    pub render_bind_group: Option<BindGroup>,
    pub particle_buffer: Option<Buffer>,
    pub particle_config_buffer: Option<Buffer>,
    pub attraction_matrix_buffer: Option<Buffer>,
    pub delta_time_buffer: Option<Buffer>,
    pub spatial_indices_buffer: Option<Buffer>,
    pub spatial_offsets_buffer: Option<Buffer>,
}
pub struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleConfig>();
        app.init_resource::<Events<RecreateParticles>>();
        app.add_plugins((
            ExtractComponentPlugin::<ParticleSystem>::default(),
            ExtractResourcePlugin::<ParticleConfig>::default(),
            ParticleUiPlugin,
        ));

        let render_app = app.sub_app_mut(RenderApp);
        render_app
            .init_resource::<Events<ExtractedRecreateParticles>>()
            .add_systems(ExtractSchedule, extract_recreate_particles_event)
            .add_systems(Render, queue_bind_group.in_set(RenderSet::Queue));

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

#[derive(Event)]
pub struct RecreateParticles;

#[derive(Event)]
pub struct ExtractedRecreateParticles;

pub fn extract_recreate_particles_event(
    mut event_reader: Extract<EventReader<RecreateParticles>>,
    mut event_writer: EventWriter<ExtractedRecreateParticles>,
) {
    for _ in event_reader.read() {
        event_writer.send(ExtractedRecreateParticles);
    }
}

pub fn generate_particles(n: u32, m: u32) -> Vec<Particle> {
    let mut rng = rand::thread_rng();
    (0..n)
        .map(|_| {
            let velocity = Vec2::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0));
            let position = Vec2::new(
                rng.gen_range(0.0..WIDTH as f32),
                rng.gen_range(0.0..HEIGHT as f32),
            );
            let particle_type = rng.gen_range(0..m as u32);

            Particle {
                velocity,
                position,
                particle_type,
            }
        })
        .collect()
}

fn queue_bind_group(
    gpu_images: Res<RenderAssets<Image>>,
    particle_config: Res<ParticleConfig>,
    particle_systems: Query<&ParticleSystem>,
    mut particle_system_render: ResMut<ParticleSystemRender>,
    render_device: Res<RenderDevice>,
    render_pipeline: Res<ParticleRenderPipeline>,
    time: Res<Time>,
    update_pipeline: Res<ParticleUpdatePipeline>,
    mut event_reader: EventReader<ExtractedRecreateParticles>,
) {
    if let Ok(system) = particle_systems.get_single() {
        let recreate = event_reader.read().next().is_some();

        let (shader_particle_config, attraction_matrix) =
            particle_config.extract_shader_variables();

        if particle_system_render.particle_buffer.is_none() || recreate {
            debug!(
                "Creating particle buffer with {} particles",
                particle_config.n
            );
            let particles = generate_particles(shader_particle_config.n, shader_particle_config.m);
            let mut byte_buffer = Vec::new();
            let mut buffer = encase::StorageBuffer::new(&mut byte_buffer);
            buffer.write(&particles).unwrap();

            let storage = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: None,
                usage: BufferUsages::COPY_DST | BufferUsages::STORAGE | BufferUsages::COPY_SRC,
                contents: buffer.into_inner(),
            });

            particle_system_render.particle_buffer = Some(storage);
        }

        if particle_system_render.particle_config_buffer.is_none() || recreate {
            debug!("Creating particle config buffer");
            let mut byte_buffer = Vec::new();
            let mut buffer = encase::UniformBuffer::new(&mut byte_buffer);
            buffer.write(&shader_particle_config).unwrap();

            let storage = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: None,
                usage: BufferUsages::COPY_DST | BufferUsages::UNIFORM,
                contents: buffer.into_inner(),
            });

            particle_system_render.particle_config_buffer = Some(storage);
        }

        if particle_system_render.attraction_matrix_buffer.is_none() || recreate {
            debug!("Creating attraction matrix buffer");
            let mut byte_buffer = Vec::new();
            let mut buffer = encase::StorageBuffer::new(&mut byte_buffer);
            buffer.write(&attraction_matrix).unwrap();

            let storage = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: None,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC, // tut das COPY_SRC not?
                contents: buffer.into_inner(),
            });

            particle_system_render.attraction_matrix_buffer = Some(storage);
        }

        if particle_system_render.delta_time_buffer.is_none() || recreate {
            debug!("Creating delta time buffer");
            let mut byte_buffer = Vec::new();
            let mut buffer = encase::UniformBuffer::new(&mut byte_buffer);
            buffer.write(&time.delta_seconds()).unwrap();

            let storage = render_device.create_buffer_with_data(&BufferInitDescriptor {
                label: None,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST, // ausprobieren ob uniform nicht reicht.
                contents: buffer.into_inner(),
            });

            particle_system_render.delta_time_buffer = Some(storage);
        }

        if particle_system_render.spatial_indices_buffer.is_none() || recreate {
            debug!("Creating spatial indices buffer");

            let storage = render_device.create_buffer(&BufferDescriptor {
                label: None,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                size: (particle_config.n * 16) as u64, // 3 components × 4 bytes per component = 12 bytes
                mapped_at_creation: false,
            });

            particle_system_render.spatial_indices_buffer = Some(storage);
        }

        if particle_system_render.spatial_offsets_buffer.is_none() || recreate {
            debug!("Creating spatial offsets buffer");

            let storage = render_device.create_buffer(&BufferDescriptor {
                label: None,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                size: (particle_config.n * 4) as u64,
                mapped_at_creation: false,
            });

            particle_system_render.spatial_offsets_buffer = Some(storage);
        }

        // read_buffer(
        //     &particle_system_render.particle_buffers[&entity],
        //     &render_device,
        //     &render_queue,
        // );

        if particle_system_render.update_bind_group.is_none() || recreate {
            let update_bind_group =
                create_update_bind_group(&render_device, &update_pipeline, &particle_system_render);
            particle_system_render.update_bind_group = Some(update_bind_group);
        }

        if particle_system_render.render_bind_group.is_none() || recreate {
            if let Some(view) = &gpu_images.get(&system.rendered_texture) {
                let render_bind_group = create_render_bind_group(
                    &render_device,
                    &render_pipeline,
                    &particle_system_render,
                    view,
                );

                particle_system_render.render_bind_group = Some(render_bind_group);
            }
        }
    }
}
