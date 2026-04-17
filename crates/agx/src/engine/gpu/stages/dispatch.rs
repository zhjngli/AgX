//! Shared GPU compute dispatch helpers.

use crate::engine::gpu::runtime::GpuRuntime;

/// Dispatch a compute shader with 2 buffer bindings.
pub(crate) fn dispatch_2buf(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    buf0: &wgpu::Buffer,
    buf1: &wgpu::Buffer,
    label: &str,
    wg: (u32, u32),
) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buf1.as_entire_binding(),
                },
            ],
        });
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(wg.0, wg.1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Dispatch a compute shader with 3 buffer bindings.
pub(crate) fn dispatch_3buf(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    buf0: &wgpu::Buffer,
    buf1: &wgpu::Buffer,
    buf2: &wgpu::Buffer,
    label: &str,
    wg: (u32, u32),
) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buf1.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buf2.as_entire_binding(),
                },
            ],
        });
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(wg.0, wg.1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Dispatch a compute shader with 4 buffer bindings.
#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_4buf(
    runtime: &GpuRuntime,
    pipeline: &wgpu::ComputePipeline,
    buf0: &wgpu::Buffer,
    buf1: &wgpu::Buffer,
    buf2: &wgpu::Buffer,
    buf3: &wgpu::Buffer,
    label: &str,
    wg: (u32, u32),
) {
    let bind_group = runtime
        .device
        .create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(label),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: buf1.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: buf2.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: buf3.as_entire_binding(),
                },
            ],
        });
    let mut encoder = runtime
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(wg.0, wg.1, 1);
    }
    runtime.queue.submit(std::iter::once(encoder.finish()));
}

/// Separable Gaussian blur: horizontal pass (lum_buffer → temp_buffer).
pub(crate) fn dispatch_blur_h(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let wg = runtime.workgroup_counts();
    dispatch_4buf(
        runtime,
        pipeline,
        &runtime.lum_buffer,
        &runtime.temp_buffer,
        &runtime.kernel_buffer,
        &runtime.params_buffer,
        "blur_horizontal",
        wg,
    );
}

/// Separable Gaussian blur: vertical pass (temp_buffer → blur_buffer).
pub(crate) fn dispatch_blur_v(runtime: &GpuRuntime, pipeline: &wgpu::ComputePipeline) {
    let wg = runtime.workgroup_counts();
    dispatch_4buf(
        runtime,
        pipeline,
        &runtime.temp_buffer,
        &runtime.blur_buffer,
        &runtime.kernel_buffer,
        &runtime.params_buffer,
        "blur_vertical",
        wg,
    );
}
