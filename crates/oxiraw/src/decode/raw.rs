//! Raw format decoding via LibRaw FFI.
//!
//! This module provides thin FFI bindings to LibRaw's C API and a safe
//! `decode_raw()` function that converts raw photo files to linear sRGB f32.

use std::ffi::{c_char, c_int, c_uint, CString};
use std::path::Path;

use image::{Rgb, Rgb32FImage};
use palette::{LinSrgb, Srgb};

use crate::error::{OxirawError, Result};

// --- FFI declarations ---

/// Opaque LibRaw processor handle.
#[repr(C)]
struct libraw_data_t {
    _opaque: [u8; 0],
}

/// Processed image output from LibRaw.
#[repr(C)]
struct libraw_processed_image_t {
    _type: c_uint,
    height: u16,
    width: u16,
    colors: u16,
    bits: u16,
    data_size: c_uint,
    data: [u8; 1],
}

extern "C" {
    fn libraw_init(flags: c_uint) -> *mut libraw_data_t;
    fn libraw_open_file(data: *mut libraw_data_t, fname: *const c_char) -> c_int;
    fn libraw_unpack(data: *mut libraw_data_t) -> c_int;
    fn libraw_dcraw_process(data: *mut libraw_data_t) -> c_int;
    fn libraw_dcraw_make_mem_image(
        data: *mut libraw_data_t,
        errc: *mut c_int,
    ) -> *mut libraw_processed_image_t;
    fn libraw_dcraw_clear_mem(img: *mut libraw_processed_image_t);
    fn libraw_recycle(data: *mut libraw_data_t);
    fn libraw_close(data: *mut libraw_data_t);
    fn libraw_strerror(err: c_int) -> *const c_char;

    fn oxiraw_get_make(data: *mut libraw_data_t, buf: *mut c_char, buf_size: c_int);
    fn oxiraw_get_model(data: *mut libraw_data_t, buf: *mut c_char, buf_size: c_int);
    fn oxiraw_get_iso(data: *mut libraw_data_t) -> f32;
    fn oxiraw_get_shutter(data: *mut libraw_data_t) -> f32;
    fn oxiraw_get_aperture(data: *mut libraw_data_t) -> f32;
    fn oxiraw_get_focal_len(data: *mut libraw_data_t) -> f32;
    fn oxiraw_get_timestamp(data: *mut libraw_data_t) -> i64;
    fn oxiraw_get_lens(data: *mut libraw_data_t, buf: *mut c_char, buf_size: c_int);
    fn oxiraw_get_lens_make(data: *mut libraw_data_t, buf: *mut c_char, buf_size: c_int);
}

// For this first version, we accept LibRaw's defaults:
// - 8-bit output
// - sRGB color space
// - Camera white balance if available, auto otherwise
// - Auto brightness

// --- Error helper ---

fn libraw_error_msg(err: c_int) -> String {
    unsafe {
        let ptr = libraw_strerror(err);
        if ptr.is_null() {
            return format!("LibRaw error code {err}");
        }
        std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

fn check_libraw(err: c_int) -> Result<()> {
    if err == 0 {
        Ok(())
    } else {
        Err(OxirawError::Decode(format!(
            "LibRaw: {}",
            libraw_error_msg(err)
        )))
    }
}

// --- RAII wrapper ---

/// RAII wrapper for a LibRaw processor. Calls `libraw_close` on drop.
struct LibRawProcessor {
    ptr: *mut libraw_data_t,
}

impl LibRawProcessor {
    fn new() -> Result<Self> {
        let ptr = unsafe { libraw_init(0) };
        if ptr.is_null() {
            return Err(OxirawError::Decode("LibRaw: failed to initialize".into()));
        }
        Ok(Self { ptr })
    }

    fn open_file(&self, path: &Path) -> Result<()> {
        let c_path = CString::new(
            path.to_str()
                .ok_or_else(|| OxirawError::Decode("invalid file path encoding".into()))?,
        )
        .map_err(|_| OxirawError::Decode("file path contains null byte".into()))?;
        check_libraw(unsafe { libraw_open_file(self.ptr, c_path.as_ptr()) })
    }

    fn unpack(&self) -> Result<()> {
        check_libraw(unsafe { libraw_unpack(self.ptr) })
    }

    fn process(&self) -> Result<()> {
        check_libraw(unsafe { libraw_dcraw_process(self.ptr) })
    }

    fn make_mem_image(&self) -> Result<ProcessedImage> {
        let mut errc: c_int = 0;
        let ptr = unsafe { libraw_dcraw_make_mem_image(self.ptr, &mut errc) };
        if ptr.is_null() {
            return Err(OxirawError::Decode(format!(
                "LibRaw: failed to create memory image: {}",
                libraw_error_msg(errc)
            )));
        }
        Ok(ProcessedImage { ptr })
    }

    fn get_make(&self) -> String {
        let mut buf = [0u8; 128];
        unsafe { oxiraw_get_make(self.ptr, buf.as_mut_ptr() as *mut c_char, 128) }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) };
        cstr.to_string_lossy().into_owned()
    }

    fn get_model(&self) -> String {
        let mut buf = [0u8; 128];
        unsafe { oxiraw_get_model(self.ptr, buf.as_mut_ptr() as *mut c_char, 128) }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) };
        cstr.to_string_lossy().into_owned()
    }

    fn get_iso(&self) -> f32 {
        unsafe { oxiraw_get_iso(self.ptr) }
    }
    fn get_shutter(&self) -> f32 {
        unsafe { oxiraw_get_shutter(self.ptr) }
    }
    fn get_aperture(&self) -> f32 {
        unsafe { oxiraw_get_aperture(self.ptr) }
    }
    fn get_focal_len(&self) -> f32 {
        unsafe { oxiraw_get_focal_len(self.ptr) }
    }
    fn get_timestamp(&self) -> i64 {
        unsafe { oxiraw_get_timestamp(self.ptr) }
    }

    fn get_lens(&self) -> String {
        let mut buf = [0u8; 256];
        unsafe { oxiraw_get_lens(self.ptr, buf.as_mut_ptr() as *mut c_char, 256) }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) };
        cstr.to_string_lossy().into_owned()
    }

    fn get_lens_make(&self) -> String {
        let mut buf = [0u8; 256];
        unsafe { oxiraw_get_lens_make(self.ptr, buf.as_mut_ptr() as *mut c_char, 256) }
        let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) };
        cstr.to_string_lossy().into_owned()
    }
}

