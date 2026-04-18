#define_import_path common::math
// Shared math utilities for AgX compute shaders.

// Hermite smoothstep: 0 at edge0, 1 at edge1, smooth cubic transition.
fn smoothstep_agx(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

// Rec. 709 luminance from sRGB gamma-encoded values.
fn luminance(r: f32, g: f32, b: f32) -> f32 {
    return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

// Linear interpolation.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    return a + (b - a) * t;
}

// Linear interpolation for vec3f.
fn lerp3(a: vec3f, b: vec3f, t: f32) -> vec3f {
    return a + (b - a) * t;
}
