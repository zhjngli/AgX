//! Dev-only fixture generators. Not part of the normal test run — each is
//! `#[ignore]`d and writes a committed binary fixture. Regenerate with, e.g.:
//!
//! ```bash
//! cargo test -p agx-e2e --test generate_fixtures -- --ignored gen_adobe_rgb_gradient
//! ```

use agx_e2e::fixture_path;
use image::{ImageBuffer, Rgb};
use img_parts::ImageICC;
use lcms2::{CIExyY, CIExyYTRIPLE, Profile, ToneCurve};

/// Build an Adobe RGB (1998) ICC profile blob via lcms2.
fn adobe_rgb_icc() -> Vec<u8> {
    let d65 = CIExyY {
        x: 0.3127,
        y: 0.3290,
        Y: 1.0,
    };
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
    Profile::new_rgb(&d65, &primaries, &[&gamma, &gamma, &gamma])
        .expect("build adobe rgb profile")
        .icc()
        .expect("serialize adobe rgb icc")
}

/// Generate `fixtures/jpeg/adobe_rgb_gradient.jpg`: a saturated gradient
/// encoded as JPEG with an embedded Adobe RGB (1998) ICC profile. Exercises
/// the SP3 input-ICC read path — the decoder must parse the profile and
/// convert wide-gamut color into the working space rather than assuming sRGB.
/// Deterministic: identical pixels + profile every run.
#[test]
#[ignore = "dev-only fixture generator; writes a committed binary"]
fn gen_adobe_rgb_gradient() {
    let w = 256u32;
    let h = 256u32;
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(w, h, |x, y| {
        let tx = x as f32 / (w - 1) as f32;
        let ty = y as f32 / (h - 1) as f32;
        Rgb([
            (tx * 255.0) as u8,
            ((1.0 - ty) * 255.0) as u8,
            ((ty * 0.5 + 0.25) * 255.0) as u8,
        ])
    });

    let mut jpeg_bytes = Vec::new();
    img.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut jpeg_bytes,
        95,
    ))
    .expect("encode jpeg");

    let mut jpeg =
        img_parts::jpeg::Jpeg::from_bytes(jpeg_bytes.into()).expect("parse generated jpeg");
    jpeg.set_icc_profile(Some(adobe_rgb_icc().into()));
    let mut tagged = Vec::new();
    jpeg.encoder()
        .write_to(&mut tagged)
        .expect("write tagged jpeg");

    let out = fixture_path("jpeg/adobe_rgb_gradient.jpg");
    std::fs::write(&out, &tagged).expect("write fixture");
    eprintln!("wrote {}", out.display());
}
