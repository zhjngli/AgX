//! AgX command-line interface.
//!
//! See the [project site](https://zhjngli.github.io/AgX/reference/cli.html)
//! for the full CLI reference.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::path::PathBuf;
use std::process;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};

use agx::{Engine, Preset};

mod batch;

#[derive(Parser)]
#[command(name = "agx", about = "Photo editing CLI with portable TOML presets")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Output encoding options shared by all commands.
#[derive(Args)]
struct OutputOpts {
    /// JPEG output quality (1-100, default 92)
    #[arg(long, default_value_t = 92)]
    quality: u8,
    /// Output format (jpeg, png, tiff). Inferred from extension if not specified.
    #[arg(long)]
    format: Option<String>,
    /// Write profiling timing data to this JSON file (requires --features profiling)
    #[cfg(feature = "profiling")]
    #[arg(long)]
    profile_output: Option<PathBuf>,
}

impl OutputOpts {
    fn parse_format(&self) -> agx::Result<Option<agx::encode::OutputFormat>> {
        self.format.as_deref().map(parse_output_format).transpose()
    }

    fn encode_options(&self) -> agx::Result<agx::encode::EncodeOptions> {
        Ok(agx::encode::EncodeOptions {
            jpeg_quality: self.quality,
            format: self.parse_format()?,
        })
    }
}

/// Per-channel HSL adjustment arguments.
#[derive(Args)]
struct HslArgs {
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
struct EditArgs {
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
    fn to_params(&self) -> agx::Parameters {
        fn parse_tc(flag: &Option<String>) -> agx::ToneCurve {
            match flag {
                Some(s) => parse_curve_points(s).unwrap_or_else(|e| {
                    eprintln!("Error parsing tone curve: {e}");
                    std::process::exit(1);
                }),
                None => agx::ToneCurve::default(),
            }
        }

        agx::Parameters {
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
                rgb: parse_tc(&self.tc_rgb),
                luma: parse_tc(&self.tc_luma),
                red: parse_tc(&self.tc_red),
                green: parse_tc(&self.tc_green),
                blue: parse_tc(&self.tc_blue),
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
        }
    }

    fn load_lut(&self) -> agx::Result<Option<Arc<agx::Lut3D>>> {
        match &self.lut {
            Some(lut_path) => Ok(Some(Arc::new(agx::Lut3D::from_cube_file(lut_path)?))),
            None => Ok(None),
        }
    }
}

/// Batch processing options shared by batch-apply and batch-edit.
#[derive(Args)]
struct BatchOpts {
    /// Directory containing input images
    #[arg(long)]
    input_dir: PathBuf,
    /// Directory for output images (created if missing)
    #[arg(long)]
    output_dir: PathBuf,
    /// Recurse into subdirectories
    #[arg(short, long, default_value_t = false)]
    recursive: bool,
    /// Number of parallel workers (0 = auto-detect CPU cores)
    #[arg(short, long, default_value_t = 0)]
    jobs: usize,
    /// Continue processing when individual files fail
    #[arg(long, default_value_t = false)]
    skip_errors: bool,
    /// Append suffix to output filenames (e.g., `_edited`)
    #[arg(long)]
    suffix: Option<String>,

    #[command(flatten)]
    output: OutputOpts,
}

#[derive(Subcommand)]
enum Commands {
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

