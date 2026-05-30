//! HEIC/HEIF format decoding via libheif FFI.
//!
//! This module provides FFI bindings to libheif's C API and safe
//! `decode_heic()` / `extract_heic_metadata()` functions that produce
//! the same linear Rec.2020 f32 output contract as the other decode paths.
//!
//! libheif handles the HEIF container and orchestrates the codec
//! backend (typically libde265 for HEVC). Orientation transformations
//! declared in the file's `irot`/`imir` boxes are applied by libheif
//! during decode.

use std::ffi::{c_char, c_int, c_void, CString};
use std::path::Path;

use image::Rgb32FImage;

use crate::color_space::{
    LINEAR_BT2020_TO_LINEAR_REC2020, LINEAR_P3_TO_LINEAR_REC2020, LINEAR_SRGB_TO_LINEAR_REC2020,
};
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
    #[cfg(feature = "icc")]
    fn heif_image_handle_get_raw_color_profile_size(handle: *const heif_image_handle) -> usize;
    #[cfg(feature = "icc")]
    fn heif_image_handle_get_raw_color_profile(
        handle: *const heif_image_handle,
        out_data: *mut c_void,
    ) -> heif_error;
    fn heif_image_handle_get_number_of_metadata_blocks(
        handle: *const heif_image_handle,
        type_filter: *const c_char,
    ) -> c_int;
    fn heif_image_handle_get_list_of_metadata_block_IDs(
        handle: *const heif_image_handle,
        type_filter: *const c_char,
        ids_out: *mut u32,
        count: c_int,
    ) -> c_int;
    fn heif_image_handle_get_metadata_size(
        handle: *const heif_image_handle,
        metadata_id: u32,
    ) -> usize;
    fn heif_image_handle_get_metadata(
        handle: *const heif_image_handle,
        metadata_id: u32,
        out_data: *mut c_void,
    ) -> heif_error;
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

    /// Read the raw embedded ICC profile bytes (rICC/prof box), if present.
    #[cfg(feature = "icc")]
    fn raw_color_profile(&self) -> Option<Vec<u8>> {
        let size = unsafe { heif_image_handle_get_raw_color_profile_size(self.ptr) };
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size];
        // SAFETY: libheif writes exactly `size` bytes (queried above) into the
        // provided buffer when a raw profile is present.
        let err = unsafe {
            heif_image_handle_get_raw_color_profile(self.ptr, buf.as_mut_ptr() as *mut c_void)
        };
        if err.code != 0 {
            return None;
        }
        Some(buf)
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
    /// BT.709 / sRGB primaries; matrix-converted to linear Rec.2020.
    Srgb,
    /// Display P3 primaries; matrix-converted to linear Rec.2020 (gamut preserved).
    DisplayP3,
    /// BT.2020 primaries; identity matrix (BT.2020 primaries == Rec.2020 primaries).
    ///
    /// Not currently returned by `probe_source_color_space` — BT.2020 sources
    /// fall back to `Srgb` with a warning until BT.2020 transfer-curve (and
    /// PQ/HLG HDR) handling lands. Kept wired so the dispatch is uniform
    /// for when the probe is eventually taught to distinguish SDR from PQ/HLG.
    #[allow(dead_code)]
    Bt2020,
}

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
        // Classify as sRGB so the matrix path is the fallback; the actual ICC
        // read (and conversion via lcms2) happens later in `decode_heic` via
        // `raw_color_profile()`, which takes precedence over this fallback when
        // the `icc` feature is enabled and a raw profile is present.
        return SourceColorSpace::Srgb;
    }

    if profile_type != HEIF_COLOR_PROFILE_TYPE_NCLX {
        // No profile declared — treat as sRGB silently (common for transcoded files).
        return SourceColorSpace::Srgb;
    }

    let mut nclx_ptr: *mut heif_color_profile_nclx = std::ptr::null_mut();
    let err = unsafe { heif_image_handle_get_nclx_color_profile(handle.ptr, &mut nclx_ptr) };
    if err.code != 0 {
        if !nclx_ptr.is_null() {
            unsafe { heif_nclx_color_profile_free(nclx_ptr) };
        }
        return SourceColorSpace::Srgb;
    }
    // SAFETY: libheif documents that on success (err.code == 0), it either
    // sets `nclx_ptr` to a non-null pointer to a valid, properly initialized
    // `heif_color_profile_nclx` struct, or leaves it null. The is_null path
    // is the `None` arm below. We borrow the struct through `as_ref` to read
    // one Copy field, then drop the borrow before passing the raw pointer
    // back to libheif's `free`.
    let primaries = match unsafe { nclx_ptr.as_ref() } {
        Some(nclx) => nclx.color_primaries,
        None => return SourceColorSpace::Srgb,
    };

    // Release the libheif-allocated struct.
    unsafe { heif_nclx_color_profile_free(nclx_ptr) };

    match primaries {
        COLOR_PRIMARIES_BT709 => SourceColorSpace::Srgb,
        COLOR_PRIMARIES_SMPTE_EG432_DISPLAY_P3 => SourceColorSpace::DisplayP3,
        COLOR_PRIMARIES_BT2020 => {
            eprintln!(
                "agx: HEIC source declares BT.2020 primaries; treating as sRGB \
                 because BT.2020 transfer curve (and PQ/HLG HDR variants) \
                 require dedicated transfer-curve handling. Tone fidelity may suffer."
            );
            SourceColorSpace::Srgb
        }
        _ => {
            eprintln!(
                "agx: HEIC source NCLX color_primaries={primaries} not recognized; \
                 treating as sRGB."
            );
            SourceColorSpace::Srgb
        }
    }
}

