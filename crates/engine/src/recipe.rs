//! Legacy recipe render engine — facade (JS `tools/recipe/`).
//!
//! Filled by phase 1.13 (Agent B). `render_tile` is the only command
//! dispatch (`applyCommand`), `text_mask` the bitmap-glyph raster,
//! `schema` the profile/manifest detectors, `color_tokens` palette
//! resolution.
mod commands;
mod document;
mod text;
pub use commands::render as render_commands;
pub(crate) use commands::render_vector_layer;
pub use document::recipe_json_to_document;
#[cfg(test)]
mod tests;
