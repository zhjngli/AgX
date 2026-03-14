use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

use oxiraw::{Engine, Preset};

mod batch;

#[derive(Parser)]
#[command(
    name = "oxiraw",
    about = "Photo editing CLI with portable TOML presets"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
        /// JPEG output quality (1-100, default 92)
        #[arg(long, default_value_t = 92)]
        quality: u8,
        /// Output format (jpeg, png, tiff). Inferred from extension if not specified.
        #[arg(long)]
        format: Option<String>,
    },
    /// Edit an image with inline parameters
    Edit {
        /// Input image path
        #[arg(short, long)]
        input: PathBuf,
        /// Output image path
        #[arg(short, long)]
        output: PathBuf,
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
        /// JPEG output quality (1-100, default 92)
        #[arg(long, default_value_t = 92)]
        quality: u8,
        /// Output format (jpeg, png, tiff). Inferred from extension if not specified.
        #[arg(long)]
        format: Option<String>,

        // --- HSL per-channel adjustments ---
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
    },
    /// Apply a TOML preset to all images in a directory
    BatchApply {
        /// Directory containing input images
        #[arg(long)]
        input_dir: PathBuf,
        /// Preset TOML file path
        #[arg(short, long)]
        preset: PathBuf,
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
        /// JPEG output quality (1-100, default 92)
        #[arg(long, default_value_t = 92)]
        quality: u8,
        /// Output format (jpeg, png, tiff). Preserved from input if not specified.
        #[arg(long)]
        format: Option<String>,
    },
    /// Edit all images in a directory with inline parameters
    BatchEdit {
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
        /// JPEG output quality (1-100, default 92)
        #[arg(long, default_value_t = 92)]
        quality: u8,
        /// Output format (jpeg, png, tiff). Preserved from input if not specified.
        #[arg(long)]
        format: Option<String>,
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

        // --- HSL per-channel adjustments ---
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
            quality,
            format,
        } => run_apply(
            &input,
            preset.as_deref(),
            &presets,
            &output,
            quality,
            format.as_deref(),
        ),
        Commands::Edit {
            input,
            output,
            exposure,
            contrast,
            highlights,
            shadows,
            whites,
            blacks,
            temperature,
            tint,
            lut,
            quality,
            format,
            hsl_red_hue,
            hsl_red_saturation,
            hsl_red_luminance,
            hsl_orange_hue,
            hsl_orange_saturation,
            hsl_orange_luminance,
            hsl_yellow_hue,
            hsl_yellow_saturation,
            hsl_yellow_luminance,
            hsl_green_hue,
            hsl_green_saturation,
            hsl_green_luminance,
            hsl_aqua_hue,
            hsl_aqua_saturation,
            hsl_aqua_luminance,
            hsl_blue_hue,
            hsl_blue_saturation,
            hsl_blue_luminance,
            hsl_purple_hue,
            hsl_purple_saturation,
            hsl_purple_luminance,
            hsl_magenta_hue,
            hsl_magenta_saturation,
            hsl_magenta_luminance,
        } => {
            let hsl = oxiraw::engine::HslChannels {
                red: oxiraw::engine::HslChannel {
                    hue: hsl_red_hue,
                    saturation: hsl_red_saturation,
                    luminance: hsl_red_luminance,
                },
                orange: oxiraw::engine::HslChannel {
                    hue: hsl_orange_hue,
                    saturation: hsl_orange_saturation,
                    luminance: hsl_orange_luminance,
                },
                yellow: oxiraw::engine::HslChannel {
                    hue: hsl_yellow_hue,
                    saturation: hsl_yellow_saturation,
                    luminance: hsl_yellow_luminance,
                },
                green: oxiraw::engine::HslChannel {
                    hue: hsl_green_hue,
                    saturation: hsl_green_saturation,
                    luminance: hsl_green_luminance,
                },
                aqua: oxiraw::engine::HslChannel {
                    hue: hsl_aqua_hue,
                    saturation: hsl_aqua_saturation,
                    luminance: hsl_aqua_luminance,
                },
                blue: oxiraw::engine::HslChannel {
                    hue: hsl_blue_hue,
                    saturation: hsl_blue_saturation,
                    luminance: hsl_blue_luminance,
                },
                purple: oxiraw::engine::HslChannel {
                    hue: hsl_purple_hue,
                    saturation: hsl_purple_saturation,
                    luminance: hsl_purple_luminance,
                },
                magenta: oxiraw::engine::HslChannel {
                    hue: hsl_magenta_hue,
                    saturation: hsl_magenta_saturation,
                    luminance: hsl_magenta_luminance,
                },
            };
            run_edit(
                &input,
                &output,
                exposure,
                contrast,
                highlights,
                shadows,
                whites,
                blacks,
                temperature,
                tint,
                lut.as_deref(),
                quality,
                format.as_deref(),
                &hsl,
            )
        }
        Commands::BatchApply {
            input_dir,
            preset,
            output_dir,
            recursive,
            jobs,
            skip_errors,
            suffix,
            quality,
            format,
        } => (|| -> oxiraw::Result<()> {
            let fmt = format.as_deref().map(parse_output_format).transpose()?;
            let summary = batch::run_batch_apply(
                &input_dir,
                &preset,
                &output_dir,
                recursive,
                quality,
                fmt,
                suffix.as_deref(),
                jobs,
                skip_errors,
            );
            if !summary.failed.is_empty() {
                process::exit(1);
            }
            Ok(())
        })(),
        Commands::BatchEdit {
            input_dir,
            output_dir,
            recursive,
            jobs,
            skip_errors,
            suffix,
            quality,
            format,
            exposure,
            contrast,
            highlights,
            shadows,
            whites,
            blacks,
            temperature,
            tint,
            lut,
            hsl_red_hue,
            hsl_red_saturation,
            hsl_red_luminance,
            hsl_orange_hue,
            hsl_orange_saturation,
            hsl_orange_luminance,
            hsl_yellow_hue,
            hsl_yellow_saturation,
            hsl_yellow_luminance,
            hsl_green_hue,
            hsl_green_saturation,
            hsl_green_luminance,
            hsl_aqua_hue,
            hsl_aqua_saturation,
            hsl_aqua_luminance,
            hsl_blue_hue,
            hsl_blue_saturation,
            hsl_blue_luminance,
            hsl_purple_hue,
            hsl_purple_saturation,
            hsl_purple_luminance,
            hsl_magenta_hue,
            hsl_magenta_saturation,
            hsl_magenta_luminance,
        } => (|| -> oxiraw::Result<()> {
            let hsl = oxiraw::engine::HslChannels {
                red: oxiraw::engine::HslChannel {
                    hue: hsl_red_hue,
                    saturation: hsl_red_saturation,
                    luminance: hsl_red_luminance,
                },
                orange: oxiraw::engine::HslChannel {
                    hue: hsl_orange_hue,
                    saturation: hsl_orange_saturation,
                    luminance: hsl_orange_luminance,
                },
                yellow: oxiraw::engine::HslChannel {
                    hue: hsl_yellow_hue,
                    saturation: hsl_yellow_saturation,
                    luminance: hsl_yellow_luminance,
                },
                green: oxiraw::engine::HslChannel {
                    hue: hsl_green_hue,
                    saturation: hsl_green_saturation,
                    luminance: hsl_green_luminance,
                },
                aqua: oxiraw::engine::HslChannel {
                    hue: hsl_aqua_hue,
                    saturation: hsl_aqua_saturation,
                    luminance: hsl_aqua_luminance,
                },
                blue: oxiraw::engine::HslChannel {
                    hue: hsl_blue_hue,
                    saturation: hsl_blue_saturation,
                    luminance: hsl_blue_luminance,
                },
                purple: oxiraw::engine::HslChannel {
                    hue: hsl_purple_hue,
                    saturation: hsl_purple_saturation,
                    luminance: hsl_purple_luminance,
                },
                magenta: oxiraw::engine::HslChannel {
                    hue: hsl_magenta_hue,
                    saturation: hsl_magenta_saturation,
                    luminance: hsl_magenta_luminance,
                },
            };
            let params = oxiraw::Parameters {
                exposure,
                contrast,
                highlights,
                shadows,
                whites,
                blacks,
                temperature,
                tint,
                hsl,
            };
            let lut_data = match lut {
                Some(ref lut_path) => Some(oxiraw::Lut3D::from_cube_file(lut_path)?),
                None => None,
            };
            let fmt = format.as_deref().map(parse_output_format).transpose()?;
            let summary = batch::run_batch_edit(
                &input_dir,
                &output_dir,
                recursive,
                &params,
                lut_data.as_ref(),
                quality,
                fmt,
                suffix.as_deref(),
                jobs,
                skip_errors,
            );
            if !summary.failed.is_empty() {
                process::exit(1);
            }
            Ok(())
        })(),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn parse_output_format(s: &str) -> oxiraw::Result<oxiraw::encode::OutputFormat> {
    match s.to_ascii_lowercase().as_str() {
        "jpeg" | "jpg" => Ok(oxiraw::encode::OutputFormat::Jpeg),
        "png" => Ok(oxiraw::encode::OutputFormat::Png),
        "tiff" | "tif" => Ok(oxiraw::encode::OutputFormat::Tiff),
        _ => Err(oxiraw::OxirawError::Encode(format!(
            "unsupported output format '{s}'. Use: jpeg, png, or tiff"
        ))),
    }
}