// --- Public API ---

/// Decode a HEIC/HEIF file into linear Rec.2020 f32.
///
/// libheif handles the container, codec backend, and orientation
/// transformations. This function inspects the source bit depth,
/// requests RGB-interleaved decode in the appropriate chroma layout,
/// and converts the pixel data to the engine's linear Rec.2020 f32 contract.
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
/// Source primaries are matrix-converted directly into linear Rec.2020,
/// preserving the wider gamut of Display P3 inputs end-to-end:
///
/// - BT.709 / sRGB primaries → `LINEAR_SRGB_TO_LINEAR_REC2020`.
/// - Display P3 primaries → `LINEAR_P3_TO_LINEAR_REC2020` (vivid reds
///   and saturated greens survive into the engine instead of being squashed).
/// - BT.2020 primaries → `LINEAR_BT2020_TO_LINEAR_REC2020` (identity). The
///   probe currently still falls back to "treat as sRGB" with a stderr
///   warning for BT.2020 sources because the BT.2020 transfer curve (and
///   PQ/HLG HDR variants) require dedicated transfer-curve handling; the
///   identity matrix is wired so that when the probe is eventually taught
///   to distinguish SDR from PQ/HLG, the matrix side is already correct.
///
/// ICC profile and unrecognized NCLX primaries fall back to "treat as
/// sRGB" with a stderr warning.
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
        -1 => {
            return Err(AgxError::Decode(
                "libheif: could not determine bit depth of source image".into(),
            ));
        }
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

    // Build a gamma-encoded, normalized [0,1] buffer (no linearization, no
    // matrix yet). Both the ICC path and the matrix path consume this.
    let mut buf: Rgb32FImage = if bits == 8 {
        Rgb32FImage::from_fn(width, height, |x, y| {
            let i = (y as usize) * stride + (x as usize) * bytes_per_pixel;
            image::Rgb([
                pixel_slice[i] as f32 / 255.0,
                pixel_slice[i + 1] as f32 / 255.0,
                pixel_slice[i + 2] as f32 / 255.0,
            ])
        })
    } else {
        // 9 or 10-bit values packed into 16-bit little-endian containers.
        let max_value = ((1u32 << bits) - 1) as f32;
        Rgb32FImage::from_fn(width, height, |x, y| {
            let i = (y as usize) * stride + (x as usize) * bytes_per_pixel;
            let r = u16::from_le_bytes([pixel_slice[i], pixel_slice[i + 1]]);
            let g = u16::from_le_bytes([pixel_slice[i + 2], pixel_slice[i + 3]]);
            let b = u16::from_le_bytes([pixel_slice[i + 4], pixel_slice[i + 5]]);
            image::Rgb([
                r as f32 / max_value,
                g as f32 / max_value,
                b as f32 / max_value,
            ])
        })
    };

    // ICC path: an embedded rICC/prof profile takes precedence over the sRGB
    // fallback. nclx-classified sources (DisplayP3/Bt2020) never reach the
    // `Srgb` arm. A genuine BT.709-nclx source also classifies as `Srgb` but
    // carries no raw profile, so `raw_color_profile()` returns `None` and it
    // falls through to the matrix path harmlessly.
    #[cfg(feature = "icc")]
    {
        if matches!(source_space, SourceColorSpace::Srgb) {
            if let Some(icc_bytes) = handle.raw_color_profile() {
                match crate::decode::icc::convert_to_working_space(&mut buf, &icc_bytes) {
                    Ok(()) => return Ok(buf),
                    Err(e) => {
                        eprintln!(
                            "agx: HEIC embedded ICC profile could not be applied ({e}); assuming sRGB"
                        );
                    }
                }
            }
        }
    }

    // Matrix path: linearize (sRGB curve) then apply source -> Rec.2020 matrix.
    let matrix = match source_space {
        SourceColorSpace::Srgb => &LINEAR_SRGB_TO_LINEAR_REC2020,
        SourceColorSpace::DisplayP3 => &LINEAR_P3_TO_LINEAR_REC2020,
        SourceColorSpace::Bt2020 => &LINEAR_BT2020_TO_LINEAR_REC2020,
    };
    for px in buf.pixels_mut() {
        let lin: LinSrgb<f32> = Srgb::new(px.0[0], px.0[1], px.0[2]).into_linear();
        px.0 = apply_matrix([lin.red, lin.green, lin.blue], matrix);
    }
    Ok(buf)
}

