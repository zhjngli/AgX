use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgxError {
    #[error("Decode error: {0}")]
    Decode(String),
    #[error("Encode error: {0}")]
    Encode(String),
    #[error("Preset error: {0}")]
    Preset(String),
    #[error("LUT error: {0}")]
    Lut(String),
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, AgxError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_decode() {
        let err = AgxError::Decode("bad file".into());
        assert_eq!(err.to_string(), "Decode error: bad file");
    }

    #[test]
    fn error_display_encode() {
        let err = AgxError::Encode("write failed".into());
        assert_eq!(err.to_string(), "Encode error: write failed");
    }

    #[test]
    fn error_display_preset() {
        let err = AgxError::Preset("parse failed".into());
        assert_eq!(err.to_string(), "Preset error: parse failed");
    }

    #[test]
    fn error_display_lut() {
        let err = AgxError::Lut("invalid size".into());
        assert_eq!(err.to_string(), "LUT error: invalid size");
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: AgxError = io_err.into();
        assert!(matches!(err, AgxError::Io(_)));
    }
}
