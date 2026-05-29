//! Embedded sRGB v4 ICC profile for output color labeling.
//!
//! AgX encodes every JPEG / PNG / TIFF output as sRGB and embeds this
//! profile so downstream tools (Preview, Photoshop, browsers) identify
//! the color space explicitly rather than guessing.
//!
//! The blob is synthesized by `crates/agx-profile-gen` via the lcms2
//! crate (MIT). The generated output inherits MIT and ships as bytes;
//! see `docs/contributing/asset-licensing.md` for the licensing rationale
//! and `crates/agx/src/encode/profiles/README.md` for the regeneration
//! recipe.

/// sRGB v4 ICC profile, embedded at compile time.
///
/// The encoder writes these bytes unconditionally into every output file;
/// see the `encode` module-level doc comment for the output-labeling
/// contract.
// `dead_code` allow is temporary: Tasks 5/6/7 (color management SP2) wire
// this const into the JPEG/PNG/TIFF encoder paths; once consumed, the
// attribute should be removed.
#[allow(dead_code)]
pub(crate) const SRGB_V4_ICC: &[u8] = include_bytes!("profiles/srgb_v4.icc");

#[cfg(test)]
mod tests {
    use super::SRGB_V4_ICC;

    /// The blob must be a valid v4 ICC profile. Profile version lives at
    /// bytes 8..12; the high byte (offset 8) is the major version. v4 → 0x04.
    #[test]
    fn srgb_v4_icc_blob_is_v4() {
        assert!(SRGB_V4_ICC.len() >= 128, "ICC header is 128 bytes minimum");
        assert_eq!(
            SRGB_V4_ICC[8], 0x04,
            "expected v4 profile, got major version {:#x}",
            SRGB_V4_ICC[8]
        );
    }

    /// Profile class at offset 12..16 must be "mntr" (display device profile).
    #[test]
    fn srgb_v4_icc_blob_is_display_class() {
        assert_eq!(
            &SRGB_V4_ICC[12..16],
            b"mntr",
            "expected display-class profile (mntr)"
        );
    }

    /// Color space at offset 16..20 must be "RGB " (4-char field, space-padded).
    #[test]
    fn srgb_v4_icc_blob_is_rgb_color_space() {
        assert_eq!(
            &SRGB_V4_ICC[16..20],
            b"RGB ",
            "expected RGB color space"
        );
    }

    /// Catch accidental swap to a different profile. Our generated blob is
    /// 584 bytes (committed); the range here is tight because the embedded
    /// bytes don't change unless someone regenerates intentionally. A swap
    /// to the ~60 KB ICC consortium preference profile, or any other
    /// substitution, trips this immediately.
    #[test]
    fn srgb_v4_icc_blob_size_in_expected_range() {
        let n = SRGB_V4_ICC.len();
        assert!(
            (500..=800).contains(&n),
            "expected blob size in 500..=800, got {}",
            n
        );
    }
}
