# HEIC/HEIF Format Support

## Summary

Add decoding support for HEIC/HEIF images (`.heic`, `.heif`), the format used by Apple devices (iPhone, iPad) since iOS 11.

## Motivation

HEIC is the default photo format on modern Apple devices. Users shooting on iPhones can't currently process their photos through AgX without first converting to JPEG, which loses quality.

## Considerations

- HEIC decoding requires a codec library (e.g., `libheif` via FFI, or a pure-Rust crate if one matures)
- Patent/licensing: HEVC codec has patent considerations — `libheif` handles this but worth understanding
- HEIF is a container format that can hold HEVC or AV1 (AVIF) encoded images
- Metadata: HEIC files carry EXIF metadata that the metadata module should extract
- Test fixtures: will need HEIC sample images added to `crates/agx-e2e/fixtures/` for e2e testing
