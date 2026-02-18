use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

use oxiraw::{Engine, Preset};

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
    Apply {
        /// Input image path
        #[arg(short, long)]
        input: PathBuf,
        /// Preset TOML file path
        #[arg(short, long)]
        preset: PathBuf,
        /// Output image path
        #[arg(short, long)]
        output: PathBuf,
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
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Apply {
            input,
            preset,
            output,
        } => run_apply(&input, &preset, &output),
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
        } => run_edit(
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
        ),
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn run_apply(
    input: &std::path::Path,
    preset_path: &std::path::Path,
    output: &std::path::Path,
) -> oxiraw::Result<()> {
    let linear = oxiraw::decode::decode(input)?;
    let preset = Preset::load_from_file(preset_path)?;
    let mut engine = Engine::new(linear);
    engine.apply_preset(&preset);
    let rendered = engine.render();
    oxiraw::encode::encode_to_file(&rendered, output)?;
    println!("Saved to {}", output.display());
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
) -> oxiraw::Result<()> {
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
    if let Some(lut_path) = lut {
        let lut = oxiraw::lut::Lut3D::from_cube_file(lut_path)?;
        engine.set_lut(Some(lut));
    }
    let rendered = engine.render();
    oxiraw::encode::encode_to_file(&rendered, output)?;
    println!("Saved to {}", output.display());
    Ok(())
}
