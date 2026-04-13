//! WGSL shader compilation and caching.

use std::borrow::Cow;
use std::collections::HashMap;

use naga_oil::compose::{ComposableModuleDescriptor, Composer, NagaModuleDescriptor};

use crate::error::AgxError;

/// Compiles and caches WGSL compute pipelines.
pub struct ShaderCache {
    pipelines: HashMap<String, wgpu::ComputePipeline>,
}

impl ShaderCache {
    /// Create a new shader cache and compile all stage shaders.
    pub fn new(device: &wgpu::Device) -> Result<Self, AgxError> {
        let mut cache = Self {
            pipelines: HashMap::new(),
        };
        cache.compile_all(device)?;
        Ok(cache)
    }

    /// Get a compiled compute pipeline by shader name.
    pub fn get(&self, name: &str) -> Option<&wgpu::ComputePipeline> {
        self.pipelines.get(name)
    }

    fn compile_all(&mut self, device: &wgpu::Device) -> Result<(), AgxError> {
        let mut composer = Composer::default();

        // Register common modules
        Self::add_module(
            &mut composer,
            "common::math",
            include_str!("../../shaders/common/math.wgsl"),
        )?;
        Self::add_module(
            &mut composer,
            "common::color",
            include_str!("../../shaders/common/color.wgsl"),
        )?;
        Self::add_module(
            &mut composer,
            "common::tone",
            include_str!("../../shaders/common/tone.wgsl"),
        )?;

        // Compile stage shaders
        let stage_shaders = [
            (
                "linear_to_srgb",
                include_str!("../../shaders/linear_to_srgb.wgsl"),
            ),
            (
                "srgb_to_linear",
                include_str!("../../shaders/srgb_to_linear.wgsl"),
            ),
            (
                "linear_adjustments",
                include_str!("../../shaders/linear_adjustments.wgsl"),
            ),
            (
                "gamma_adjustments",
                include_str!("../../shaders/gamma_adjustments.wgsl"),
            ),
        ];

        for (name, source) in stage_shaders {
            let module = Self::compose_shader(device, &mut composer, name, source)?;
            let pipeline = Self::create_pipeline(device, name, &module);
            self.pipelines.insert(name.to_string(), pipeline);
        }

        Ok(())
    }

    fn add_module(composer: &mut Composer, name: &str, source: &str) -> Result<(), AgxError> {
        composer
            .add_composable_module(ComposableModuleDescriptor {
                source,
                file_path: name,
                ..Default::default()
            })
            .map_err(|e| AgxError::Gpu(format!("shader module '{name}' error: {e}")))?;
        Ok(())
    }

    fn compose_shader(
        device: &wgpu::Device,
        composer: &mut Composer,
        name: &str,
        source: &str,
    ) -> Result<wgpu::ShaderModule, AgxError> {
        // Use naga_oil to compose (resolves #import directives)
        let naga_module = composer
            .make_naga_module(NagaModuleDescriptor {
                source,
                file_path: name,
                ..Default::default()
            })
            .map_err(|e| AgxError::Gpu(format!("shader '{name}' compose error: {e}")))?;

        // Validate the composed naga module
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&naga_module)
        .map_err(|e| AgxError::Gpu(format!("shader '{name}' validation error: {e}")))?;

        // Convert composed naga IR back to WGSL source
        let wgsl_source = naga::back::wgsl::write_string(
            &naga_module,
            &info,
            naga::back::wgsl::WriterFlags::empty(),
        )
        .map_err(|e| AgxError::Gpu(format!("shader '{name}' WGSL write error: {e}")))?;

        // Create wgpu shader module from the composed WGSL.
        // wgpu uses its own naga (v24) to parse this; naga 23 writer
        // output is standard WGSL, so cross-version is fine.
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(wgsl_source)),
        });
        Ok(shader_module)
    }

    fn create_pipeline(
        device: &wgpu::Device,
        name: &str,
        module: &wgpu::ShaderModule,
    ) -> wgpu::ComputePipeline {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(name),
            layout: None,
            module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        })
    }
}
