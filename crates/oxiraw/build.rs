fn main() {
    #[cfg(feature = "raw")]
    {
        println!("cargo:rustc-link-lib=raw");

        let mut libraw_include = None;

        if cfg!(target_os = "macos") {
            if let Ok(output) = std::process::Command::new("brew")
                .args(["--prefix", "libraw"])
                .output()
            {
                if output.status.success() {
                    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    println!("cargo:rustc-link-search=native={prefix}/lib");
                    libraw_include = Some(format!("{prefix}/include"));
                }
            }
        }

        let mut build = cc::Build::new();
        build.file("src/decode/libraw_meta.c");
        if let Some(ref inc) = libraw_include {
            build.include(inc);
        }

        // Fallback: if libraw/libraw.h is not found system-wide, use a minimal stub
        // so the crate compiles (raw decoding will fail at link/runtime without the real lib).
        let manifest_dir =
            std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        let stub_dir = format!("{manifest_dir}/libraw-stub");
        if std::path::Path::new(&stub_dir).exists() && libraw_include.is_none() {
            build.include(&stub_dir);
        }

        build.compile("oxiraw_libraw_meta");
    }
}
