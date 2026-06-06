//! Embedded ICC profiles for output color labeling.
//!
//! AgX embeds an ICC profile in every JPEG / PNG / TIFF output so downstream
//! tools (Preview, Photoshop, browsers) identify the color space explicitly
//! rather than guessing. The encoder selects the blob matching the chosen
//! output gamut via [`icc_for`]: sRGB (the default), Display P3, or Adobe RGB.
//!
//! The blobs are synthesized by `crates/agx-profile-gen` via the lcms2
//! crate (MIT). The generated output inherits MIT and ships as bytes;
//! see `docs/contributing/asset-licensing.md` for the licensing rationale
//! and `crates/agx/src/encode/profiles/README.md` for the regeneration
//! recipe.

/// sRGB v4 ICC profile, embedded at compile time.
///
/// The encoder writes these bytes unconditionally into every output file;
/// see the `encode` module-level doc comment for the output-labeling
/// contract.
pub(crate) const SRGB_V4_ICC: &[u8] = include_bytes!("profiles/srgb_v4.icc");

/// Display P3 v4 ICC profile, embedded at compile time.
pub(crate) const DISPLAY_P3_V4_ICC: &[u8] = include_bytes!("profiles/display_p3_v4.icc");

/// Adobe RGB (1998) v4 ICC profile, embedded at compile time.
pub(crate) const ADOBE_RGB_V4_ICC: &[u8] = include_bytes!("profiles/adobe_rgb_v4.icc");

/// The `'static` ICC blob (embedded at compile time) that labels output
/// encoded in `gamut`.
pub(crate) fn icc_for(gamut: crate::encode::OutputGamut) -> &'static [u8] {
    use crate::encode::OutputGamut;
    match gamut {
        OutputGamut::Srgb => SRGB_V4_ICC,
        OutputGamut::DisplayP3 => DISPLAY_P3_V4_ICC,
        OutputGamut::AdobeRgb => ADOBE_RGB_V4_ICC,
    }
}

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
        assert_eq!(&SRGB_V4_ICC[16..20], b"RGB ", "expected RGB color space");
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

    /// The Display P3 and Adobe RGB blobs must also be valid v4 RGB display
    /// profiles in the same size band — guards an accidental swap or a botched
    /// regeneration of either new blob (the sibling sRGB checks above don't
    /// cover them).
    #[test]
    fn new_gamut_blobs_are_v4_rgb_display_profiles() {
        use super::{ADOBE_RGB_V4_ICC, DISPLAY_P3_V4_ICC};
        for (name, blob) in [
            ("display_p3", DISPLAY_P3_V4_ICC),
            ("adobe_rgb", ADOBE_RGB_V4_ICC),
        ] {
            assert!(blob.len() >= 128, "{name}: ICC header is 128 bytes minimum");
            assert_eq!(blob[8], 0x04, "{name}: expected v4 profile");
            assert_eq!(&blob[12..16], b"mntr", "{name}: expected display class");
            assert_eq!(&blob[16..20], b"RGB ", "{name}: expected RGB color space");
            assert!(
                (500..=800).contains(&blob.len()),
                "{name}: expected blob size in 500..=800, got {}",
                blob.len()
            );
        }
    }
}
