use super::Lut3D;
use crate::error::{AgxError, Result};

/// Parse a `.cube` format string into a `Lut3D`.
///
/// The `.cube` format is a plain text format defined by Adobe for DaVinci Resolve.
/// It is the de facto standard for LUT interchange across photo and video editing tools.
///
/// # Supported keywords
///
/// - `TITLE "name"` — optional title
/// - `LUT_3D_SIZE N` — required, defines an N×N×N cube
/// - `DOMAIN_MIN r g b` — input minimum per channel (default 0.0 0.0 0.0)
/// - `DOMAIN_MAX r g b` — input maximum per channel (default 1.0 1.0 1.0)
/// - Lines starting with `#` are comments
///
/// # Data layout
///
/// After the header, each line contains three space-separated floats (R G B output).
/// Entries are ordered with R changing fastest, then G, then B.
pub fn parse_cube(text: &str) -> Result<Lut3D> {
    let mut title: Option<String> = None;
    let mut size: Option<usize> = None;
    let mut domain_min = [0.0f32, 0.0, 0.0];
    let mut domain_max = [1.0f32, 1.0, 1.0];
    let mut table: Vec<[f32; 3]> = Vec::new();

    for (line_num, line) in text.lines().enumerate() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix("TITLE") {
            let rest = rest.trim();
            let unquoted = rest.trim_matches('"');
            title = Some(unquoted.to_string());
            continue;
        }

        if let Some(rest) = line.strip_prefix("LUT_3D_SIZE") {
            size = Some(rest.trim().parse::<usize>().map_err(|_| {
                AgxError::Lut(format!("line {}: invalid LUT_3D_SIZE", line_num + 1))
            })?);
            continue;
        }

        if let Some(rest) = line.strip_prefix("DOMAIN_MIN") {
            domain_min = parse_rgb_line(rest.trim(), line_num)?;
            continue;
        }

        if let Some(rest) = line.strip_prefix("DOMAIN_MAX") {
            domain_max = parse_rgb_line(rest.trim(), line_num)?;
            continue;
        }

        // Skip unsupported keywords (don't error on them)
        if line.starts_with("LUT_1D_SIZE")
            || line.starts_with("LUT_1D_INPUT_RANGE")
            || line.starts_with("LUT_3D_INPUT_RANGE")
            || line.starts_with("LUT_IN_VIDEO_RANGE")
            || line.starts_with("LUT_OUT_VIDEO_RANGE")
        {
            continue;
        }

        // Data line — three floats
        let rgb = parse_rgb_line(line, line_num)?;
        table.push(rgb);
    }

    let size = size.ok_or_else(|| AgxError::Lut("missing LUT_3D_SIZE".into()))?;
    let expected = size * size * size;

    if table.len() != expected {
        return Err(AgxError::Lut(format!(
            "expected {} entries for size {}, got {}",
            expected,
            size,
            table.len()
        )));
    }

    Ok(Lut3D {
        title,
        size,
        domain_min,
        domain_max,
        table,
    })
}

fn parse_rgb_line(line: &str, line_num: usize) -> Result<[f32; 3]> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() != 3 {
        return Err(AgxError::Lut(format!(
            "line {}: expected 3 values, got {}",
            line_num + 1,
            parts.len()
        )));
    }
    let r = parts[0].parse::<f32>().map_err(|_| {
        AgxError::Lut(format!(
            "line {}: invalid float '{}'",
            line_num + 1,
            parts[0]
        ))
    })?;
    let g = parts[1].parse::<f32>().map_err(|_| {
        AgxError::Lut(format!(
            "line {}: invalid float '{}'",
            line_num + 1,
            parts[1]
        ))
    })?;
    let b = parts[2].parse::<f32>().map_err(|_| {
        AgxError::Lut(format!(
            "line {}: invalid float '{}'",
            line_num + 1,
            parts[2]
        ))
    })?;
    Ok([r, g, b])
}
