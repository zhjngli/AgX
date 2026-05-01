//! AgX command-line interface.
//!
//! See the [project site](https://zhjngli.github.io/AgX/reference/cli.html)
//! for the full CLI reference.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, CommandFactory, Parser, Subcommand};

pub mod output;
pub mod validate;

/// Create an engine with the appropriate pipeline based on the `--gpu` flag.
pub fn create_engine(image: image::Rgb32FImage, use_gpu: bool) -> agx::Engine {
    if use_gpu {
        #[cfg(feature = "gpu")]
        return agx::Engine::new_gpu_auto(image);
        #[cfg(not(feature = "gpu"))]
        eprintln!("Warning: --gpu requires the 'gpu' feature; using CPU");
    }
    agx::Engine::new(image)
}

/// Top-level CLI arguments.
#[derive(Parser)]
#[command(name = "agx", about = "Photo editing CLI with portable TOML presets")]
pub struct Cli {
    /// Use GPU acceleration (opt-in). Falls back to CPU if no GPU is available.
    #[arg(long, global = true)]
    pub gpu: bool,
    /// Selected subcommand and its arguments.
    #[command(subcommand)]
    pub command: Commands,
}

/// Output encoding options shared by all commands.
#[derive(Args)]
pub struct OutputOpts {
    /// JPEG output quality (1-100, default 92)
    #[arg(long, default_value_t = 92)]
    pub quality: u8,
    /// Output format (jpeg, png, tiff). Inferred from extension if not specified.
    #[arg(long)]
    format: Option<String>,
    /// Write profiling timing data to this JSON file (requires --features profiling)
    #[cfg(feature = "profiling")]
    #[arg(long)]
    pub profile_output: Option<PathBuf>,
}

impl OutputOpts {
    /// Parse the explicit output format, if provided.
    pub fn parse_format(&self) -> agx::Result<Option<agx::encode::OutputFormat>> {
        self.format.as_deref().map(parse_output_format).transpose()
    }

    /// Build encoder options from the CLI flags.
    pub fn encode_options(&self) -> agx::Result<agx::encode::EncodeOptions> {
        Ok(agx::encode::EncodeOptions {
            jpeg_quality: self.quality,
            format: self.parse_format()?,
        })
    }
}

/// Per-channel HSL adjustment arguments.
#[derive(Args)]
pub struct HslArgs {
    /// Red hue shift (-180 to +180 degrees)
    #[arg(
        long = "hsl-red-hue",
        visible_alias = "hsl-red-h",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_red_hue: f32,
    /// Red saturation (-100 to +100)
    #[arg(
        long = "hsl-red-saturation",
        visible_alias = "hsl-red-s",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_red_saturation: f32,
    /// Red luminance (-100 to +100)
    #[arg(
        long = "hsl-red-luminance",
        visible_alias = "hsl-red-l",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_red_luminance: f32,

    /// Orange hue shift (-180 to +180 degrees)
    #[arg(
        long = "hsl-orange-hue",
        visible_alias = "hsl-orange-h",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_orange_hue: f32,
    /// Orange saturation (-100 to +100)
    #[arg(
        long = "hsl-orange-saturation",
        visible_alias = "hsl-orange-s",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_orange_saturation: f32,
    /// Orange luminance (-100 to +100)
    #[arg(
        long = "hsl-orange-luminance",
        visible_alias = "hsl-orange-l",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_orange_luminance: f32,

    /// Yellow hue shift (-180 to +180 degrees)
    #[arg(
        long = "hsl-yellow-hue",
        visible_alias = "hsl-yellow-h",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_yellow_hue: f32,
    /// Yellow saturation (-100 to +100)
    #[arg(
        long = "hsl-yellow-saturation",
        visible_alias = "hsl-yellow-s",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_yellow_saturation: f32,
    /// Yellow luminance (-100 to +100)
    #[arg(
        long = "hsl-yellow-luminance",
        visible_alias = "hsl-yellow-l",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_yellow_luminance: f32,