        #[command(flatten)]
        edit: EditArgs,
        #[command(flatten)]
        output_opts: OutputOpts,
    },
    /// Apply a TOML preset to all images in a directory
    BatchApply {
        /// Preset TOML file path
        #[arg(short, long)]
        preset: PathBuf,

        #[command(flatten)]
        batch: BatchOpts,
    },
    /// Edit all images in a directory with inline parameters
    BatchEdit {
        #[command(flatten)]
        edit: EditArgs,
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
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Apply {
            input,
            preset,
            presets,
            output,
            output_opts,
        } => run_apply(&input, preset.as_deref(), &presets, &output, &output_opts),
        Commands::Edit {
            input,
            output,
            edit,
            output_opts,
        } => run_edit(&input, &output, &edit, &output_opts),
        Commands::BatchApply { preset, batch } => run_batch_apply(&preset, &batch),
        Commands::BatchEdit { edit, batch } => run_batch_edit(&edit, &batch),
        Commands::MultiApply {
            input,
            preset,
            output,
            noop,
            jobs,
        } => run_multi_apply(&input, &preset, &output, noop, jobs),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn parse_output_format(s: &str) -> agx::Result<agx::encode::OutputFormat> {
    agx::encode::OutputFormat::from_extension(s).ok_or_else(|| {
        agx::AgxError::Encode(format!(
            "unsupported output format '{s}'. Use: jpeg, png, or tiff"
        ))
    })
}

#[cfg(feature = "profiling")]
fn write_profile_entry(
    path: &std::path::Path,
    image_name: &str,
    preset_name: &str,
    dimensions: (u32, u32),
    decode_ms: f64,
    render_profile: &agx::RenderProfile,
    encode_ms: f64,
) -> agx::Result<()> {
    use std::io::Write;

    let mut stages = serde_json::Map::new();
    stages.insert("decode".to_string(), serde_json::Value::from(decode_ms));
    for (name, ms) in &render_profile.stages {
        stages.insert(name.clone(), serde_json::Value::from(*ms));
    }
    stages.insert("encode".to_string(), serde_json::Value::from(encode_ms));

    let total_ms = decode_ms + render_profile.total_ms + encode_ms;

    let entry = serde_json::json!({
        "image": image_name,
        "preset": preset_name,
        "dimensions": [dimensions.0, dimensions.1],
        "stages": stages,
        "total_ms": total_ms,
    });

    let mut entries: Vec<serde_json::Value> = match std::fs::read_to_string(path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(agx::AgxError::Io(e)),
    };
    entries.push(entry);

    let mut file = std::fs::File::create(path).map_err(agx::AgxError::Io)?;
    file.write_all(serde_json::to_string_pretty(&entries).unwrap().as_bytes())
        .map_err(agx::AgxError::Io)?;
    Ok(())
}

fn run_apply(
    input: &std::path::Path,
    preset_path: Option<&std::path::Path>,
    presets: &[PathBuf],
    output: &std::path::Path,
    output_opts: &OutputOpts,
) -> agx::Result<()> {
    #[cfg(feature = "profiling")]
    let decode_start = std::time::Instant::now();

    let metadata = agx::metadata::extract_metadata(input);
    let linear = agx::decode::decode(input)?;

    #[cfg(feature = "profiling")]
    let decode_ms = decode_start.elapsed().as_secs_f64() * 1000.0;

    let mut engine = Engine::new(linear);

    #[cfg(feature = "profiling")]
    let preset_name = if !presets.is_empty() {
        presets
            .iter()
            .map(|p| {
                p.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("+")
    } else if let Some(path) = preset_path {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    } else {
        "none".to_string()
    };

    if !presets.is_empty() {
        for path in presets {
            let preset = Preset::load_from_file(path)?;
            engine.layer_preset(&preset);
        }
    } else if let Some(path) = preset_path {
        let preset = Preset::load_from_file(path)?;
        engine.apply_preset(&preset);
    }

    let result = engine.render();
    let rendered = result.image;
    let opts = output_opts.encode_options()?;

    #[cfg(feature = "profiling")]
    let encode_start = std::time::Instant::now();

    let final_path =
        agx::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())?;

    #[cfg(feature = "profiling")]
    let encode_ms = encode_start.elapsed().as_secs_f64() * 1000.0;

    println!("Saved to {}", final_path.display());

    #[cfg(feature = "profiling")]
    if let Some(ref profile_path) = output_opts.profile_output {
        if let Some(profile) = result.profile {
            let dims = (rendered.width(), rendered.height());
            let image_name = input.file_name().unwrap_or_default().to_string_lossy();
            write_profile_entry(
                profile_path,
                &image_name,
                &preset_name,
                dims,
                decode_ms,
                &profile,
                encode_ms,
            )?;
        }
    }

    Ok(())
}

fn run_edit(
    input: &std::path::Path,
    output: &std::path::Path,
    edit: &EditArgs,
    output_opts: &OutputOpts,
) -> agx::Result<()> {
    #[cfg(feature = "profiling")]
    let decode_start = std::time::Instant::now();

    let metadata = agx::metadata::extract_metadata(input);
    let linear = agx::decode::decode(input)?;

    #[cfg(feature = "profiling")]
    let decode_ms = decode_start.elapsed().as_secs_f64() * 1000.0;

    let mut engine = Engine::new(linear);
    engine.set_params(edit.to_params());
    if let Some(lut) = edit.load_lut()? {
        engine.set_lut(Some(lut));
    }
    let result = engine.render();
    let rendered = result.image;
    let opts = output_opts.encode_options()?;

    #[cfg(feature = "profiling")]
    let encode_start = std::time::Instant::now();

    let final_path =
        agx::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())?;

    #[cfg(feature = "profiling")]
    let encode_ms = encode_start.elapsed().as_secs_f64() * 1000.0;

    println!("Saved to {}", final_path.display());

    #[cfg(feature = "profiling")]
    if let Some(ref profile_path) = output_opts.profile_output {
        if let Some(profile) = result.profile {
            let dims = (rendered.width(), rendered.height());
            let image_name = input.file_name().unwrap_or_default().to_string_lossy();
            write_profile_entry(
                profile_path,
                &image_name,
                "edit",
                dims,
                decode_ms,
                &profile,
                encode_ms,
            )?;
        }
    }

    Ok(())
}

fn run_batch_apply(preset_path: &std::path::Path, batch: &BatchOpts) -> agx::Result<()> {
    let fmt = batch.output.parse_format()?;
    let summary = batch::run_batch_apply(
        &batch.input_dir,
        preset_path,
        &batch.output_dir,
        batch.recursive,
        batch.output.quality,
        fmt,
        batch.suffix.as_deref(),
        batch.jobs,
        batch.skip_errors,
    );
    if !summary.failed.is_empty() {
        process::exit(1);
    }
    Ok(())
}

fn run_batch_edit(edit: &EditArgs, batch: &BatchOpts) -> agx::Result<()> {
    let params = edit.to_params();
    let lut_data = edit.load_lut()?;
    let fmt = batch.output.parse_format()?;
    let summary = batch::run_batch_edit(
        &batch.input_dir,
        &batch.output_dir,
        batch.recursive,
        &params,
        lut_data,
        batch.output.quality,
        fmt,
        batch.suffix.as_deref(),
        batch.jobs,
        batch.skip_errors,
    );
    if !summary.failed.is_empty() {
        process::exit(1);
    }
    Ok(())
}

/// Render a decoded image with an optional preset and encode to PNG.
/// Used by `run_multi_apply` for both sequential and parallel paths.
fn render_and_encode(
    image: image::Rgb32FImage,
    preset: Option<&agx::Preset>,
    output_path: &std::path::Path,
    metadata: Option<&agx::metadata::ImageMetadata>,
) -> agx::Result<()> {
    let mut engine = Engine::new(image);
    if let Some(p) = preset {
        engine.apply_preset(p);
    }
    let result = engine.render();
    let final_path = agx::encode::encode_to_file_with_options(
        &result.image,
        output_path,
        &agx::encode::EncodeOptions::default(),
        metadata,
    )?;
    println!("Saved to {}", final_path.display());
    Ok(())
}

fn run_multi_apply(
    input: &std::path::Path,
    presets: &[PathBuf],
    output_dir: &std::path::Path,
    noop: bool,
    jobs: usize,
) -> agx::Result<()> {
    let image_stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    std::fs::create_dir_all(output_dir).map_err(agx::AgxError::Io)?;

    let metadata = agx::metadata::extract_metadata(input);
    let decoded = agx::decode::decode(input)?;

    let loaded: Vec<(String, agx::Preset)> = presets
        .iter()
        .map(|path| {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let preset = agx::Preset::load_from_file(path)?;
            Ok((name, preset))
        })
        .collect::<agx::Result<Vec<_>>>()?;

    if noop {
        let noop_path = output_dir.join(format!("{image_stem}_noop.png"));
        render_and_encode(decoded.clone(), None, &noop_path, metadata.as_ref())?;
    }

    if jobs <= 1 {
        for (name, preset) in &loaded {
            let out_path = output_dir.join(format!("{image_stem}_{name}.png"));
            render_and_encode(decoded.clone(), Some(preset), &out_path, metadata.as_ref())?;
        }
    } else {
        // OS threads for concurrency control; each render's internal rayon
        // parallelism uses the global pool. Chunks bound concurrent memory usage.
        let errors: std::sync::Mutex<Vec<agx::AgxError>> = std::sync::Mutex::new(Vec::new());

        for chunk in loaded.chunks(jobs) {
            std::thread::scope(|s| {
                for (name, preset) in chunk {
                    let decoded = &decoded;
                    let metadata = &metadata;
                    let errors = &errors;
                    s.spawn(move || {
                        let out_path = output_dir.join(format!("{image_stem}_{name}.png"));
                        match render_and_encode(
                            decoded.clone(),
                            Some(preset),
                            &out_path,
                            metadata.as_ref(),
                        ) {
                            Ok(()) => {}
                            Err(e) => errors.lock().unwrap().push(e),
                        }
                    });
                }
            });
        }

        let errs = errors.into_inner().unwrap();
        if let Some(first) = errs.into_iter().next() {
            return Err(first);
        }
    }

    Ok(())
}
