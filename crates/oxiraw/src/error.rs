use thiserror::Error;

#[derive(Debug, Error)]
pub enum OxirawError {
    #[error("Decode error: {0}")]
    Decode(String),
    #[error("Encode error: {0}")]
    Encode(String),
    #[error("Preset error: {0}")]
    Preset(String),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, OxirawError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_decode() {
        let err = OxirawError::Decode("bad file".into());
        assert_eq!(err.to_string(), "Decode error: bad file");
    }

    #[test]
    fn error_display_encode() {
        let err = OxirawError::Encode("write failed".into());
        assert_eq!(err.to_string(), "Encode error: write failed");
    }

    #[test]
    fn error_display_preset() {
        let err = OxirawError::Preset("parse failed".into());
        assert_eq!(err.to_string(), "Preset error: parse failed");
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: OxirawError = io_err.into();
        assert!(matches!(err, OxirawError::Io(_)));
    }
}