    /// Green hue shift (-180 to +180 degrees)
    #[arg(
        long = "hsl-green-hue",
        visible_alias = "hsl-green-h",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_green_hue: f32,
    /// Green saturation (-100 to +100)
    #[arg(
        long = "hsl-green-saturation",
        visible_alias = "hsl-green-s",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_green_saturation: f32,
    /// Green luminance (-100 to +100)
    #[arg(
        long = "hsl-green-luminance",
        visible_alias = "hsl-green-l",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_green_luminance: f32,

    /// Aqua hue shift (-180 to +180 degrees)
    #[arg(
        long = "hsl-aqua-hue",
        visible_alias = "hsl-aqua-h",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_aqua_hue: f32,
    /// Aqua saturation (-100 to +100)
    #[arg(
        long = "hsl-aqua-saturation",
        visible_alias = "hsl-aqua-s",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_aqua_saturation: f32,
    /// Aqua luminance (-100 to +100)
    #[arg(
        long = "hsl-aqua-luminance",
        visible_alias = "hsl-aqua-l",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_aqua_luminance: f32,

    /// Blue hue shift (-180 to +180 degrees)
    #[arg(
        long = "hsl-blue-hue",
        visible_alias = "hsl-blue-h",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_blue_hue: f32,
    /// Blue saturation (-100 to +100)
    #[arg(
        long = "hsl-blue-saturation",
        visible_alias = "hsl-blue-s",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_blue_saturation: f32,
    /// Blue luminance (-100 to +100)
    #[arg(
        long = "hsl-blue-luminance",
        visible_alias = "hsl-blue-l",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_blue_luminance: f32,

    /// Purple hue shift (-180 to +180 degrees)
    #[arg(
        long = "hsl-purple-hue",
        visible_alias = "hsl-purple-h",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_purple_hue: f32,
    /// Purple saturation (-100 to +100)
    #[arg(
        long = "hsl-purple-saturation",
        visible_alias = "hsl-purple-s",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_purple_saturation: f32,
    /// Purple luminance (-100 to +100)
    #[arg(
        long = "hsl-purple-luminance",
        visible_alias = "hsl-purple-l",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_purple_luminance: f32,

    /// Magenta hue shift (-180 to +180 degrees)
    #[arg(
        long = "hsl-magenta-hue",
        visible_alias = "hsl-magenta-h",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_magenta_hue: f32,
    /// Magenta saturation (-100 to +100)
    #[arg(
        long = "hsl-magenta-saturation",
        visible_alias = "hsl-magenta-s",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_magenta_saturation: f32,
    /// Magenta luminance (-100 to +100)
    #[arg(
        long = "hsl-magenta-luminance",
        visible_alias = "hsl-magenta-l",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    hsl_magenta_luminance: f32,
}

impl HslArgs {
    fn to_hsl_channels(&self) -> agx::HslChannels {
        agx::HslChannels {
            red: agx::HslChannel {
                hue: self.hsl_red_hue,
                saturation: self.hsl_red_saturation,
                luminance: self.hsl_red_luminance,
            },
            orange: agx::HslChannel {
                hue: self.hsl_orange_hue,
                saturation: self.hsl_orange_saturation,
                luminance: self.hsl_orange_luminance,
            },
            yellow: agx::HslChannel {
                hue: self.hsl_yellow_hue,
                saturation: self.hsl_yellow_saturation,
                luminance: self.hsl_yellow_luminance,
            },
            green: agx::HslChannel {
                hue: self.hsl_green_hue,
                saturation: self.hsl_green_saturation,
                luminance: self.hsl_green_luminance,
            },
            aqua: agx::HslChannel {
                hue: self.hsl_aqua_hue,
                saturation: self.hsl_aqua_saturation,
                luminance: self.hsl_aqua_luminance,
            },
            blue: agx::HslChannel {
                hue: self.hsl_blue_hue,
                saturation: self.hsl_blue_saturation,
                luminance: self.hsl_blue_luminance,
            },
            purple: agx::HslChannel {
                hue: self.hsl_purple_hue,
                saturation: self.hsl_purple_saturation,
                luminance: self.hsl_purple_luminance,
            },
            magenta: agx::HslChannel {
                hue: self.hsl_magenta_hue,
                saturation: self.hsl_magenta_saturation,
                luminance: self.hsl_magenta_luminance,
            },
        }
    }
}

