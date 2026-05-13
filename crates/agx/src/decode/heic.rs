//! HEIC/HEIF format decoding via libheif FFI.
//!
//! This module provides FFI bindings to libheif's C API and safe
//! `decode_heic()` / `extract_heic_metadata()` functions that produce
//! the same linear sRGB f32 output contract as the other decode paths.
//!
//! libheif handles the HEIF container and orchestrates the codec
//! backend (typically libde265 for HEVC). Orientation transformations
//! declared in the file's `irot`/`imir` boxes are applied by libheif
//! during decode.

use std::ffi::{c_char, c_int, c_void, CString};
use std::path::Path;

use image::Rgb32FImage;

use crate::error::{AgxError, Result};

// --- FFI types ---

#[allow(non_camel_case_types)]
#[repr(C)]
struct heif_context {
    _opaque: [u8; 0],
}

#[allow(non_camel_case_types)]
#[repr(C)]
struct heif_image_handle {
    _opaque: [u8; 0],
}

#[allow(non_camel_case_types)]
#[repr(C)]
struct heif_image {
    _opaque: [u8; 0],
}

// libheif's `heif_error_code` and `heif_suberror_code` are C enums. On Clang
// and GCC for x86-64 and arm64 — the platforms this crate targets — C enums
// are int-sized, so we represent them as `c_int`. If porting to a target
// where the C compiler uses `-fshort-enums`, this struct layout would need
// to be revisited.
#[allow(non_camel_case_types)]
#[repr(C)]
struct heif_error {
    code: c_int,
    subcode: c_int,
    message: *const c_char,
}

// Chroma format enum values from libheif (see libheif/heif.h)
#[allow(dead_code)]
const HEIF_COLORSPACE_RGB: c_int = 1;
#[allow(dead_code)]
const HEIF_CHROMA_INTERLEAVED_RGB: c_int = 10;
const HEIF_CHROMA_INTERLEAVED_RRGGBB_LE: c_int = 14;

// Channel enum for plane access
#[allow(dead_code)]
const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

// heif_color_profile_type enum values (FOURCC codes)
const HEIF_COLOR_PROFILE_TYPE_NCLX: c_int = 0x6e636c78; // 'nclx'
const HEIF_COLOR_PROFILE_TYPE_RICC: c_int = 0x72494343; // 'rICC'
const HEIF_COLOR_PROFILE_TYPE_PROF: c_int = 0x70726f66; // 'prof'

// ITU-T H.273 color_primaries values
const COLOR_PRIMARIES_BT709: u32 = 1;
const COLOR_PRIMARIES_BT2020: u32 = 9;
const COLOR_PRIMARIES_SMPTE_EG432_DISPLAY_P3: u32 = 12; // Display P3 (D65, used by iPhone)

// libheif's `heif_color_profile_nclx` mirrors a C struct where color_primaries,
// transfer_characteristics, and matrix_coefficients are C enums. On Clang/GCC
// for x86-64 and arm64 — the platforms this crate targets — C enums are
// int-sized (4 bytes). The Rust struct must match this layout exactly; a size
// mismatch causes field reads at wrong offsets and silently breaks gamut
// detection. The `_pad0`/`_pad1` fields provide the alignment padding that the
// C compiler inserts, making the struct exactly 52 bytes.
#[repr(C)]
#[allow(non_camel_case_types)]
#[allow(dead_code)]
struct heif_color_profile_nclx {
    version: u8,
    _pad0: [u8; 3],
    color_primaries: u32,
    transfer_characteristics: u32,
    matrix_coefficients: u32,
    full_range_flag: u8,
    _pad1: [u8; 3],
    color_primary_red_x: f32,
    color_primary_red_y: f32,
    color_primary_green_x: f32,
    color_primary_green_y: f32,
    color_primary_blue_x: f32,
    color_primary_blue_y: f32,
    color_primary_white_x: f32,
    color_primary_white_y: f32,
}

extern "C" {
    fn heif_context_alloc() -> *mut heif_context;
    fn heif_context_free(ctx: *mut heif_context);
    fn heif_context_read_from_file(
        ctx: *mut heif_context,
        filename: *const c_char,
        options: *const c_void,
    ) -> heif_error;
    fn heif_context_get_primary_image_handle(
        ctx: *mut heif_context,
        out_handle: *mut *mut heif_image_handle,
    ) -> heif_error;
    fn heif_image_handle_release(handle: *const heif_image_handle);
    fn heif_image_handle_get_luma_bits_per_pixel(handle: *const heif_image_handle) -> c_int;
    fn heif_decode_image(
        handle: *const heif_image_handle,
        out_image: *mut *mut heif_image,
        colorspace: c_int,
        chroma: c_int,
        options: *const c_void,
    ) -> heif_error;
    fn heif_image_release(img: *const heif_image);
    fn heif_image_get_plane_readonly(
        img: *const heif_image,
        channel: c_int,
        out_stride: *mut c_int,
    ) -> *const u8;
    fn heif_image_get_width(img: *const heif_image, channel: c_int) -> c_int;
    fn heif_image_get_height(img: *const heif_image, channel: c_int) -> c_int;
    fn heif_image_handle_get_color_profile_type(handle: *const heif_image_handle) -> c_int;
    fn heif_image_handle_get_nclx_color_profile(
        handle: *const heif_image_handle,
        out_data: *mut *mut heif_color_profile_nclx,
    ) -> heif_error;
    fn heif_nclx_color_profile_free(profile: *mut heif_color_profile_nclx);
}

