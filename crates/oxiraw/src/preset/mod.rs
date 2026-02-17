use serde::{Deserialize, Serialize};

use crate::engine::Parameters;
use crate::error::{OxirawError, Result};

/// Preset metadata (name, version, author).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PresetMetadata {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: String,
}

/// Tone adjustment section of a preset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct ToneSection {
    #[serde(default)]
    exposure: f32,
    #[serde(default)]
    contrast: f32,
    #[serde(default)]
    highlights: f32,
    #[serde(default)]
    shadows: f32,
    #[serde(default)]
    whites: f32,
    #[serde(default)]
    blacks: f32,
}

/// White balance section of a preset.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct WhiteBalanceSection {
    #[serde(default)]
    temperature: f32,
    #[serde(default)]
    tint: f32,
}

/// Internal TOML layout for a preset file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PresetRaw {
    #[serde(default)]
    metadata: PresetMetadata,
    #[serde(default)]
    tone: ToneSection,
    #[serde(default)]
    white_balance: WhiteBalanceSection,
}

/// A photo editing preset.
///
/// Presets are declarative parameter sets stored as TOML files.
/// Missing values default to neutral (no change).
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub metadata: PresetMetadata,
    pub params: Parameters,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            metadata: PresetMetadata::default(),
            params: Parameters::default(),
        }
    }
}

impl Preset {
    /// Parse a preset from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        let raw: PresetRaw =
            toml::from_str(toml_str).map_err(|e| OxirawError::Preset(e.to_string()))?;
        Ok(Self {
            metadata: raw.metadata,
            params: Parameters {
                exposure: raw.tone.exposure,
                contrast: raw.tone.contrast,
                highlights: raw.tone.highlights,
                shadows: raw.tone.shadows,
                whites: raw.tone.whites,
                blacks: raw.tone.blacks,
                temperature: raw.white_balance.temperature,
                tint: raw.white_balance.tint,
            },
        })
    }

    /// Serialize the preset to a TOML string.
    pub fn to_toml(&self) -> Result<String> {
        let raw = PresetRaw {
            metadata: self.metadata.clone(),
            tone: ToneSection {
                exposure: self.params.exposure,
                contrast: self.params.contrast,
                highlights: self.params.highlights,
                shadows: self.params.shadows,
                whites: self.params.whites,
                blacks: self.params.blacks,
            },
            white_balance: WhiteBalanceSection {
                temperature: self.params.temperature,
                tint: self.params.tint,
            },
        };
        toml::to_string_pretty(&raw).map_err(|e| OxirawError::Preset(e.to_string()))
    }

    /// Load a preset from a TOML file.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Self::from_toml(&content)
    }

    /// Save the preset to a TOML file.
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<()> {
        let toml_str = self.to_toml()?;
        std::fs::write(path, toml_str)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preset_is_neutral() {
        let preset = Preset::default();
        assert_eq!(preset.params, Parameters::default());
        assert_eq!(preset.metadata.name, "");
    }

    #[test]
    fn serialize_contains_expected_keys() {
        let mut preset = Preset::default();
        preset.metadata.name = "Test".into();
        preset.params.exposure = 1.5;
        preset.params.temperature = 30.0;

        let toml_str = preset.to_toml().unwrap();
        assert!(toml_str.contains("name = \"Test\""));
        assert!(toml_str.contains("exposure = 1.5"));
        assert!(toml_str.contains("temperature = 30.0"));
    }

    #[test]
    fn deserialize_parses_values() {
        let toml_str = r#"
[metadata]
name = "Golden Hour"
version = "1.0"
author = "test"

[tone]
exposure = 0.5
contrast = 15.0
highlights = -30.0
shadows = 25.0

[white_balance]
temperature = 6200.0
tint = 10.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert_eq!(preset.metadata.name, "Golden Hour");
        assert_eq!(preset.params.exposure, 0.5);
        assert_eq!(preset.params.contrast, 15.0);
        assert_eq!(preset.params.highlights, -30.0);
        assert_eq!(preset.params.shadows, 25.0);
        assert_eq!(preset.params.temperature, 6200.0);
        assert_eq!(preset.params.tint, 10.0);
    }

    #[test]
    fn roundtrip_serialize_deserialize() {
        let mut preset = Preset::default();
        preset.metadata.name = "Roundtrip".into();
        preset.params.exposure = 2.0;
        preset.params.contrast = -10.0;
        preset.params.temperature = 50.0;
        preset.params.tint = -5.0;

        let toml_str = preset.to_toml().unwrap();
        let parsed = Preset::from_toml(&toml_str).unwrap();
        assert_eq!(preset, parsed);
    }

    #[test]
    fn missing_fields_default_to_zero() {
        let toml_str = r#"
[metadata]
name = "Minimal"

[tone]
exposure = 1.0
"#;
        let preset = Preset::from_toml(toml_str).unwrap();
        assert_eq!(preset.params.exposure, 1.0);
        assert_eq!(preset.params.contrast, 0.0);
        assert_eq!(preset.params.highlights, 0.0);
        assert_eq!(preset.params.temperature, 0.0);
    }

    #[test]
    fn invalid_toml_returns_error() {
        let result = Preset::from_toml("this is not valid toml {{{{");
        assert!(result.is_err());
    }

    #[test]
    fn file_save_and_load_roundtrip() {
        let temp_path = std::env::temp_dir().join("oxiraw_test_preset.toml");

        let mut preset = Preset::default();
        preset.metadata.name = "File Test".into();
        preset.params.exposure = 1.5;
        preset.params.contrast = 20.0;

        preset.save_to_file(&temp_path).unwrap();
        let loaded = Preset::load_from_file(&temp_path).unwrap();
        assert_eq!(preset, loaded);

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn load_nonexistent_file_returns_error() {
        let result = Preset::load_from_file(std::path::Path::new("/nonexistent/preset.toml"));
        assert!(result.is_err());
    }
}