/// Inline editing parameters (tone, white balance, LUT, HSL).
#[derive(Args)]
pub struct EditArgs {
    /// Exposure in stops (-5.0 to +5.0)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    exposure: f32,
    /// Contrast (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    contrast: f32,
    /// Highlights (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    highlights: f32,
    /// Shadows (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    shadows: f32,
    /// Whites (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    whites: f32,
    /// Blacks (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    blacks: f32,
    /// White balance temperature shift
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    temperature: f32,
    /// White balance tint shift
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    tint: f32,
    /// Path to a .cube LUT file
    #[arg(long)]
    lut: Option<PathBuf>,

    /// Vignette amount (-100 to +100). Negative darkens edges, positive brightens.
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    vignette_amount: f32,
    /// Vignette shape: elliptical (default) or circular
    #[arg(long, default_value = "elliptical")]
    vignette_shape: agx::VignetteShape,

    // --- Color grading ---
    /// Color grading: shadow wheel hue (0-360 degrees)
    #[arg(long = "cg-shadows-hue", default_value_t = 0.0)]
    cg_shadows_hue: f32,
    /// Color grading: shadow wheel saturation (0-100)
    #[arg(long = "cg-shadows-sat", default_value_t = 0.0)]
    cg_shadows_sat: f32,
    /// Color grading: shadow wheel luminance (-100 to +100)
    #[arg(
        long = "cg-shadows-lum",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    cg_shadows_lum: f32,
    /// Color grading: midtone wheel hue (0-360 degrees)
    #[arg(long = "cg-midtones-hue", default_value_t = 0.0)]
    cg_midtones_hue: f32,
    /// Color grading: midtone wheel saturation (0-100)
    #[arg(long = "cg-midtones-sat", default_value_t = 0.0)]
    cg_midtones_sat: f32,
    /// Color grading: midtone wheel luminance (-100 to +100)
    #[arg(
        long = "cg-midtones-lum",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    cg_midtones_lum: f32,
    /// Color grading: highlight wheel hue (0-360 degrees)
    #[arg(long = "cg-highlights-hue", default_value_t = 0.0)]
    cg_highlights_hue: f32,
    /// Color grading: highlight wheel saturation (0-100)
    #[arg(long = "cg-highlights-sat", default_value_t = 0.0)]
    cg_highlights_sat: f32,
    /// Color grading: highlight wheel luminance (-100 to +100)
    #[arg(
        long = "cg-highlights-lum",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    cg_highlights_lum: f32,
    /// Color grading: global wheel hue (0-360 degrees)
    #[arg(long = "cg-global-hue", default_value_t = 0.0)]
    cg_global_hue: f32,
    /// Color grading: global wheel saturation (0-100)
    #[arg(long = "cg-global-sat", default_value_t = 0.0)]
    cg_global_sat: f32,
    /// Color grading: global wheel luminance (-100 to +100)
    #[arg(
        long = "cg-global-lum",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    cg_global_lum: f32,
    /// Color grading: shadow/highlight balance (-100 to +100)
    #[arg(long = "cg-balance", default_value_t = 0.0, allow_hyphen_values = true)]
    cg_balance: f32,

    /// Tone curve — RGB master channel points (e.g. "0.0:0.0,0.25:0.15,0.75:0.85,1.0:1.0")
    #[arg(long = "tc-rgb")]
    tc_rgb: Option<String>,
    /// Tone curve — Luminance channel points
    #[arg(long = "tc-luma")]
    tc_luma: Option<String>,
    /// Tone curve — Red channel points
    #[arg(long = "tc-red")]
    tc_red: Option<String>,
    /// Tone curve — Green channel points
    #[arg(long = "tc-green")]
    tc_green: Option<String>,
    /// Tone curve — Blue channel points
    #[arg(long = "tc-blue")]
    tc_blue: Option<String>,