// --- Error helpers ---

#[allow(dead_code)]
unsafe fn heif_error_message(err: &heif_error) -> String {
    if err.message.is_null() {
        return format!("libheif error code {}", err.code);
    }
    std::ffi::CStr::from_ptr(err.message)
        .to_string_lossy()
        .into_owned()
}

#[allow(dead_code)]
unsafe fn check_heif(err: heif_error) -> Result<()> {
    if err.code == 0 {
        Ok(())
    } else {
        Err(AgxError::Decode(format!(
            "libheif: {}",
            heif_error_message(&err)
        )))
    }
}

// --- RAII wrappers ---

#[allow(dead_code)]
struct HeifContext {
    ptr: *mut heif_context,
}

#[allow(dead_code)]
impl HeifContext {
    fn new() -> Result<Self> {
        let ptr = unsafe { heif_context_alloc() };
        if ptr.is_null() {
            return Err(AgxError::Decode(
                "libheif: failed to allocate context".into(),
            ));
        }
        Ok(Self { ptr })
    }

    fn read_from_file(&self, path: &Path) -> Result<()> {
        let c_path = CString::new(
            path.to_str()
                .ok_or_else(|| AgxError::Decode("invalid file path encoding".into()))?,
        )
        .map_err(|_| AgxError::Decode("file path contains null byte".into()))?;
        unsafe {
            check_heif(heif_context_read_from_file(
                self.ptr,
                c_path.as_ptr(),
                std::ptr::null(),
            ))
        }
    }

    fn primary_image_handle(&self) -> Result<HeifImageHandle> {
        let mut handle: *mut heif_image_handle = std::ptr::null_mut();
        unsafe {
            check_heif(heif_context_get_primary_image_handle(self.ptr, &mut handle))?;
        }
        if handle.is_null() {
            return Err(AgxError::Decode(
                "libheif: file has no primary image".into(),
            ));
        }
        Ok(HeifImageHandle { ptr: handle })
    }
}

impl Drop for HeifContext {
    fn drop(&mut self) {
        unsafe { heif_context_free(self.ptr) };
    }
}

#[allow(dead_code)]
struct HeifImageHandle {
    ptr: *mut heif_image_handle,
}

#[allow(dead_code)]
impl HeifImageHandle {
    fn luma_bits_per_pixel(&self) -> i32 {
        unsafe { heif_image_handle_get_luma_bits_per_pixel(self.ptr) as i32 }
    }

    fn decode(&self, colorspace: c_int, chroma: c_int) -> Result<HeifImage> {
        let mut img: *mut heif_image = std::ptr::null_mut();
        unsafe {
            check_heif(heif_decode_image(
                self.ptr,
                &mut img,
                colorspace,
                chroma,
                std::ptr::null(),
            ))?;
        }
        if img.is_null() {
            return Err(AgxError::Decode(
                "libheif: decode returned null image".into(),
            ));
        }
        Ok(HeifImage { ptr: img })
    }
}

impl Drop for HeifImageHandle {
    fn drop(&mut self) {
        unsafe { heif_image_handle_release(self.ptr) };
    }
}

#[allow(dead_code)]
struct HeifImage {
    ptr: *mut heif_image,
}

#[allow(dead_code)]
impl HeifImage {
    fn width(&self) -> u32 {
        unsafe { heif_image_get_width(self.ptr, HEIF_CHANNEL_INTERLEAVED) as u32 }
    }

    fn height(&self) -> u32 {
        unsafe { heif_image_get_height(self.ptr, HEIF_CHANNEL_INTERLEAVED) as u32 }
    }

    /// Returns a borrowed pointer + stride for the interleaved channel.
    /// Caller must not retain references past the HeifImage's lifetime.
    fn plane_readonly(&self) -> (*const u8, i32) {
        let mut stride: c_int = 0;
        let data = unsafe {
            heif_image_get_plane_readonly(self.ptr, HEIF_CHANNEL_INTERLEAVED, &mut stride)
        };
        (data, stride as i32)
    }
}

