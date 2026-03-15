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
        build.compile("agx_libraw_meta");
    }
}
