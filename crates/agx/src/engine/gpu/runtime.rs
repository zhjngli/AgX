//! GPU device, queue, and buffer management.

use crate::error::AgxError;

/// Manages wgpu resources for GPU-accelerated rendering.
///
/// Created once per `Engine` and reused across renders. Owns the device,
/// queue, pixel buffer, params buffer, and staging buffer.
pub struct GpuRuntime {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    /// GPU-side pixel buffer (storage, read/write).
    pub(crate) pixel_buffer: wgpu::Buffer,
    /// GPU-side uniform buffer for [`super::params::GpuParameters`].
    pub(crate) params_buffer: wgpu::Buffer,
    /// GPU-side staging buffer for reading pixels back to CPU.
    pub(crate) staging_buffer: wgpu::Buffer,
    /// Tone curve lookup tables (5 x 256 entries).
    pub(crate) tone_curve_buffer: wgpu::Buffer,
    /// Optional 3D LUT texture.
    pub(crate) lut_texture: Option<wgpu::Texture>,
    /// Optional 3D LUT texture view.
    pub(crate) lut_texture_view: Option<wgpu::TextureView>,
    /// Optional 3D LUT sampler.
    pub(crate) lut_sampler: Option<wgpu::Sampler>,
    /// Fallback 1x1x1 identity LUT texture view (used when no LUT loaded).
    pub(crate) fallback_lut_view: wgpu::TextureView,
    /// Fallback LUT sampler.
    pub(crate) fallback_lut_sampler: wgpu::Sampler,
    /// Single-channel luminance buffer for detail/blur operations.
    pub(crate) lum_buffer: wgpu::Buffer,
    /// Single-channel temp buffer for separable blur intermediate.
    pub(crate) temp_buffer: wgpu::Buffer,
    /// Single-channel buffer for blurred luminance (detail apply reads this).
    pub(crate) blur_buffer: wgpu::Buffer,
    /// Storage buffer for Gaussian blur kernel weights.
    pub(crate) kernel_buffer: wgpu::Buffer,
    /// Single-channel accumulator for denoise wavelet reconstruction.
    pub(crate) denoise_accum_buffer: wgpu::Buffer,
    /// Scratch buffers for multi-pass stages (dehaze guided filter, etc.).
    pub(crate) scratch_a: wgpu::Buffer,
    pub(crate) scratch_b: wgpu::Buffer,
    pub(crate) scratch_c: wgpu::Buffer,
    pub(crate) scratch_d: wgpu::Buffer,
    /// Image width in pixels.
    pub(crate) width: u32,
    /// Image height in pixels.
    pub(crate) height: u32,
}

impl GpuRuntime {
    /// Create a new GPU runtime for images of the given dimensions.
    pub fn new(width: u32, height: u32) -> Result<Self, AgxError> {
        Self::new_inner(width, height, false)
    }

    /// Create a GPU runtime using wgpu's software fallback adapter.
    pub fn new_fallback(width: u32, height: u32) -> Result<Self, AgxError> {
        Self::new_inner(width, height, true)
    }