/// Extract EXIF metadata from a HEIC/HEIF file as a raw byte buffer.
///
/// Returns `None` on any error (missing file, malformed container, no EXIF
/// block present). The returned bytes are a standard TIFF header + IFDs
/// buffer — no `Exif\0\0` prefix, no app-marker — same shape as a TIFF
/// IFD on its own. Downstream encode paths that need `Exif\0\0` (notably
/// JPEG segment injection via `img-parts`) prepend it themselves; consumers
/// of `little_exif` may pass the buffer directly.
///
/// HEIF stores EXIF with a leading 4-byte TIFF-header-offset prefix per
/// ISO/IEC 23008-12. This function strips that prefix when present.
pub fn extract_heic_metadata(path: &Path) -> Option<Vec<u8>> {
    let ctx = HeifContext::new().ok()?;
    ctx.read_from_file(path).ok()?;
    let handle = ctx.primary_image_handle().ok()?;

    let exif_type = CString::new("Exif").ok()?;
    let count =
        unsafe { heif_image_handle_get_number_of_metadata_blocks(handle.ptr, exif_type.as_ptr()) };
    if count <= 0 {
        return None;
    }

    let mut ids: Vec<u32> = vec![0; count as usize];
    let got = unsafe {
        heif_image_handle_get_list_of_metadata_block_IDs(
            handle.ptr,
            exif_type.as_ptr(),
            ids.as_mut_ptr(),
            count,
        )
    };
    if got <= 0 {
        return None;
    }

    // Take the first EXIF block (HEIC files normally carry one).
    let id = ids[0];
    let size = unsafe { heif_image_handle_get_metadata_size(handle.ptr, id) };
    if size == 0 {
        return None;
    }

    let mut buf: Vec<u8> = vec![0; size];
    let err =
        unsafe { heif_image_handle_get_metadata(handle.ptr, id, buf.as_mut_ptr() as *mut c_void) };
    if err.code != 0 {
        return None;
    }

    // Strip the 4-byte TIFF-header-offset prefix when present. Per
    // ISO/IEC 23008-12, EXIF blocks in HEIF lead with a 4-byte big-endian
    // offset. A non-stripped buffer would confuse downstream EXIF parsers.
    if buf.len() > 4 {
        let offset = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        // The offset is to "II*\0" or "MM\0*" (TIFF header). If the bytes at
        // (4 + offset) look like a TIFF header, treat the leading 4 bytes as
        // the prefix and trim. Otherwise leave the buffer untouched.
        let probe_at = 4 + offset;
        if probe_at + 2 <= buf.len() {
            let probe = &buf[probe_at..probe_at + 2];
            if probe == b"II" || probe == b"MM" {
                buf.drain(..probe_at);
            }
        }
    }

    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
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

    #[test]
    fn extract_heic_metadata_nonexistent_returns_none() {
        let meta = extract_heic_metadata(Path::new("/nonexistent/photo.heic"));
        assert!(meta.is_none());
    }
}