impl Drop for LibRawProcessor {
    fn drop(&mut self) {
        unsafe {
            libraw_recycle(self.ptr);
            libraw_close(self.ptr);
        }
    }
}

/// RAII wrapper for a LibRaw processed image. Calls `libraw_dcraw_clear_mem` on drop.
struct ProcessedImage {
    ptr: *mut libraw_processed_image_t,
}

impl ProcessedImage {
    fn width(&self) -> u32 {
        unsafe { (*self.ptr).width as u32 }
    }

    fn height(&self) -> u32 {
        unsafe { (*self.ptr).height as u32 }
    }

    fn colors(&self) -> u16 {
        unsafe { (*self.ptr).colors }
    }

    fn bits(&self) -> u16 {
        unsafe { (*self.ptr).bits }
    }

    fn data_size(&self) -> usize {
        unsafe { (*self.ptr).data_size as usize }
    }

    fn data(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts((*self.ptr).data.as_ptr(), self.data_size()) }
    }
}

impl Drop for ProcessedImage {
    fn drop(&mut self) {
        unsafe {
            libraw_dcraw_clear_mem(self.ptr);
        }
    }
}

// --- Public API ---

/// Decode a raw photo file into linear sRGB f32 using LibRaw.
///
/// LibRaw handles the full processing pipeline: file parsing, unpacking,
/// demosaicing, color conversion, and white balance. The output is sRGB
/// which we convert to linear sRGB f32 for the oxiraw engine.
///
/// # Supported formats
///
/// CR2, CR3, NEF, NRW, ARW, SRF, SR2, RAF, DNG, RW2, ORF, PEF, SRW,
/// and many more — anything LibRaw supports (~1000 camera models).
pub fn decode_raw(path: &Path) -> Result<Rgb32FImage> {
    let processor = LibRawProcessor::new()?;
    processor.open_file(path)?;
    processor.unpack()?;
    processor.process()?;

    let img = processor.make_mem_image()?;

    let width = img.width();
    let height = img.height();
    let colors = img.colors();
    let bits = img.bits();

    if colors != 3 {
        return Err(OxirawError::Decode(format!(
            "LibRaw: expected 3 color channels, got {colors}"
        )));
    }

    let data = img.data();

    let linear = match bits {
        8 => Rgb32FImage::from_fn(width, height, |x, y| {
            let idx = ((y * width + x) * 3) as usize;
            let sr = data[idx] as f32 / 255.0;
            let sg = data[idx + 1] as f32 / 255.0;
            let sb = data[idx + 2] as f32 / 255.0;
            let lin: LinSrgb<f32> = Srgb::new(sr, sg, sb).into_linear();
            Rgb([lin.red, lin.green, lin.blue])
        }),
        16 => Rgb32FImage::from_fn(width, height, |x, y| {
            let idx = ((y * width + x) * 3) as usize * 2;
            let sr = u16::from_ne_bytes([data[idx], data[idx + 1]]) as f32 / 65535.0;
            let sg = u16::from_ne_bytes([data[idx + 2], data[idx + 3]]) as f32 / 65535.0;
            let sb = u16::from_ne_bytes([data[idx + 4], data[idx + 5]]) as f32 / 65535.0;
            let lin: LinSrgb<f32> = Srgb::new(sr, sg, sb).into_linear();
            Rgb([lin.red, lin.green, lin.blue])
        }),
        _ => {
            return Err(OxirawError::Decode(format!(
                "LibRaw: unsupported bit depth {bits}"
            )));
        }
    };

    Ok(linear)
}

