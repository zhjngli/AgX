//! Input ICC profile parsing and conversion into the engine working space
//! (linear Rec.2020), backed by LittleCMS (lcms2).
//!
//! When an input file embeds an ICC profile, the decoder hands the raw
//! profile bytes plus the *gamma-encoded* decoded pixels to
//! [`convert_to_working_space`], which runs a single lcms2 transform from the
//! parsed profile to a synthetic linear-Rec.2020 destination profile. This
//! replaces the blanket "assume sRGB" path for ICC-tagged inputs.
//!
//! On any malformed or unsupported profile the function returns `Err`; the
//! caller falls back to the sRGB path, so a bad profile never aborts a decode.

use image::Rgb32FImage;
use lcms2::{CIExyY, CIExyYTRIPLE, Intent, PixelFormat, Profile, ToneCurve, Transform};

use crate::error::{AgxError, Result};

/// D65 white point in CIE xyY.
const D65: CIExyY = CIExyY {
    x: 0.3127,
    y: 0.3290,
    Y: 1.0,
};

/// Build the destination profile: linear Rec.2020 (Rec.2020 primaries, D65,
/// gamma 1.0). Returns `Err` if lcms2 cannot construct it.
fn linear_rec2020_profile() -> Result<Profile> {
    let primaries = CIExyYTRIPLE {
        Red: CIExyY {
            x: 0.708,
            y: 0.292,
            Y: 1.0,
        },
        Green: CIExyY {
            x: 0.170,
            y: 0.797,
            Y: 1.0,
        },
        Blue: CIExyY {
            x: 0.131,
            y: 0.046,
            Y: 1.0,
        },
    };
    let linear = ToneCurve::new(1.0);
    Profile::new_rgb(&D65, &primaries, &[&linear, &linear, &linear])
        .map_err(|_| AgxError::Decode("failed to build linear Rec.2020 profile".into()))
}

/// Convert a decoded, gamma-encoded RGB f32 buffer from the color space
/// described by `icc_bytes` into linear Rec.2020, in place.
///
/// `buf` must contain the input's gamma-encoded values normalized to `[0, 1]`
/// (the raw `image`-crate / libheif output, *without* any sRGB linearization
/// applied). lcms2 applies the input profile's own transfer curve and primary
/// conversion.
///
/// Returns `Err` on a malformed/unsupported profile or any lcms2 failure; the
/// caller should fall back to the sRGB assumption.
pub(crate) fn convert_to_working_space(buf: &mut Rgb32FImage, icc_bytes: &[u8]) -> Result<()> {
    let input = Profile::new_icc(icc_bytes)
        .map_err(|_| AgxError::Decode("malformed or unsupported ICC profile".into()))?;
    let dest = linear_rec2020_profile()?;
    let transform = Transform::new(
        &input,
        PixelFormat::RGB_FLT,
        &dest,
        PixelFormat::RGB_FLT,
        Intent::RelativeColorimetric,
    )
    .map_err(|_| AgxError::Decode("failed to build ICC transform".into()))?;

    // Zero-copy: an `Rgb32FImage` stores its samples as a flat row-major
    // `[f32]` with pixel-contiguous layout and no row padding, so the backing
    // buffer reinterprets directly as `[[f32; 3]]` for the in-place transform —
    // no per-image allocation or extra pass. `[f32; 3]` is `Pod` and matches
    // lcms2's `RGB_FLT` pixel size (12 bytes).
    let pixels: &mut [[f32; 3]] = bytemuck::cast_slice_mut(buf);
    transform.transform_in_place(pixels);
    Ok(())
}