impl Drop for HeifImage {
    fn drop(&mut self) {
        unsafe { heif_image_release(self.ptr) };
    }
}

// --- Color space detection and gamut mapping ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceColorSpace {
    /// BT.709 / sRGB primaries; no matrix conversion needed.
    Srgb,
    /// Display P3 primaries; matrix-converted to sRGB in linear space.
    DisplayP3,
    /// BT.2020 primaries; matrix-converted to sRGB in linear space.
    Bt2020,
}

/// 3x3 matrix to convert Display P3 (D65) linear values into linear sRGB.
const P3_TO_SRGB: [[f32; 3]; 3] = [
    [1.2249, -0.2247, 0.0000],
    [-0.0420, 1.0419, 0.0000],
    [-0.0197, -0.0786, 1.0982],
];

/// 3x3 matrix to convert BT.2020 (D65) linear values into linear sRGB.
const BT2020_TO_SRGB: [[f32; 3]; 3] = [
    [1.6605, -0.5876, -0.0728],
    [-0.1246, 1.1329, -0.0083],
    [-0.0182, -0.1006, 1.1187],
];

fn apply_matrix(rgb: [f32; 3], m: &[[f32; 3]; 3]) -> [f32; 3] {
    [
        m[0][0] * rgb[0] + m[0][1] * rgb[1] + m[0][2] * rgb[2],
        m[1][0] * rgb[0] + m[1][1] * rgb[1] + m[1][2] * rgb[2],
        m[2][0] * rgb[0] + m[2][1] * rgb[1] + m[2][2] * rgb[2],
    ]
}

/// Inspect the file's color profile and classify the source space.
///
/// Returns `SourceColorSpace::Srgb` as a safe fallback when the file
/// declares an ICC profile (deferred to color-management work) or an
/// unknown NCLX combination. In both cases a stderr warning is emitted.
fn probe_source_color_space(handle: &HeifImageHandle) -> SourceColorSpace {
    let profile_type = unsafe { heif_image_handle_get_color_profile_type(handle.ptr) };

    if profile_type == HEIF_COLOR_PROFILE_TYPE_RICC || profile_type == HEIF_COLOR_PROFILE_TYPE_PROF
    {
        eprintln!(
            "agx: HEIC source declares an ICC color profile; gamut-mapping as sRGB. \
             Full ICC support requires the color-management work."
        );
        return SourceColorSpace::Srgb;
    }

    if profile_type != HEIF_COLOR_PROFILE_TYPE_NCLX {
        // No profile declared — treat as sRGB silently (common for transcoded files).
        return SourceColorSpace::Srgb;
    }

    let mut nclx_ptr: *mut heif_color_profile_nclx = std::ptr::null_mut();
    let err = unsafe { heif_image_handle_get_nclx_color_profile(handle.ptr, &mut nclx_ptr) };
    if err.code != 0 || nclx_ptr.is_null() {
        return SourceColorSpace::Srgb;
    }

    let primaries = unsafe { (*nclx_ptr).color_primaries };

    // Release the libheif-allocated struct.
    unsafe { heif_nclx_color_profile_free(nclx_ptr) };

    match primaries {
        COLOR_PRIMARIES_BT709 => SourceColorSpace::Srgb,
        COLOR_PRIMARIES_SMPTE_EG432_DISPLAY_P3 => SourceColorSpace::DisplayP3,
        COLOR_PRIMARIES_BT2020 => SourceColorSpace::Bt2020,
        _ => {
            eprintln!(
                "agx: HEIC source NCLX color_primaries={primaries} not recognized; \
                 gamut-mapping as sRGB."
            );
            SourceColorSpace::Srgb
        }
    }
}

// --- Public API ---

