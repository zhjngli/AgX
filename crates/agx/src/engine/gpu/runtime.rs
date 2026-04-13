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
    /// GPU-side staging buffer for reading pixels back to CPU.
    pub(crate) staging_buffer: wgpu::Buffer,
    /// Image width in pixels.
    pub(crate) width: u32,
    /// Image height in pixels.
    pub(crate) height: u32,
}

impl GpuRuntime {
    /// Create a new GPU runtime for images of the given dimensions.
    pub fn new(width: u32, height: u32) -> Result<Self, AgxError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .ok_or_else(|| AgxError::Gpu("no GPU adapter found".into()))?;

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("agx-gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .map_err(|e| AgxError::Gpu(format!("device creation failed: {e}")))?;

        let pixel_count = (width as u64) * (height as u64);
        // 3 floats per pixel, 4 bytes per float
        let buffer_size = pixel_count * 3 * 4;

        let pixel_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pixel_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("staging_buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            pixel_buffer,
            staging_buffer,
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
