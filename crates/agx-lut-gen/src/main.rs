mod looks;
mod transforms;

use std::fs;
use std::io::Write;
use std::path::PathBuf;

const LUT_SIZE: usize = 33;

fn main() {
    let output_dir = parse_output_dir();

    fs::create_dir_all(&output_dir).unwrap_or_else(|e| {
        eprintln!("Error creating output directory {:?}: {}", output_dir, e);
        std::process::exit(1);
    });

    let looks = looks::all_looks();
    println!(
        "Generating {} LUTs ({}x{}x{})...",
        looks.len(),
        LUT_SIZE,
        LUT_SIZE,
        LUT_SIZE
    );

    for look in &looks {
        let path = output_dir.join(format!("{}.cube", look.name));
        write_cube_file(&path, look.name, look.transform);
        println!("  wrote {}", path.display());
    }

    println!("Done.");
}

fn parse_output_dir() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--output-dir" {
            if i + 1 < args.len() {
                return PathBuf::from(&args[i + 1]);
            } else {
                eprintln!("Error: --output-dir requires a value");
                std::process::exit(1);
            }
        }
        i += 1;
    }
    PathBuf::from("crates/agx-e2e/fixtures/looks/luts")
}

fn write_cube_file(
    path: &std::path::Path,
    title: &str,
    transform: fn(f32, f32, f32) -> (f32, f32, f32),
) {
    let mut file = fs::File::create(path).unwrap_or_else(|e| {
        eprintln!("Error creating {:?}: {}", path, e);
        std::process::exit(1);
    });

    // Header
    writeln!(file, "TITLE \"{}\"", title).unwrap();
    writeln!(file, "LUT_3D_SIZE {}", LUT_SIZE).unwrap();
    writeln!(file, "DOMAIN_MIN 0.0 0.0 0.0").unwrap();
    writeln!(file, "DOMAIN_MAX 1.0 1.0 1.0").unwrap();

    let n = (LUT_SIZE - 1) as f32;

    // Standard .cube iteration order: B outer, G middle, R inner
    for bi in 0..LUT_SIZE {
        for gi in 0..LUT_SIZE {
            for ri in 0..LUT_SIZE {
                let r_in = ri as f32 / n;
                let g_in = gi as f32 / n;
                let b_in = bi as f32 / n;

                let (r_out, g_out, b_out) = transform(r_in, g_in, b_in);

                writeln!(file, "{:.6} {:.6} {:.6}", r_out, g_out, b_out).unwrap();
            }
        }
    }
}
