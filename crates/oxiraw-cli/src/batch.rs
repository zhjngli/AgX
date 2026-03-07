use std::path::{Path, PathBuf};

/// Standard image extensions (always recognized).
const STANDARD_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "tiff", "tif"];

/// Check if a file has a recognized image extension.
fn is_image_file(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_ascii_lowercase(),
        None => return false,
    };
    STANDARD_EXTENSIONS.contains(&ext.as_str()) || oxiraw::decode::is_raw_extension(path)
}

/// Discover image files in a directory, optionally recursing into subdirectories.
/// Returns paths sorted alphabetically for deterministic processing order.
pub fn discover_images(dir: &Path, recursive: bool) -> Vec<PathBuf> {
    let mut images = Vec::new();
    collect_images(dir, recursive, &mut images);
    images.sort();
    images
}

fn collect_images(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && recursive {
            collect_images(&path, true, out);
        } else if path.is_file() && is_image_file(&path) {
            out.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_files(dir: &std::path::Path, names: &[&str]) {
        for name in names {
            fs::write(dir.join(name), b"fake").unwrap();
        }
    }

    #[test]
    fn discover_finds_jpeg_files() {
        let dir = TempDir::new().unwrap();
        create_test_files(dir.path(), &["a.jpg", "b.jpeg", "c.txt", "d.png"]);
        let found = discover_images(dir.path(), false);
        assert_eq!(found.len(), 3); // jpg, jpeg, png
    }

    #[test]
    fn discover_skips_non_image_files() {
        let dir = TempDir::new().unwrap();
        create_test_files(dir.path(), &["readme.md", "notes.txt", ".hidden"]);
        let found = discover_images(dir.path(), false);
        assert!(found.is_empty());
    }

    #[test]
    fn discover_recursive_finds_subdirs() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        create_test_files(dir.path(), &["a.jpg"]);
        create_test_files(&sub, &["b.png"]);

        let flat = discover_images(dir.path(), false);
        assert_eq!(flat.len(), 1); // only top-level

        let recursive = discover_images(dir.path(), true);
        assert_eq!(recursive.len(), 2); // top + sub
    }

    #[test]
    fn discover_case_insensitive_extensions() {
        let dir = TempDir::new().unwrap();
        create_test_files(dir.path(), &["a.JPG", "b.Png", "c.TIFF"]);
        let found = discover_images(dir.path(), false);
        assert_eq!(found.len(), 3);
    }

    #[test]
    fn discover_sorted_by_name() {
        let dir = TempDir::new().unwrap();
        create_test_files(dir.path(), &["c.jpg", "a.jpg", "b.jpg"]);
        let found = discover_images(dir.path(), false);
        let names: Vec<_> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect();
        assert_eq!(names, vec!["a.jpg", "b.jpg", "c.jpg"]);
    }
}
