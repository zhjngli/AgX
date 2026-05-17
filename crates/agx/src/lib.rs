//! AgX — open-source preset-first photo editing library.
//!
//! See the [project site](https://zhjngli.github.io/AgX/) for tutorials,
//! how-to guides, the CLI reference, and the preset format reference.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod adjust;
pub mod color_space;
pub mod decode;
pub mod encode;
pub mod engine;
pub mod error;
pub mod lut;
pub mod metadata;
pub mod preset;

pub use adjust::{
    ColorGradingParams, ColorWheel, DehazeParams, DetailParams, GrainParams, GrainType,
    NoiseReductionParams, SharpeningParams, ToneCurve, ToneCurveParams, VignetteShape,
};
pub use decode::decode;
pub use encode::{EncodeOptions, OutputFormat};
pub use engine::{
    ColorSpace, Engine, HslChannel, HslChannels, Parameters, PartialColorGradingParams,
    PartialColorWheel, PartialDehazeParams, PartialDetailParams, PartialGrainParams,
    PartialHslChannel, PartialHslChannels, PartialNoiseReductionParams, PartialParameters,
    PartialSharpeningParams, PartialToneCurve, PartialToneCurveParams, PartialVignetteParams,
    RenderResult, VignetteParams,
};

#[cfg(feature = "profiling")]
pub use engine::RenderProfile;
pub use error::{AgxError, Result};
pub use lut::Lut3D;
pub use metadata::ImageMetadata;
pub use preset::Preset;