/// Decode a HEIC/HEIF file into linear sRGB f32.
///
/// libheif handles the container, codec backend, and orientation
/// transformations. This function inspects the source bit depth,
/// requests RGB-interleaved decode in the appropriate chroma layout,
/// and converts the pixel data to the engine's linear sRGB f32 contract.
///
/// # Supported sources
///
/// 8-bit and 10-bit HEIF images. iPhone HEIC captures are the primary
/// target. Multi-image HEIF containers are read for their primary image
/// only; auxiliary images (depth, burst, alternate exposures) are not
/// surfaced.
///
/// # Color space
///
/// Source primaries declared as BT.709 or sRGB are treated as sRGB
/// directly. Display P3 and BT.2020 sources are gamut-mapped to sRGB
/// at decode (see the design doc for the deferred wide-gamut work).
/// ICC profile and unknown matrices fall back to "treat as sRGB"
/// with a stderr warning.
pub fn decode_heic(path: &Path) -> Result<Rgb32FImage> {
    use palette::{LinSrgb, Srgb};

    let ctx = HeifContext::new()?;
    ctx.read_from_file(path)?;
    let handle = ctx.primary_image_handle()?;
    let source_space = probe_source_color_space(&handle);

    let bits = handle.luma_bits_per_pixel();
    let (chroma, bytes_per_pixel) = match bits {
        8 => (HEIF_CHROMA_INTERLEAVED_RGB, 3),
        9 | 10 => (HEIF_CHROMA_INTERLEAVED_RRGGBB_LE, 6),
        _ => {
            return Err(AgxError::Decode(format!(
                "libheif: unsupported bit depth {bits}"
            )));
        }
    };

    let img = handle.decode(HEIF_COLORSPACE_RGB, chroma)?;
    let width = img.width();
    let height = img.height();
    let (data, stride) = img.plane_readonly();
    if data.is_null() {
        return Err(AgxError::Decode(
            "libheif: decoded image has no pixel data".into(),
        ));
    }
    if stride <= 0 {
        return Err(AgxError::Decode(
            "libheif: decoded plane has invalid stride".into(),
        ));
    }
    let stride = stride as usize;

    // Safety: `data` points to a buffer of at least `stride * height` bytes
    // allocated by libheif and owned by `img`. The slice is dropped before
    // `img` (which holds the allocation) goes out of scope. `stride > 0` was
    // asserted above; on any real HEIF the product fits comfortably in `usize`.
    let pixel_slice: &[u8] = unsafe { std::slice::from_raw_parts(data, stride * height as usize) };

    let buf = if bits == 8 {
        Rgb32FImage::from_fn(width, height, |x, y| {
            let row_offset = (y as usize) * stride;
            let col_offset = (x as usize) * bytes_per_pixel;
            let i = row_offset + col_offset;
            let sr = pixel_slice[i] as f32 / 255.0;
            let sg = pixel_slice[i + 1] as f32 / 255.0;
            let sb = pixel_slice[i + 2] as f32 / 255.0;
            let lin: LinSrgb<f32> = Srgb::new(sr, sg, sb).into_linear();
            let lin_rgb = [lin.red, lin.green, lin.blue];
            let out = match source_space {
                SourceColorSpace::Srgb => lin_rgb,
                SourceColorSpace::DisplayP3 => apply_matrix(lin_rgb, &P3_TO_SRGB),
                SourceColorSpace::Bt2020 => apply_matrix(lin_rgb, &BT2020_TO_SRGB),
            };
            image::Rgb(out)
        })
    } else {
        // 9 or 10-bit values packed into 16-bit little-endian containers.
        let max_value = ((1u32 << bits) - 1) as f32;
        Rgb32FImage::from_fn(width, height, |x, y| {
            let row_offset = (y as usize) * stride;
            let col_offset = (x as usize) * bytes_per_pixel;
            let i = row_offset + col_offset;
            let r_raw = u16::from_le_bytes([pixel_slice[i], pixel_slice[i + 1]]);
            let g_raw = u16::from_le_bytes([pixel_slice[i + 2], pixel_slice[i + 3]]);
            let b_raw = u16::from_le_bytes([pixel_slice[i + 4], pixel_slice[i + 5]]);
            let sr = r_raw as f32 / max_value;
            let sg = g_raw as f32 / max_value;
            let sb = b_raw as f32 / max_value;
            let lin: LinSrgb<f32> = Srgb::new(sr, sg, sb).into_linear();
            let lin_rgb = [lin.red, lin.green, lin.blue];
            let out = match source_space {
                SourceColorSpace::Srgb => lin_rgb,
                SourceColorSpace::DisplayP3 => apply_matrix(lin_rgb, &P3_TO_SRGB),
                SourceColorSpace::Bt2020 => apply_matrix(lin_rgb, &BT2020_TO_SRGB),
            };
            image::Rgb(out)
        })
    };

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heif_context_init_and_drop() {
        let ctx = HeifContext::new().unwrap();
        drop(ctx);
    }

    #[test]
    fn decode_heic_nonexistent_file_returns_error() {
        let result = decode_heic(Path::new("/nonexistent/photo.heic"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("libheif"),
            "Error should mention libheif: {err_msg}"
        );
    }

    #[test]
    fn nclx_struct_size_matches_libheif() {
        // libheif's heif_color_profile_nclx is 52 bytes on the target platforms
        // (Clang/GCC, x86-64 and arm64). A drift here means our FFI reads at
        // wrong offsets and gamut detection silently fails.
        assert_eq!(std::mem::size_of::<heif_color_profile_nclx>(), 52);
    }

    #[test]
    fn apply_matrix_identity_preserves_input() {
        let id = [[1.0f32, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        let v = [0.5f32, 0.3, 0.8];
        let out = apply_matrix(v, &id);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert!((out[1] - 0.3).abs() < 1e-6);
        assert!((out[2] - 0.8).abs() < 1e-6);
    }
}
