use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::process;

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
    #[arg(long, value_parser = parse_vignette_shape, default_value = "elliptical")]
    vignette_shape: agx::VignetteShape,

    #[command(flatten)]
    hsl: HslArgs,
}

fn parse_vignette_shape(s: &str) -> Result<agx::VignetteShape, String> {
    match s {
        "elliptical" => Ok(agx::VignetteShape::Elliptical),
        "circular" => Ok(agx::VignetteShape::Circular),
        _ => Err(format!(
            "invalid vignette shape '{s}'. Use: elliptical or circular"
        )),
    }
}

impl EditArgs {
    fn to_params(&self) -> agx::Parameters {
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
        }
    }

    fn load_lut(&self) -> agx::Result<Option<agx::Lut3D>> {
        match &self.lut {
            Some(lut_path) => Ok(Some(agx::Lut3D::from_cube_file(lut_path)?)),
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
        lut_data.as_ref(),
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