    /// Sharpening amount (0-100)
    #[arg(long = "sharpen-amount", default_value_t = 0.0)]
    sharpen_amount: f32,
    /// Sharpening radius / sigma (0.5-3.0)
    #[arg(long = "sharpen-radius", default_value_t = 1.0)]
    sharpen_radius: f32,
    /// Sharpening threshold (0-100). Higher = sharpen finer detail.
    #[arg(long = "sharpen-threshold", default_value_t = 25.0)]
    sharpen_threshold: f32,
    /// Sharpening masking (0-100). Limits sharpening to textured areas.
    #[arg(long = "sharpen-masking", default_value_t = 0.0)]
    sharpen_masking: f32,
    /// Clarity: local contrast at medium frequencies (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    clarity: f32,
    /// Texture: local contrast at high frequencies (-100 to +100)
    #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
    texture: f32,

    /// Dehaze amount (-100 to +100). Positive removes haze, negative adds haze.
    #[arg(
        long = "dehaze-amount",
        default_value_t = 0.0,
        allow_hyphen_values = true
    )]
    dehaze_amount: f32,

    /// Noise reduction: luminance strength (0-100)
    #[arg(long = "nr-luminance", default_value_t = 0.0)]
    nr_luminance: f32,
    /// Noise reduction: color strength (0-100)
    #[arg(long = "nr-color", default_value_t = 0.0)]
    nr_color: f32,
    /// Noise reduction: detail preservation (0-100)
    #[arg(long = "nr-detail", default_value_t = 0.0)]
    nr_detail: f32,

    /// Grain type (fine, silver, harsh)
    #[arg(long = "grain-type", default_value_t = agx::GrainType::Silver)]
    grain_type: agx::GrainType,
    /// Grain amount (0-100)
    #[arg(long = "grain-amount", default_value_t = 0.0)]
    grain_amount: f32,
    /// Grain size (0-100)
    #[arg(long = "grain-size", default_value_t = 50.0)]
    grain_size: f32,

    #[command(flatten)]
    hsl: HslArgs,
}

fn parse_curve_points(s: &str) -> Result<agx::ToneCurve, String> {
    let mut points = Vec::new();
    for pair in s.split(',') {
        let pair = pair.trim();
        let parts: Vec<&str> = pair.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("invalid point '{pair}', expected x:y"));
        }
        let x: f32 = parts[0]
            .trim()
            .parse()
            .map_err(|_| format!("invalid x value in '{pair}'"))?;
        let y: f32 = parts[1]
            .trim()
            .parse()
            .map_err(|_| format!("invalid y value in '{pair}'"))?;
        points.push((x, y));
    }
    let curve = agx::ToneCurve { points };
    curve.validate()?;
    Ok(curve)
}

