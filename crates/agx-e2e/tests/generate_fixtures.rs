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

/// Build a ProPhoto RGB (ROMM RGB) ICC profile blob via lcms2. ProPhoto is a
/// very wide gamut: D50 white point, gamma 1.8, primaries that extend well
/// beyond Rec.2020 (the blue/green are partly outside the spectral locus). It
/// exercises the widest-gamut input conversion the e2e suite covers.
fn prophoto_icc() -> Vec<u8> {
    let d50 = CIExyY {
        x: 0.3457,
        y: 0.3585,
        Y: 1.0,
    };
    let primaries = CIExyYTRIPLE {
        Red: CIExyY {
            x: 0.7347,
            y: 0.2653,
            Y: 1.0,
        },
        Green: CIExyY {
            x: 0.1596,
            y: 0.8404,
            Y: 1.0,
        },
        Blue: CIExyY {
            x: 0.0366,
            y: 0.0001,
            Y: 1.0,
        },
    };
    let gamma = ToneCurve::new(1.8);
    Profile::new_rgb(&d50, &primaries, &[&gamma, &gamma, &gamma])
        .expect("build prophoto profile")
        .icc()
        .expect("serialize prophoto icc")
}

/// Shared 256×256 saturated RGB gradient used by every synthetic ICC fixture.
/// Wide-gamut content (saturated primaries) so the embedded profile's wider
/// gamut is observable after conversion into the working space.
fn gradient_256() -> ImageBuffer<Rgb<u8>, Vec<u8>> {
    let w = 256u32;
    let h = 256u32;
    ImageBuffer::from_fn(w, h, |x, y| {
        let tx = x as f32 / (w - 1) as f32;
        let ty = y as f32 / (h - 1) as f32;
        Rgb([
            (tx * 255.0) as u8,
            ((1.0 - ty) * 255.0) as u8,
            ((ty * 0.5 + 0.25) * 255.0) as u8,
        ])
    })
}

/// Generate `fixtures/jpeg/adobe_rgb_gradient.jpg`: a saturated gradient
/// encoded as JPEG with an embedded Adobe RGB (1998) ICC profile. Exercises
/// the SP3 input-ICC read path — the decoder must parse the profile and
/// convert wide-gamut color into the working space rather than assuming sRGB.
/// Deterministic: identical pixels + profile every run.
#[test]
#[ignore = "dev-only fixture generator; writes a committed binary"]
fn gen_adobe_rgb_gradient() {
    let mut jpeg_bytes = Vec::new();
    gradient_256()
        .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
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

/// Generate `fixtures/png/prophoto_gradient.png`: the shared gradient encoded
/// as PNG with an embedded ProPhoto RGB ICC profile (iCCP chunk). Exercises
/// PNG ICC extraction *and* the widest-gamut input conversion in the suite.
#[test]
#[ignore = "dev-only fixture generator; writes a committed binary"]
fn gen_prophoto_gradient() {
    let mut png_bytes = Vec::new();
    gradient_256()
        .write_with_encoder(image::codecs::png::PngEncoder::new(&mut png_bytes))
        .expect("encode png");

    let mut png = img_parts::png::Png::from_bytes(png_bytes.into()).expect("parse generated png");
    png.set_icc_profile(Some(prophoto_icc().into()));
    let mut tagged = Vec::new();
    png.encoder()
        .write_to(&mut tagged)
        .expect("write tagged png");

    let out = fixture_path("png/prophoto_gradient.png");
    std::fs::write(&out, &tagged).expect("write fixture");
    eprintln!("wrote {}", out.display());
}

/// Generate `fixtures/tiff/adobe_rgb_gradient.tiff`: the shared gradient
/// encoded as TIFF with an embedded Adobe RGB ICC profile (ICCProfile tag
/// 0x8773). Exercises TIFF ICC-tag extraction. img-parts cannot write TIFF, so
/// this writes the tag directly via the `tiff` crate — the same API the encoder
/// uses on the output side.
#[test]
#[ignore = "dev-only fixture generator; writes a committed binary"]
fn gen_adobe_rgb_tiff() {
    use tiff::encoder::{colortype, TiffEncoder};
    use tiff::tags::Tag;

    let img = gradient_256();
    let (w, h) = (img.width(), img.height());
    let raw = img.into_raw();
    let icc = adobe_rgb_icc();

    let mut buf = Vec::new();
    {
        let mut tiff = TiffEncoder::new(std::io::Cursor::new(&mut buf)).expect("tiff encoder");
        let mut image = tiff
            .new_image::<colortype::RGB8>(w, h)
            .expect("tiff new_image");
        image
            .encoder()
            .write_tag(Tag::IccProfile, icc.as_slice())
            .expect("write icc tag");
        image.write_data(&raw).expect("write tiff data");
    }

    let out = fixture_path("tiff/adobe_rgb_gradient.tiff");
    std::fs::write(&out, &buf).expect("write fixture");
    eprintln!("wrote {}", out.display());
}
