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

    #[command(flatten)]
    hsl: HslArgs,
}

fn parse_curve_points(s: &str) -> Result<Vec<(f32, f32)>, String> {
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
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return Err(format!("point ({x}, {y}) out of range [0, 1]"));
        }
        points.push((x, y));
    }
    if points.len() < 2 {
        return Err("tone curve needs at least 2 points".to_string());
    }
    if (points[0].0).abs() > 1e-6 {
        return Err(format!("first point x must be 0.0, got {}", points[0].0));
    }
    if (points.last().unwrap().0 - 1.0).abs() > 1e-6 {
        return Err(format!(
            "last point x must be 1.0, got {}",
            points.last().unwrap().0
        ));
    }
    for i in 1..points.len() {
        if points[i].0 <= points[i - 1].0 {
            return Err(format!(
                "points must have strictly increasing x: {} >= {}",
                points[i].0,
                points[i - 1].0
            ));
        }
    }
    Ok(points)
}

impl EditArgs {
    fn to_params(&self) -> agx::Parameters {
        fn parse_tc(flag: &Option<String>) -> agx::ToneCurve {
            match flag {
                Some(s) => {
                    let points = parse_curve_points(s).unwrap_or_else(|e| {
                        eprintln!("Error parsing tone curve: {e}");
                        std::process::exit(1);
                    });
                    agx::ToneCurve { points }
                }
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

fn run_apply(
    input: &std::path::Path,
    preset_path: Option<&std::path::Path>,
    presets: &[PathBuf],
    output: &std::path::Path,
    output_opts: &OutputOpts,
) -> agx::Result<()> {
    let metadata = agx::metadata::extract_metadata(input);
    let linear = agx::decode::decode(input)?;
    let mut engine = Engine::new(linear);

    if !presets.is_empty() {
        for path in presets {
            let preset = Preset::load_from_file(path)?;
            engine.layer_preset(&preset);
        }
    } else if let Some(path) = preset_path {
        let preset = Preset::load_from_file(path)?;
        engine.apply_preset(&preset);
    }

    let rendered = engine.render();
    let opts = output_opts.encode_options()?;
    let final_path =
        agx::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())?;
    println!("Saved to {}", final_path.display());
    Ok(())
}

fn run_edit(
    input: &std::path::Path,
    output: &std::path::Path,
    edit: &EditArgs,
    output_opts: &OutputOpts,
) -> agx::Result<()> {
    let metadata = agx::metadata::extract_metadata(input);
    let linear = agx::decode::decode(input)?;
    let mut engine = Engine::new(linear);
    engine.set_params(edit.to_params());
    if let Some(lut) = edit.load_lut()? {
        engine.set_lut(Some(lut));
    }
    let rendered = engine.render();
    let opts = output_opts.encode_options()?;
    let final_path =
        agx::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())?;
    println!("Saved to {}", final_path.display());
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
