//! `pixeldraw-cli` — headless document + atlas tooling for skills and CI.
//!
//! Every command prints one JSON object on stdout (errors included), which is
//! what makes it safe to wrap in an AI skill: no human-oriented tables to
//! scrape, no locale-dependent text, and a non-zero exit code on failure.
//!
//! ```text
//! pixeldraw-cli info <doc.json>
//! pixeldraw-cli layers <doc.json>
//! pixeldraw-cli edit <input.draw.json> <patch.json> <output.draw.json>
//! pixeldraw-cli flatten <doc.json> <out.png>
//! pixeldraw-cli atlas spec
//! pixeldraw-cli atlas plan <doc.json> [options]
//! pixeldraw-cli atlas export <doc.json> <out.png> [options]
//! pixeldraw-cli atlas inspect <atlas.png>
//! ```
//!
//! Atlas options:
//! `--mode layers|composed`, `--columns N`, `--rows N`, `--cell N|WxH`,
//! `--original-size`, `--no-preset`, `--slot NAME=layer1,layer2` (repeatable,
//! application order bottom → top).
//!
//! Submodules: `atlas` owns the atlas subcommands, `edit` applies JSON patches,
//! and `doc` loads/summarises documents.

mod atlas;
mod doc;
mod edit;
#[cfg(test)]
mod tests;

use std::fs;

use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::io::export::{FlattenOptions, flatten_to_rgba};
use pixel_engine::io::png::encode_png;
use pixel_engine::io::serialize::document_to_json;

use atlas::atlas_command;
use doc::{doc_summary, io_error, layer_rows, load_doc, need, write_file};
use edit::apply_edit_patch;

fn main() {
    match run() {
        Ok(value) => print_json(&value),
        Err(e) => {
            print_json(&serde_json::json!({
                "ok": false,
                "error": { "code": e.code, "message": e.message }
            }));
            std::process::exit(1);
        }
    }
}

fn print_json(value: &serde_json::Value) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(e) => eprintln!("json encode failed: {e}"),
    }
}

fn run() -> EngineResult<serde_json::Value> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "-h" | "--help" | "help") {
        return Ok(serde_json::json!({ "ok": true, "usage": USAGE }));
    }
    match args[0].as_str() {
        "info" => {
            need(&args, 2, "info <doc.json>")?;
            let (doc, warnings) = load_doc(&args[1])?;
            Ok(serde_json::json!({
                "ok": true,
                "document": doc_summary(&doc),
                "layers": layer_rows(&doc),
                "warnings": warnings,
            }))
        }
        "layers" => {
            need(&args, 2, "layers <doc.json>")?;
            let (doc, _) = load_doc(&args[1])?;
            Ok(serde_json::json!({
                "ok": true,
                "document": doc_summary(&doc),
                "layers": layer_rows(&doc),
            }))
        }
        "edit" => {
            need(
                &args,
                4,
                "edit <input.draw.json> <patch.json> <output.draw.json>",
            )?;
            let (mut doc, _) = load_doc(&args[1])?;
            let patch_bytes = fs::read(&args[2]).map_err(|e| io_error(&args[2], e))?;
            let patch: serde_json::Value = serde_json::from_slice(&patch_bytes)
                .map_err(|e| DrawToolError::invalid_state(format!("{}: not JSON: {e}", args[2])))?;
            let summary = apply_edit_patch(&mut doc, &patch)?;
            let value = document_to_json(&doc)?;
            let report = pixel_engine::io::validate::validate_document(&value);
            if !report.ok {
                return Err(DrawToolError::invalid_state(format!(
                    "edited document failed validation: {}",
                    report
                        .errors
                        .first()
                        .map(|e| e.message.as_str())
                        .unwrap_or("unknown validation error")
                )));
            }
            let bytes = serde_json::to_vec_pretty(&value).map_err(|e| {
                DrawToolError::invalid_state(format!("document encode failed: {e}"))
            })?;
            write_file(&args[3], &bytes)?;
            Ok(serde_json::json!({
                "ok": true,
                "input": args[1],
                "output": args[3],
                "operations_applied": summary.operations_applied,
                "pixel_changes": summary.pixel_changes,
                "atlas": doc.atlas.as_ref(),
                "document": doc_summary(&doc),
                "layers": layer_rows(&doc),
            }))
        }
        "flatten" => {
            need(&args, 3, "flatten <doc.json> <out.png>")?;
            let (doc, _) = load_doc(&args[1])?;
            let (w, h) = (doc.canvas.width_px, doc.canvas.height_px);
            let rgba = flatten_to_rgba(
                &doc.layer_sources(),
                w,
                h,
                FlattenOptions {
                    include_background: true,
                },
            );
            let bytes = encode_png(&rgba, w, h)?;
            write_file(&args[2], &bytes)?;
            Ok(serde_json::json!({
                "ok": true,
                "output": args[2],
                "width": w,
                "height": h,
                "bytes": bytes.len(),
            }))
        }
        "atlas" => atlas_command(&args[1..]),
        other => Err(DrawToolError::invalid_state(format!(
            "unknown command {other:?}; see --help"
        ))),
    }
}

const USAGE: &str = "\
pixeldraw-cli — headless Pixeldraw document + atlas tool

USAGE:
  pixeldraw-cli info <doc.json>
  pixeldraw-cli layers <doc.json>
  pixeldraw-cli edit <input.draw.json> <patch.json> <output.draw.json>
  pixeldraw-cli flatten <doc.json> <out.png>
  pixeldraw-cli atlas spec
  pixeldraw-cli atlas plan <doc.json> [options]
  pixeldraw-cli atlas export <doc.json> <out.png> [options]
  pixeldraw-cli atlas inspect <atlas.png>
  pixeldraw-cli atlas render-cell <doc.json> <out.png> [--cell WxH] [--layers a,b]

ATLAS OPTIONS:
  --mode layers|composed   view/save mode (default: composed when --slot is used)
  --columns N             1..=64 columns; default is the near-square ladder
  --rows N                1..=64 fixed rows; default grows rows to fit
  --cell N|WxH            cell size (default: document clamped to 512x512)
  --margin N              transparent outer sheet margin in pixels (default 0)
  --padding N             transparent gap between cells in pixels (default 0)
  --original-size         use the document canvas as the cell size
  --no-preset             do not prepend the automatic all-layers slot
  --slot NAME=a,b,c        ordered slot (bottom → top), repeatable

EDIT PATCH:
  JSON object: {\"operations\":[...]}; see docs/cli_edit.md for operations.

All output is one JSON object. The binary is the CLI half of the
`pixeldraw-atlas` skill; see docs/skills/pixeldraw-atlas/SKILL.md.
";