struct RawMetadataFields {
    make: String,
    model: String,
    iso: f32,
    shutter: f32,
    aperture: f32,
    focal_len: f32,
    timestamp: i64,
    lens: String,
    lens_make: String,
}

/// Extract metadata from a raw file using LibRaw's parsed fields.
///
/// Returns raw EXIF bytes (not wrapped in `ImageMetadata`).
pub fn extract_raw_metadata(path: &Path) -> Option<Vec<u8>> {
    let processor = LibRawProcessor::new().ok()?;
    processor.open_file(path).ok()?;

    let fields = RawMetadataFields {
        make: processor.get_make(),
        model: processor.get_model(),
        iso: processor.get_iso(),
        shutter: processor.get_shutter(),
        aperture: processor.get_aperture(),
        focal_len: processor.get_focal_len(),
        timestamp: processor.get_timestamp(),
        lens: processor.get_lens(),
        lens_make: processor.get_lens_make(),
    };

    construct_exif_from_fields(&fields)
}

fn construct_exif_from_fields(fields: &RawMetadataFields) -> Option<Vec<u8>> {
    use little_exif::exif_tag::ExifTag;
    use little_exif::metadata::Metadata;
    use little_exif::rational::uR64;

    let mut metadata = Metadata::new();

    if !fields.make.is_empty() {
        metadata.set_tag(ExifTag::Make(fields.make.clone()));
    }
    if !fields.model.is_empty() {
        metadata.set_tag(ExifTag::Model(fields.model.clone()));
    }
    if fields.iso > 0.0 {
        metadata.set_tag(ExifTag::ISO(vec![fields.iso as u16]));
    }
    if fields.shutter > 0.0 {
        let rational = if fields.shutter >= 1.0 {
            uR64 {
                nominator: fields.shutter as u32,
                denominator: 1u32,
            }
        } else {
            uR64 {
                nominator: 1u32,
                denominator: (1.0 / fields.shutter).round() as u32,
            }
        };
        metadata.set_tag(ExifTag::ExposureTime(vec![rational]));
    }
    if fields.aperture > 0.0 {
        let num = (fields.aperture * 10.0).round() as u32;
        metadata.set_tag(ExifTag::FNumber(vec![uR64 {
            nominator: num,
            denominator: 10,
        }]));
    }
    if fields.focal_len > 0.0 {
        let num = (fields.focal_len * 10.0).round() as u32;
        metadata.set_tag(ExifTag::FocalLength(vec![uR64 {
            nominator: num,
            denominator: 10,
        }]));
    }
    if fields.timestamp > 0 {
        if let Some(dt_str) = timestamp_to_exif_datetime(fields.timestamp) {
            metadata.set_tag(ExifTag::DateTimeOriginal(dt_str));
        }
    }
    if !fields.lens.is_empty() {
        metadata.set_tag(ExifTag::LensModel(fields.lens.clone()));
    }
    if !fields.lens_make.is_empty() {
        metadata.set_tag(ExifTag::LensMake(fields.lens_make.clone()));
    }

    let exif_bytes = metadata
        .as_u8_vec(little_exif::filetype::FileExtension::JPEG)
        .ok()?;
    if exif_bytes.is_empty() {
        return None;
    }

    Some(exif_bytes)
}

fn timestamp_to_exif_datetime(timestamp: i64) -> Option<String> {
    if timestamp <= 0 {
        return None;
    }
    let secs_per_day: i64 = 86400;
    let mut days = timestamp / secs_per_day;
    let day_secs = (timestamp % secs_per_day) as u32;
    let hours = day_secs / 3600;
    let minutes = (day_secs % 3600) / 60;
    let seconds = day_secs % 60;
    let mut year = 1970i32;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap_year(year);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    let day = days as u32 + 1;
    Some(format!(
        "{year:04}:{month:02}:{day:02} {hours:02}:{minutes:02}:{seconds:02}"
    ))
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libraw_processor_init_and_drop() {
        let processor = LibRawProcessor::new().unwrap();
        drop(processor);
    }

    #[test]
    fn decode_raw_nonexistent_file_returns_error() {
        let result = decode_raw(Path::new("/nonexistent/photo.cr2"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("LibRaw"),
            "Error should mention LibRaw: {err_msg}"
        );
    }

    #[test]
    fn extract_raw_metadata_nonexistent_returns_none() {
        let meta = extract_raw_metadata(Path::new("/nonexistent/photo.raf"));
        assert!(meta.is_none());
    }
}