/// Build an Adobe RGB (1998) ICC blob via lcms2. Test-only helper shared by
/// this module's tests and `decode`'s ICC-dispatch tests.
#[cfg(test)]
pub(crate) fn adobe_rgb_icc() -> Vec<u8> {
    let primaries = CIExyYTRIPLE {
        Red: CIExyY {
            x: 0.6400,
            y: 0.3300,
            Y: 1.0,
        },
        Green: CIExyY {
            x: 0.2100,
            y: 0.7100,
            Y: 1.0,
        },
        Blue: CIExyY {
            x: 0.1500,
            y: 0.0600,
            Y: 1.0,
        },
    };
    let gamma = ToneCurve::new(2.19921875);
    Profile::new_rgb(&D65, &primaries, &[&gamma, &gamma, &gamma])
        .expect("build adobe rgb profile")
        .icc()
        .expect("serialize adobe rgb icc")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgb};

    /// An sRGB ICC profile should convert to (approximately) the same linear
    /// Rec.2020 values as AgX's built-in sRGB path.
    #[test]
    fn srgb_profile_matches_builtin_path() {
        use crate::color_space::LINEAR_SRGB_TO_LINEAR_REC2020;
        use palette::{LinSrgb, Srgb};

        let srgb_icc = Profile::new_srgb().icc().expect("srgb icc");
        let samples = [[0.5_f32, 0.5, 0.5], [0.8, 0.2, 0.4], [0.1, 0.6, 0.9]];

        for s in samples {
            let mut buf: Rgb32FImage = ImageBuffer::from_pixel(1, 1, Rgb(s));
            convert_to_working_space(&mut buf, &srgb_icc).expect("convert");
            let got = buf.get_pixel(0, 0).0;

            let lin: LinSrgb<f32> = Srgb::new(s[0], s[1], s[2]).into_linear();
            let m = &LINEAR_SRGB_TO_LINEAR_REC2020;
            let expected = [
                m[0][0] * lin.red + m[0][1] * lin.green + m[0][2] * lin.blue,
                m[1][0] * lin.red + m[1][1] * lin.green + m[1][2] * lin.blue,
                m[2][0] * lin.red + m[2][1] * lin.green + m[2][2] * lin.blue,
            ];
            for c in 0..3 {
                assert!(
                    (got[c] - expected[c]).abs() < 3e-3,
                    "channel {c}: got {} expected {}",
                    got[c],
                    expected[c]
                );
            }
        }
    }

    /// An Adobe RGB profile must produce *different* working-space values than
    /// naively assuming sRGB — proving the embedded profile is honored.
    #[test]
    fn adobe_rgb_differs_from_srgb_assumption() {
        use crate::color_space::LINEAR_SRGB_TO_LINEAR_REC2020;
        use palette::{LinSrgb, Srgb};

        let icc = adobe_rgb_icc();
        let red = [1.0_f32, 0.0, 0.0];

        let mut buf: Rgb32FImage = ImageBuffer::from_pixel(1, 1, Rgb(red));
        convert_to_working_space(&mut buf, &icc).expect("convert");
        let adobe = buf.get_pixel(0, 0).0;

        let lin: LinSrgb<f32> = Srgb::new(red[0], red[1], red[2]).into_linear();
        let m = &LINEAR_SRGB_TO_LINEAR_REC2020;
        let srgb_assumed = [
            m[0][0] * lin.red + m[0][1] * lin.green + m[0][2] * lin.blue,
            m[1][0] * lin.red + m[1][1] * lin.green + m[1][2] * lin.blue,
            m[2][0] * lin.red + m[2][1] * lin.green + m[2][2] * lin.blue,
        ];

        let max_diff = (0..3)
            .map(|c| (adobe[c] - srgb_assumed[c]).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            max_diff > 0.02,
            "Adobe RGB red should differ from sRGB-assumed red; max_diff={max_diff}"
        );
        for (c, &v) in adobe.iter().enumerate() {
            assert!(v.is_finite(), "channel {c} not finite");
        }
    }

    /// Garbage profile bytes must return Err (caller falls back), never panic.
    #[test]
    fn malformed_profile_returns_err() {
        let mut buf: Rgb32FImage = ImageBuffer::from_pixel(2, 2, Rgb([0.5_f32, 0.5, 0.5]));
        let result = convert_to_working_space(&mut buf, b"this is not an icc profile");
        assert!(result.is_err());
    }
}
