//! AgX command-line interface.
//!
//! See the [project site](https://zhjngli.github.io/AgX/reference/cli.html)
//! for the full CLI reference.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::path::PathBuf;
use std::process;

use clap::Parser;

use agx::{Engine, Preset};
use agx_cli::{BatchOpts, Cli, Commands, EditArgs, OutputOpts};

mod batch;

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
    engine.set_params(edit.to_params()?);
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
    let params = edit.to_params()?;
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
