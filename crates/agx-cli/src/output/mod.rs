//! Output formatters for `agx validate` (and future commands that need
//! both human and JSON output).

mod human;
mod json;

pub use human::format_human;
pub use json::format_json;
