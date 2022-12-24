use std::{borrow::Cow, ops::Deref};

use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::{RenderContext, RenderDevice, RenderQueue},
    },
};
use wgpu::Maintain;

use crate::{Particle, HEIGHT, PARTICLE_COUNT, WIDTH, WORKGROUP_SIZE};

pub fn compute_pipeline_descriptor(
    shader: Handle<Shader>,
    entry_point: &str,
    bind_group_layout: &BindGroupLayout,
) -> ComputePipelineDescriptor {
    ComputePipelineDescriptor {
        label: None,
        layout: Some(vec![bind_group_layout.clone()]),
        shader,
        shader_defs: vec![],
        entry_point: Cow::from(entry_point.to_owned()),
    }
}

pub fn run_compute_pass(
    render_context: &mut RenderContext,
    bind_group: &BindGroup,
    pipeline_cache: &PipelineCache,
    pipeline: CachedComputePipelineId,
) {
    let mut pass = render_context
        .command_encoder
        .begin_compute_pass(&ComputePassDescriptor::default());

    pass.set_bind_group(0, bind_group, &[]);

    let pipeline = pipeline_cache.get_compute_pipeline(pipeline).unwrap();
    pass.set_pipeline(pipeline);

    pass.dispatch_workgroups(PARTICLE_COUNT / WORKGROUP_SIZE, 1, 1);
}

//ugh lazy dupe
pub fn run_compute_pass_2d(
    render_context: &mut RenderContext,
    bind_group: &BindGroup,
    pipeline_cache: &PipelineCache,
    pipeline: CachedComputePipelineId,
) {
    let mut pass = render_context
        .command_encoder
        .begin_compute_pass(&ComputePassDescriptor::default());

    pass.set_bind_group(0, bind_group, &[]);

    let pipeline = pipeline_cache.get_compute_pipeline(pipeline).unwrap();
    pass.set_pipeline(pipeline);

    pass.dispatch_workgroups(
        WIDTH as u32 / WORKGROUP_SIZE,
        HEIGHT as u32 / WORKGROUP_SIZE,
        1,
    );
}

// Helper function to print out gpu data for debugging
#[allow(dead_code)]
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
