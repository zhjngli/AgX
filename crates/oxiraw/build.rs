fn main() {
    #[cfg(feature = "raw")]
    {
        // Link to LibRaw system library
        println!("cargo:rustc-link-lib=raw");

        // On macOS with Homebrew, add the library search path
        if cfg!(target_os = "macos") {
            if let Ok(output) = std::process::Command::new("brew")
                .args(["--prefix", "libraw"])
                .output()
            {
                if output.status.success() {
                    let prefix = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    println!("cargo:rustc-link-search=native={prefix}/lib");
                }
            }
        }
    }
}
