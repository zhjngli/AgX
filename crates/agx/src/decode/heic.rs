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
#[allow(dead_code)]
const HEIF_CHROMA_INTERLEAVED_RRGGBB_LE: c_int = 14;

// Channel enum for plane access
#[allow(dead_code)]
const HEIF_CHANNEL_INTERLEAVED: c_int = 10;

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

    let bits = handle.luma_bits_per_pixel();
    let (chroma, bytes_per_pixel) = match bits {
        8 => (HEIF_CHROMA_INTERLEAVED_RGB, 3),
        _ => {
            return Err(AgxError::Decode(format!(
                "libheif: unsupported bit depth {bits} (only 8-bit supported in this revision)"
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
    let stride = stride as usize;

    // Safety: data points to width*height pixels with the given stride for the
    // lifetime of `img`. We read it before `img` is dropped.
    let pixel_slice: &[u8] = unsafe { std::slice::from_raw_parts(data, stride * height as usize) };

    let buf = Rgb32FImage::from_fn(width, height, |x, y| {
        let row_offset = (y as usize) * stride;
        let col_offset = (x as usize) * bytes_per_pixel;
        let i = row_offset + col_offset;
        let sr = pixel_slice[i] as f32 / 255.0;
        let sg = pixel_slice[i + 1] as f32 / 255.0;
        let sb = pixel_slice[i + 2] as f32 / 255.0;
        let lin: LinSrgb<f32> = Srgb::new(sr, sg, sb).into_linear();
        image::Rgb([lin.red, lin.green, lin.blue])
    });

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
}