fn run_apply(
    input: &std::path::Path,
    preset_path: Option<&std::path::Path>,
    presets: &[PathBuf],
    output: &std::path::Path,
    quality: u8,
    format: Option<&str>,
) -> oxiraw::Result<()> {
    let metadata = oxiraw::metadata::extract_metadata(input);
    let linear = oxiraw::decode::decode(input)?;
    let mut engine = Engine::new(linear);

    if !presets.is_empty() {
        // Multi-preset mode: layer left-to-right
        for path in presets {
            let preset = Preset::load_from_file(path)?;
            engine.layer_preset(&preset);
        }
    } else if let Some(path) = preset_path {
        // Single preset mode: full replacement (backward compatible)
        let preset = Preset::load_from_file(path)?;
        engine.apply_preset(&preset);
    }

    let rendered = engine.render();
    let fmt = format.map(parse_output_format).transpose()?;
    let opts = oxiraw::encode::EncodeOptions {
        jpeg_quality: quality,
        format: fmt,
    };
    let final_path =
        oxiraw::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())?;
    println!("Saved to {}", final_path.display());
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_edit(
    input: &std::path::Path,
    output: &std::path::Path,
    exposure: f32,
    contrast: f32,
    highlights: f32,
    shadows: f32,
    whites: f32,
    blacks: f32,
    temperature: f32,
    tint: f32,
    lut: Option<&std::path::Path>,
    quality: u8,
    format: Option<&str>,
    hsl: &oxiraw::engine::HslChannels,
) -> oxiraw::Result<()> {
    let metadata = oxiraw::metadata::extract_metadata(input);
    let linear = oxiraw::decode::decode(input)?;
    let mut engine = Engine::new(linear);
    let params = engine.params_mut();
    params.exposure = exposure;
    params.contrast = contrast;
    params.highlights = highlights;
    params.shadows = shadows;
    params.whites = whites;
    params.blacks = blacks;
    params.temperature = temperature;
    params.tint = tint;
    params.hsl = hsl.clone();
    if let Some(lut_path) = lut {
        let lut = oxiraw::lut::Lut3D::from_cube_file(lut_path)?;
        engine.set_lut(Some(lut));
    }
    let rendered = engine.render();
    let fmt = format.map(parse_output_format).transpose()?;
    let opts = oxiraw::encode::EncodeOptions {
        jpeg_quality: quality,
        format: fmt,
    };
    let final_path =
        oxiraw::encode::encode_to_file_with_options(&rendered, output, &opts, metadata.as_ref())?;
    println!("Saved to {}", final_path.display());
    Ok(())
}
