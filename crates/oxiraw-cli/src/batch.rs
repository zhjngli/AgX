use std::path::{Path, PathBuf};

/// Standard (non-raw) image file extensions recognized by the CLI.
const STANDARD_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tiff", "tif"];

/// Returns `true` if `path` has a standard image extension or a known raw extension.
fn is_image_file(path: &Path) -> bool {
    let has_standard_ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| STANDARD_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()));
    has_standard_ext || oxiraw::decode::is_raw_extension(path)
}

/// Scan `dir` for image files, optionally recursing into subdirectories.
/// Returns a sorted `Vec<PathBuf>` of discovered image files.
pub fn discover_images(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    collect_images(dir, recursive, &mut out);
    out.sort();
    out
}

/// Recursively (or not) collect image file paths from `dir` into `out`.
fn collect_images(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if recursive {
                collect_images(&path, recursive, out);
            }
        } else if path.is_file() && is_image_file(&path) {
            out.push(path);
        }
    }
}

/// Resolve the output path for a processed image.
///
/// Mirrors the subdirectory structure from `input_dir` into `output_dir`,
/// appends an optional suffix before the extension, and overrides the
/// extension when `format_ext` is provided.  Raw-format inputs default to
/// `.jpg` when no explicit format is given.
pub fn resolve_output_path(
    input: &Path,
    input_dir: &Path,
    output_dir: &Path,
    suffix: Option<&str>,
    format_ext: Option<&str>,
) -> PathBuf {
    // 1. Strip the input_dir prefix to get the relative path.
    let relative = input
        .strip_prefix(input_dir)
        .unwrap_or(input.file_name().map(Path::new).unwrap_or(input));

    // 2. Determine extension: explicit format > raw-default "jpg" > original.
    let ext = if let Some(fmt) = format_ext {
        fmt.to_string()
    } else if oxiraw::decode::is_raw_extension(input) {
        "jpg".to_string()
    } else {
        input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg")
            .to_string()
    };

    // 3. Get the file stem from the relative path's filename.
    let stem = relative
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    // 4. Build filename with optional suffix.
    let filename = match suffix {
        Some(s) => format!("{stem}{s}.{ext}"),
        None => format!("{stem}.{ext}"),
    };

    // 5. Join output_dir + parent of relative + filename.
    let parent = relative.parent().unwrap_or(Path::new(""));
    output_dir.join(parent).join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discover_finds_image_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("photo.jpg"), b"").unwrap();
        fs::write(tmp.path().join("photo.jpeg"), b"").unwrap();
        fs::write(tmp.path().join("photo.png"), b"").unwrap();
        fs::write(tmp.path().join("notes.txt"), b"").unwrap();

        let found = discover_images(tmp.path(), false);
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|p| p.extension().unwrap() != "txt"));
    }

    #[test]
    fn discover_skips_non_image_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("readme.md"), b"").unwrap();
        fs::write(tmp.path().join("data.txt"), b"").unwrap();
        fs::write(tmp.path().join(".hidden"), b"").unwrap();

        let found = discover_images(tmp.path(), false);
        assert!(found.is_empty());
    }

    #[test]
    fn discover_recursive_finds_subdirs() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.jpg"), b"").unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("b.png"), b"").unwrap();

        let flat = discover_images(tmp.path(), false);
        assert_eq!(flat.len(), 1);

        let deep = discover_images(tmp.path(), true);
        assert_eq!(deep.len(), 2);
    }

    #[test]
    fn discover_case_insensitive_extensions() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.JPG"), b"").unwrap();
        fs::write(tmp.path().join("b.Png"), b"").unwrap();
        fs::write(tmp.path().join("c.TIFF"), b"").unwrap();

        let found = discover_images(tmp.path(), false);
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn discover_sorted_by_name() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("charlie.jpg"), b"").unwrap();
        fs::write(tmp.path().join("alpha.png"), b"").unwrap();
        fs::write(tmp.path().join("bravo.tiff"), b"").unwrap();

        let found = discover_images(tmp.path(), false);
        let names: Vec<&str> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["alpha.png", "bravo.tiff", "charlie.jpg"]);
    }

    #[test]
    fn resolve_output_preserves_filename() {
        let result = resolve_output_path(
            Path::new("/photos/IMG_001.jpg"),
            Path::new("/photos"),
            Path::new("/edited"),
            None,
            None,
        );
        assert_eq!(result, PathBuf::from("/edited/IMG_001.jpg"));
    }

    #[test]
    fn resolve_output_preserves_subdirectory() {
        let result = resolve_output_path(
            Path::new("/photos/day1/IMG_001.jpg"),
            Path::new("/photos"),
            Path::new("/edited"),
            None,
            None,
        );
        assert_eq!(result, PathBuf::from("/edited/day1/IMG_001.jpg"));
    }

    #[test]
    fn resolve_output_applies_suffix() {
        let result = resolve_output_path(
            Path::new("/photos/IMG_001.jpg"),
            Path::new("/photos"),
            Path::new("/edited"),
            Some("_processed"),
            None,
        );
        assert_eq!(result, PathBuf::from("/edited/IMG_001_processed.jpg"));
    }

    #[test]
    fn resolve_output_overrides_format() {
        let result = resolve_output_path(
            Path::new("/photos/IMG_001.png"),
            Path::new("/photos"),
            Path::new("/edited"),
            None,
            Some("jpeg"),
        );
        assert_eq!(result, PathBuf::from("/edited/IMG_001.jpeg"));
    }

    #[test]
    fn resolve_output_raw_defaults_to_jpg() {
        let result = resolve_output_path(
            Path::new("/photos/IMG_001.cr2"),
            Path::new("/photos"),
            Path::new("/edited"),
            None,
            None,
        );
        assert_eq!(result, PathBuf::from("/edited/IMG_001.jpg"));
    }

    #[test]
    fn resolve_output_suffix_plus_format() {
        let result = resolve_output_path(
            Path::new("/photos/IMG_001.cr2"),
            Path::new("/photos"),
            Path::new("/edited"),
            Some("_edited"),
            Some("tiff"),
        );
        assert_eq!(result, PathBuf::from("/edited/IMG_001_edited.tiff"));
    }
}
