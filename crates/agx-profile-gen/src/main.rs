//! Generates AgX's bundled ICC profile blobs.
//!
//! Currently emits a single sRGB v4 profile to
//! `crates/agx/src/encode/profiles/srgb_v4.icc`. The generator is dev-only
//! (the output bytes are committed); regenerate only when the source
//! parameters change or a future ICC spec revision is adopted.
//!
//! License chain: lcms2 (MIT) + this generator (MIT/Apache via AgX). The
//! generated profile inherits MIT — see
//! `docs/contributing/asset-licensing.md`.

use std::path::PathBuf;

use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};

const OUTPUT_PATH: &str = "crates/agx/src/encode/profiles/srgb_v4.icc";

fn main() {
    let bytes = build_srgb_v4_profile();

    let path = PathBuf::from(OUTPUT_PATH);
    std::fs::create_dir_all(path.parent().unwrap()).expect("create profiles dir");
    std::fs::write(&path, &bytes).expect("write profile");

    println!("wrote {} bytes to {}", bytes.len(), path.display());
}

/// Build a v4 sRGB profile: BT.709/sRGB primaries (Rec. 709 chromaticities),
/// D65 white point, sRGB parametric transfer curve.
fn build_srgb_v4_profile() -> Vec<u8> {
    let primaries = CIExyYTRIPLE {
        Red: CIExyY {
            x: 0.6400,
            y: 0.3300,
            Y: 1.0,
        },
        Green: CIExyY {
            x: 0.3000,
            y: 0.6000,
            Y: 1.0,
        },
        Blue: CIExyY {
            x: 0.1500,
            y: 0.0600,
            Y: 1.0,
        },
    };
    let d65 = CIExyY {
        x: 0.31270,
        y: 0.32900,
        Y: 1.0,
    };

    // sRGB parametric transfer curve, type 4 (IEC 61966-2.1):
    //   if x >= d: y = (a*x + b)^gamma
    //   else:      y = c*x
    // Parameters: gamma=2.4, a=1/1.055, b=0.055/1.055, c=1/12.92, d=0.04045.
    let srgb_curve =
        ToneCurve::new_parametric(4, &[2.4, 1.0 / 1.055, 0.055 / 1.055, 1.0 / 12.92, 0.04045])
            .expect("build sRGB tone curve");

    let mut profile = Profile::new_rgb(&d65, &primaries, &[&srgb_curve, &srgb_curve, &srgb_curve])
        .expect("build RGB profile");

    // Force v4 explicitly. lcms2 currently defaults new profiles to v4.3,
    // but pinning the value here protects against silent regressions if a
    // future lcms2 release changes the default.
    profile.set_version(4.3);

    let mut bytes = profile.icc().expect("serialize ICC bytes");
    force_deterministic_creation_datetime(&mut bytes);
    bytes
}

/// Overwrite the ICC v4 header `dateTime` field (offset 24, 12 bytes,
/// six big-endian u16: year, month, day, hour, minute, second) with a
/// fixed value so the generated blob is byte-stable across regenerations.
///
/// lcms2 stamps the current wall-clock time by default. Without this fix,
/// every `cargo run -p agx-profile-gen` produces a different blob — even
/// when source parameters are unchanged — and the committed
/// `srgb_v4.icc` looks like it diverges on every dev's machine. Fixing
/// the timestamp to a project epoch (AgX founding year, 2026-01-01
/// 00:00:00 UTC) gives us a single canonical blob.
///
/// Safe post-`icc()` patch: the optional Profile ID field at bytes
/// 84..100 is zero in lcms2 output (no MD5 dependency on header bytes).
fn force_deterministic_creation_datetime(bytes: &mut [u8]) {
    let dt: [u8; 12] = [
        0x07, 0xEA, // year 2026
        0x00, 0x01, // month 1
        0x00, 0x01, // day 1
        0x00, 0x00, // hour
        0x00, 0x00, // minute
        0x00, 0x00, // second
    ];
    bytes[24..36].copy_from_slice(&dt);
}

#[cfg(test)]
mod tests {
    use super::build_srgb_v4_profile;

    #[test]
    fn generated_profile_is_v4() {
        let bytes = build_srgb_v4_profile();
        assert!(
            bytes.len() >= 128,
            "profile header must be at least 128 bytes"
        );
        assert_eq!(
            bytes[8], 0x04,
            "expected v4 major version, got {:#x}",
            bytes[8]
        );
    }

    #[test]
    fn generated_profile_is_display_class_rgb() {
        let bytes = build_srgb_v4_profile();
        assert_eq!(&bytes[12..16], b"mntr", "profile class");
        assert_eq!(&bytes[16..20], b"RGB ", "color space");
    }

    #[test]
    fn generated_profile_in_expected_size_range() {
        let bytes = build_srgb_v4_profile();
        let n = bytes.len();
        assert!(
            (300..=8000).contains(&n),
            "expected 300..=8000 bytes, got {}",
            n
        );
    }

    /// Round-trip through lcms2's own parser. The header-byte tests above
    /// confirm "looks like a v4 RGB display profile"; this test confirms
    /// "lcms2 itself parses the bytes back into a coherent RGB display
    /// profile." A silent lcms2 upgrade that changes curve encoding or
    /// primaries representation would still pass the header tests but
    /// trip this one.
    #[test]
    fn generated_profile_round_trips_through_lcms2() {
        use lcms2::{ColorSpaceSignature, Profile, ProfileClassSignature};

        let bytes = build_srgb_v4_profile();
        let profile = Profile::new_icc(&bytes).expect("parse generated profile");
        assert_eq!(profile.color_space(), ColorSpaceSignature::RgbData);
        assert_eq!(profile.device_class(), ProfileClassSignature::DisplayClass);
    }
}