impl EditArgs {
    /// Convert CLI edit flags into render parameters.
    pub fn to_params(&self) -> agx::Result<agx::Parameters> {
        fn parse_tc(flag: &Option<String>) -> agx::Result<agx::ToneCurve> {
            match flag {
                Some(s) => parse_curve_points(s)
                    .map_err(|e| agx::AgxError::Preset(format!("Error parsing tone curve: {e}"))),
                None => Ok(agx::ToneCurve::default()),
            }
        }

        Ok(agx::Parameters {
            exposure: self.exposure,
            contrast: self.contrast,
            highlights: self.highlights,
            shadows: self.shadows,
            whites: self.whites,
            blacks: self.blacks,
            temperature: self.temperature,
            tint: self.tint,
            hsl: self.hsl.to_hsl_channels(),
            vignette: agx::VignetteParams {
                amount: self.vignette_amount,
                shape: self.vignette_shape,
            },
            color_grading: agx::ColorGradingParams {
                shadows: agx::ColorWheel {
                    hue: self.cg_shadows_hue,
                    saturation: self.cg_shadows_sat,
                    luminance: self.cg_shadows_lum,
                },
                midtones: agx::ColorWheel {
                    hue: self.cg_midtones_hue,
                    saturation: self.cg_midtones_sat,
                    luminance: self.cg_midtones_lum,
                },
                highlights: agx::ColorWheel {
                    hue: self.cg_highlights_hue,
                    saturation: self.cg_highlights_sat,
                    luminance: self.cg_highlights_lum,
                },
                global: agx::ColorWheel {
                    hue: self.cg_global_hue,
                    saturation: self.cg_global_sat,
                    luminance: self.cg_global_lum,
                },
                balance: self.cg_balance,
            },
            tone_curve: agx::ToneCurveParams {
                rgb: parse_tc(&self.tc_rgb)?,
                luma: parse_tc(&self.tc_luma)?,
                red: parse_tc(&self.tc_red)?,
                green: parse_tc(&self.tc_green)?,
                blue: parse_tc(&self.tc_blue)?,
            },
            detail: agx::DetailParams {
                sharpening: agx::SharpeningParams {
                    amount: self.sharpen_amount,
                    radius: self.sharpen_radius,
                    threshold: self.sharpen_threshold,
                    masking: self.sharpen_masking,
                },
                clarity: self.clarity,
                texture: self.texture,
            },
            dehaze: agx::DehazeParams {
                amount: self.dehaze_amount,
            },
            noise_reduction: agx::NoiseReductionParams {
                luminance: self.nr_luminance,
                color: self.nr_color,
                detail: self.nr_detail,
            },
            grain: agx::GrainParams {
                grain_type: self.grain_type,
                amount: self.grain_amount,
                size: self.grain_size,
                seed: None,
            },
        })
    }

    /// Load the optional LUT file referenced by the CLI flags.
    pub fn load_lut(&self) -> agx::Result<Option<Arc<agx::Lut3D>>> {
        match &self.lut {
            Some(lut_path) => Ok(Some(Arc::new(agx::Lut3D::from_cube_file(lut_path)?))),
            None => Ok(None),
        }
    }
}

/// Batch processing options shared by batch-apply and batch-edit.
#[derive(Args)]
pub struct BatchOpts {
    /// Directory containing input images
    #[arg(long)]
    pub input_dir: PathBuf,
    /// Directory for output images (created if missing)
    #[arg(long)]
    pub output_dir: PathBuf,
    /// Recurse into subdirectories
    #[arg(short, long, default_value_t = false)]
    pub recursive: bool,
    /// Number of parallel workers (0 = auto-detect CPU cores)
    #[arg(short, long, default_value_t = 0)]
    pub jobs: usize,
    /// Continue processing when individual files fail
    #[arg(long, default_value_t = false)]
    pub skip_errors: bool,
    /// Append suffix to output filenames (e.g., `_edited`)
    #[arg(long)]
    pub suffix: Option<String>,

    /// Shared output encoding options for each batch result.
    #[command(flatten)]
    pub output: OutputOpts,
}

/// Output format for commands that support both human-readable and machine-readable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output (default).
    Human,
    /// Machine-readable JSON output.
    Json,
}