    fn new_inner(width: u32, height: u32, force_fallback: bool) -> Result<Self, AgxError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: force_fallback,
        }))
        .ok_or_else(|| AgxError::Gpu("no GPU adapter found".into()))?;

        let adapter_limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("agx-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits {
                    max_buffer_size: adapter_limits.max_buffer_size,
                    max_storage_buffer_binding_size: adapter_limits.max_storage_buffer_binding_size,
                    ..wgpu::Limits::default()
                },
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| AgxError::Gpu(format!("device creation failed: {e}")))?;

        let pixel_count = (width as u64) * (height as u64);
        // 3 floats per pixel, 4 bytes per float
        let buffer_size = pixel_count * 3 * 4;

        let limits = device.limits();
        let max_buf = limits.max_buffer_size;
        let max_binding = limits.max_storage_buffer_binding_size as u64;
        let effective_limit = max_buf.min(max_binding);
        if buffer_size > effective_limit {
            return Err(AgxError::Gpu(format!(
                "image too large for GPU: pixel buffer {buffer_size} bytes exceeds device limit {effective_limit}"
            )));
        }

        let pixel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params_buffer"),
            size: std::mem::size_of::<super::params::GpuParameters>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let single_channel_size = pixel_count * 4; // 1 f32 per pixel
        let lum_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lum_buffer"),
            size: single_channel_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let temp_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("temp_buffer"),
            size: single_channel_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let blur_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("blur_buffer"),
            size: single_channel_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let kernel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kernel_buffer"),
            size: 512 * 4, // max 512 kernel weights
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let denoise_accum_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("denoise_accum_buffer"),
            size: single_channel_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let scratch_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let scratch_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scratch_a"),
            size: single_channel_size,
            usage: scratch_usage,
            mapped_at_creation: false,
        });
        let scratch_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scratch_b"),
            size: single_channel_size,
            usage: scratch_usage,
            mapped_at_creation: false,
        });
        let scratch_c = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scratch_c"),
            size: single_channel_size,
            usage: scratch_usage,
            mapped_at_creation: false,
        });
        let scratch_d = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scratch_d"),
            size: single_channel_size,
            usage: scratch_usage,
            mapped_at_creation: false,
        });

        let tone_curve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("tone_curve_buffer"),
            size: (5 * 256 * 4) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // 1x1x1 identity LUT (passthrough) — uses Rgba16Float for filterable sampling
        let fallback_lut = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("fallback_lut"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        // Write a single [0,0,0,1] texel — identity doesn't matter since lut_active will be 0
        let fallback_data: [half::f16; 4] = [
            half::f16::from_f32(0.0),
            half::f16::from_f32(0.0),
            half::f16::from_f32(0.0),
            half::f16::from_f32(1.0),
        ];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &fallback_lut,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&fallback_data),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let fallback_lut_view = fallback_lut.create_view(&wgpu::TextureViewDescriptor::default());
        let fallback_lut_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("fallback_lut_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Ok(Self {
            device,
            queue,
            pixel_buffer,
            params_buffer,
            staging_buffer,
            tone_curve_buffer,
            lut_texture: None,
            lut_texture_view: None,
            lut_sampler: None,
            fallback_lut_view,
            fallback_lut_sampler,
            lum_buffer,
            temp_buffer,
            blur_buffer,
            kernel_buffer,
            denoise_accum_buffer,
            scratch_a,
            scratch_b,
            scratch_c,
            scratch_d,
            width,
            height,
        })
    }

    /// Upload pixel data from CPU to GPU.
    /// Input: flat slice of `[r, g, b]` f32 triples in row-major order.
    pub fn upload_pixels(&self, pixels: &[[f32; 3]]) {
        let bytes: &[u8] = bytemuck::cast_slice(pixels);
        self.queue.write_buffer(&self.pixel_buffer, 0, bytes);
    }

    /// Upload Gaussian kernel weights to the kernel buffer.
    pub fn upload_kernel(&self, kernel: &[f32]) {
        self.queue
            .write_buffer(&self.kernel_buffer, 0, bytemuck::cast_slice(kernel));
    }

    /// Upload a [`GpuParameters`](super::params::GpuParameters) struct to the uniform buffer.
    pub fn upload_params(&self, params: &super::params::GpuParameters) {
        let bytes: &[u8] = bytemuck::bytes_of(params);
        self.queue.write_buffer(&self.params_buffer, 0, bytes);
    }

    /// Upload precomputed tone curve LUTs to GPU.
    pub fn upload_tone_curves(&self, data: &[f32; 1280]) {
        self.queue
            .write_buffer(&self.tone_curve_buffer, 0, bytemuck::cast_slice(data));
    }

    /// Upload a 3D LUT as a GPU texture with trilinear sampling.
    ///
    /// Uses `Rgba16Float` format for filterable texture sampling.
    /// Half-float precision is more than sufficient for LUT entries in 0.0-1.0 range.
    pub fn upload_lut(&mut self, lut: &crate::lut::Lut3D) {
        let size = lut.size as u32;
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lut_3d"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: size,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let rgba: Vec<[half::f16; 4]> = lut
            .table
            .iter()
            .map(|rgb| {
                [
                    half::f16::from_f32(rgb[0]),
                    half::f16::from_f32(rgb[1]),
                    half::f16::from_f32(rgb[2]),
                    half::f16::from_f32(1.0),
                ]
            })
            .collect();
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&rgba),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size * 8),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: size,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lut_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        self.lut_texture = Some(texture);
        self.lut_texture_view = Some(view);
        self.lut_sampler = Some(sampler);
    }

    /// Download pixel data from GPU to CPU.
    /// Returns a `Vec<[f32; 3]>` in row-major order.
    pub fn download_pixels(&self) -> Vec<[f32; 3]> {
        let buffer_size = self.pixel_buffer.size();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("download_encoder"),
            });
        encoder.copy_buffer_to_buffer(&self.pixel_buffer, 0, &self.staging_buffer, 0, buffer_size);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = self.staging_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("GPU channel closed")
            .expect("GPU buffer map failed");

        let data = slice.get_mapped_range();
        let pixels: Vec<[f32; 3]> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        self.staging_buffer.unmap();

        pixels
    }

    /// Download a single-channel f32 buffer from GPU to CPU.
    pub(crate) fn download_single_channel(&self, src: &wgpu::Buffer) -> Vec<f32> {
        let size = (self.pixel_count() as u64) * 4;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("download_single_channel"),
            });
        encoder.copy_buffer_to_buffer(src, 0, &self.staging_buffer, 0, size);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = self.staging_buffer.slice(..size);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("GPU channel closed")
            .expect("GPU buffer map failed");

        let data = slice.get_mapped_range();
        let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        self.staging_buffer.unmap();

        result
    }

    /// Total number of pixels in the image.
    pub fn pixel_count(&self) -> u32 {
        self.width * self.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu_available() -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .is_some()
    }

    #[test]
    fn upload_download_roundtrip() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter found");
            return;
        }
        let runtime = GpuRuntime::new(2, 2).unwrap();
        let pixels = vec![
            [0.1, 0.2, 0.3],
            [0.4, 0.5, 0.6],
            [0.7, 0.8, 0.9],
            [1.0, 0.0, 0.5],
        ];
        runtime.upload_pixels(&pixels);
        let result = runtime.download_pixels();
        assert_eq!(result.len(), 4);
        for (i, (a, b)) in pixels.iter().zip(result.iter()).enumerate() {
            for c in 0..3 {
                assert!(
                    (a[c] - b[c]).abs() < 1e-6,
                    "pixel[{i}][{c}]: expected {}, got {}",
                    a[c],
                    b[c]
                );
            }
        }
    }

    #[test]
    fn runtime_creation_stores_dimensions() {
        if !gpu_available() {
            eprintln!("skipping: no GPU adapter found");
            return;
        }
        let runtime = GpuRuntime::new(100, 200).unwrap();
        assert_eq!(runtime.width, 100);
        assert_eq!(runtime.height, 200);
        assert_eq!(runtime.pixel_count(), 20_000);
    }
}