/// Supported CLI subcommands.
#[derive(Subcommand)]
pub enum Commands {
    /// Apply a TOML preset to an image
    #[command(group = clap::ArgGroup::new("preset_source").required(true))]
    Apply {
        /// Input image path
        #[arg(short, long)]
        input: PathBuf,
        /// Preset TOML file path (single preset, full replacement)
        #[arg(short, long, group = "preset_source")]
        preset: Option<PathBuf>,
        /// Preset TOML files to layer (left-to-right, last-write-wins)
        #[arg(long, group = "preset_source", num_args = 1..)]
        presets: Vec<PathBuf>,
        /// Output image path
        #[arg(short, long)]
        output: PathBuf,

        /// Shared output encoding options.
        #[command(flatten)]
        output_opts: OutputOpts,
    },
    /// Edit an image with inline parameters
    Edit {
        /// Input image path
        #[arg(short, long)]
        input: PathBuf,
        /// Output image path
        #[arg(short, long)]
        output: PathBuf,

        /// Inline edit parameters.
        #[command(flatten)]
        edit: EditArgs,
        /// Shared output encoding options.
        #[command(flatten)]
        output_opts: OutputOpts,
    },
    /// Apply a TOML preset to all images in a directory
    BatchApply {
        /// Preset TOML file path
        #[arg(short, long)]
        preset: PathBuf,

        /// Shared batch processing options.
        #[command(flatten)]
        batch: BatchOpts,
    },
    /// Edit all images in a directory with inline parameters
    BatchEdit {
        /// Inline edit parameters.
        #[command(flatten)]
        edit: EditArgs,
        /// Shared batch processing options.
        #[command(flatten)]
        batch: BatchOpts,
    },
    /// Apply multiple presets to a single image (decode once, render per preset)
    MultiApply {
        /// Input image path
        #[arg(short, long)]
        input: PathBuf,
        /// Preset TOML file(s) to apply (one output per preset)
        #[arg(short, long, required = true, num_args = 1..)]
        preset: Vec<PathBuf>,
        /// Output directory (created if missing)
        #[arg(short, long)]
        output: PathBuf,
        /// Also render a no-preset (identity) output
        #[arg(long, default_value_t = false)]
        noop: bool,
        /// Number of preset renders to run concurrently (default: 1)
        #[arg(short, long, default_value_t = 1)]
        jobs: usize,
    },
    /// Validate one or more preset files for correctness without rendering.
    ///
    /// Reports unknown fields, type mismatches, out-of-range values, missing
    /// LUT files, and extends chain problems. Exits 0 if all clean, 1 if any
    /// file has errors.
    Validate {
        /// Paths to preset TOML files. Use shell glob to validate many at once.
        #[arg(required = true)]
        paths: Vec<std::path::PathBuf>,

        /// Suppress "ok" lines for clean files; only show files with errors.
        #[arg(short, long)]
        quiet: bool,

        /// Output format.
        #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
        format: OutputFormat,
    },
}

fn parse_output_format(s: &str) -> agx::Result<agx::encode::OutputFormat> {
    agx::encode::OutputFormat::from_extension(s).ok_or_else(|| {
        agx::AgxError::Encode(format!(
            "unsupported output format '{s}'. Use: jpeg, png, or tiff"
        ))
    })
}

/// Build the fully-configured clap command for `agx`.
pub fn build_cli() -> clap::Command {
    Cli::command()
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{build_cli, Cli, Commands};

    #[test]
    fn build_cli_returns_valid_command() {
        let command = build_cli();

        command.clone().debug_assert();

        assert_eq!(command.get_name(), "agx");

        let subcommands: Vec<_> = command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name().to_string())
            .collect();

        assert!(subcommands.iter().any(|name| name == "apply"));
        assert!(subcommands.iter().any(|name| name == "edit"));
        assert!(subcommands.iter().any(|name| name == "batch-apply"));
        assert!(subcommands.iter().any(|name| name == "batch-edit"));
        assert!(subcommands.iter().any(|name| name == "multi-apply"));
    }

    #[test]
    fn edit_to_params_returns_error_for_invalid_tone_curve() {
        let cli = Cli::parse_from([
            "agx",
            "edit",
            "--input",
            "input.png",
            "--output",
            "output.png",
            "--tc-rgb",
            "not-a-curve",
        ]);

        let Commands::Edit { edit, .. } = cli.command else {
            panic!("expected edit command");
        };

        let error = edit.to_params().unwrap_err();

        assert!(error.to_string().contains("Error parsing tone curve"));
    }
}
