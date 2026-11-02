//! `atlas` subcommands: spec/plan/export/inspect/render-cell.

use std::fs;

use pixel_engine::atlas::{
    ATLAS_DEFAULT_CELL_MAX, ATLAS_MAX_CELLS, ATLAS_MAX_LAYERS_PER_CELL, ATLAS_MAX_PX,
    ATLAS_META_VERSION, AtlasCellSpec, AtlasLayout, AtlasMetadata, AtlasMode, best_columns,
    default_cell_size, export_atlas_png, metadata_for, resolve_layer_ids,
};
use pixel_engine::errors::{DrawToolError, EngineResult};
use pixel_engine::io::png::{decode_png_large, encode_png};
use pixel_engine::model::Document;

use crate::doc::{doc_summary, io_error, load_doc, need, write_file};

pub(crate) fn atlas_command(args: &[String]) -> EngineResult<serde_json::Value> {
    let Some(sub) = args.first() else {
        return Err(DrawToolError::invalid_state("atlas needs a subcommand"));
    };
    match sub.as_str() {
        "spec" => Ok(serde_json::json!({
            "ok": true,
            "metadata_version": ATLAS_META_VERSION,
            "max_cells": ATLAS_MAX_CELLS,
            "max_layers_per_cell": ATLAS_MAX_LAYERS_PER_CELL,
            "default_cell_max": ATLAS_DEFAULT_CELL_MAX,
            "max_pixels": ATLAS_MAX_PX,
            "modes": AtlasMode::ALL.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
            "layout_ladder": "1x64 through 64x1; columns is 1..=64 and rows = ceil(cells/columns)",
            "save_modes": {
                "layers": "one cell per document layer, top-first",
                "composed": "one cell per ordered slot; optional preset slot 1 = all layers"
            }
        })),
        "plan" => {
            need(args, 2, "atlas plan <doc.json> [options]")?;
            let (doc, _) = load_doc(&args[1])?;
            let opts = AtlasOptions::parse(&args[2..])?;
            let (layout, meta) = plan(&doc, &opts)?;
            Ok(plan_json(&doc, &layout, &meta))
        }
        "export" => {
            need(args, 3, "atlas export <doc.json> <out.png> [options]")?;
            let (doc, _) = load_doc(&args[1])?;
            let opts = AtlasOptions::parse(&args[3..])?;
            let (layout, _) = plan(&doc, &opts)?;
            let export = export_atlas_png(&doc, &layout)?;
            write_file(&args[2], &export.bytes)?;
            let mut value = plan_json(&doc, &layout, &export.metadata);
            value["output"] = serde_json::json!(args[2]);
            value["bytes"] = serde_json::json!(export.bytes.len());
            Ok(value)
        }
        "inspect" => {
            need(args, 2, "atlas inspect <atlas.png>")?;
            let bytes = fs::read(&args[1]).map_err(|e| io_error(&args[1], e))?;
            let img = decode_png_large(&bytes)?;
            let raw = img
                .metadata_chunk("dtAt")
                .ok_or_else(|| DrawToolError::invalid_state("PNG has no dtAt atlas metadata"))?;
            let meta = AtlasMetadata::from_json_bytes(raw)?;
            Ok(serde_json::json!({
                "ok": true,
                "png": { "width": img.width, "height": img.height },
                "metadata": meta,
            }))
        }
        "render-cell" => {
            // Handy for skills that want one cell without the sheet: render a
            // layer list into a single PNG at the atlas default cell size.
            need(
                args,
                3,
                "atlas render-cell <doc.json> <out.png> [--cell WxH] [--layers a,b]",
            )?;
            let (doc, _) = load_doc(&args[1])?;
            let mut cell = default_cell_size(doc.canvas.width_px, doc.canvas.height_px);
            let mut selectors: Option<Vec<String>> = None;
            let mut i = 3;
            while i < args.len() {
                match args[i].as_str() {
                    "--cell" => {
                        let raw = args
                            .get(i + 1)
                            .ok_or_else(|| DrawToolError::invalid_state("--cell needs N or WxH"))?;
                        cell = parse_cell(raw)?;
                        i += 2;
                    }
                    "--layers" => {
                        let raw = args
                            .get(i + 1)
                            .ok_or_else(|| DrawToolError::invalid_state("--layers needs a,b,c"))?;
                        selectors = Some(
                            raw.split(',')
                                .map(str::trim)
                                .filter(|s| !s.is_empty())
                                .map(str::to_string)
                                .collect(),
                        );
                        i += 2;
                    }
                    other => {
                        return Err(DrawToolError::invalid_state(format!(
                            "unknown render-cell option {other:?}"
                        )));
                    }
                }
            }
            let ids = match selectors {
                Some(selectors) => resolve_layer_ids(&doc, &selectors)?,
                None => doc.layers.iter().map(|l| l.id.clone()).collect(),
            };
            let spec = AtlasCellSpec::composed("cell", ids);
            let rendered = pixel_engine::atlas::build_cell_rgba(&doc, &spec, cell.0, cell.1)?;
            let bytes = encode_png(&rendered.rgba, rendered.width, rendered.height)?;
            write_file(&args[2], &bytes)?;
            Ok(serde_json::json!({
                "ok": true,
                "output": args[2],
                "width": rendered.width,
                "height": rendered.height,
                "bytes": bytes.len(),
            }))
        }
        other => Err(DrawToolError::invalid_state(format!(
            "unknown atlas subcommand {other:?}"
        ))),
    }
}

pub(crate) fn plan(
    doc: &Document,
    opts: &AtlasOptions,
) -> EngineResult<(AtlasLayout, AtlasMetadata)> {
    let (cell_w, cell_h) = if opts.original_size {
        (doc.canvas.width_px, doc.canvas.height_px)
    } else {
        opts.cell
            .unwrap_or_else(|| default_cell_size(doc.canvas.width_px, doc.canvas.height_px))
    };
    let mode = opts.mode.unwrap_or(if opts.slots.is_empty() {
        AtlasMode::Layers
    } else {
        AtlasMode::Composed
    });
    let mut slots = Vec::with_capacity(opts.slots.len());
    for (i, (name, selectors)) in opts.slots.iter().enumerate() {
        let ids = resolve_layer_ids(doc, selectors)?;
        let name = if name.trim().is_empty() {
            format!("Slot {}", i + 1)
        } else {
            name.clone()
        };
        slots.push(AtlasCellSpec {
            name,
            merged: ids.len() > 1,
            layer_ids: ids,
        });
    }
    let mut layout = AtlasLayout {
        mode,
        columns: opts.columns.unwrap_or(1),
        rows: opts.rows,
        cell_w,
        cell_h,
        margin: opts.margin,
        padding: opts.padding,
        preset_all: opts.preset_all,
        slots,
    };
    let count = layout.cells(doc).len();
    if opts.columns.is_none() {
        layout.columns = opts.rows.map_or_else(
            || best_columns(count.max(1)),
            |rows| {
                (count.max(1) as u32)
                    .div_ceil(rows)
                    .clamp(1, ATLAS_MAX_CELLS as u32)
            },
        );
    }
    layout.validate(doc)?;
    let specs = layout.cells(doc);
    let (sw, sh) = layout.sheet_size(doc);
    let meta = metadata_for(doc, &layout, &specs, sw, sh);
    meta.validate()?;
    Ok((layout, meta))
}

fn plan_json(doc: &Document, layout: &AtlasLayout, meta: &AtlasMetadata) -> serde_json::Value {
    let cells = layout.cells(doc);
    serde_json::json!({
        "ok": true,
        "document": doc_summary(doc),
        "mode": layout.mode.as_str(),
        "columns": layout.columns,
        "rows": layout.rows(doc),
        "cell": { "width": meta.cell.w, "height": meta.cell.h },
        "margin": meta.margin,
        "padding": meta.padding,
        "sheet": { "width": meta.sheet.w, "height": meta.sheet.h },
        "preset_all": meta.preset_all,
        "cells": cells.iter().enumerate().map(|(i, cell)| serde_json::json!({
            "index": i,
            "name": cell.name,
            "merged": cell.merged,
            "layer_ids": cell.layer_ids,
            "sources": cell.layer_ids.iter().filter_map(|id| doc.find_layer(id).map(|l| l.name.clone())).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "metadata": meta,
    })
}

pub(crate) fn parse_cell(raw: &str) -> EngineResult<(u32, u32)> {
    let (w, h) = match raw.split_once(['x', 'X']) {
        Some((w, h)) => (w, h),
        None => (raw, raw),
    };
    let w: u32 = w
        .trim()
        .parse()
        .map_err(|_| DrawToolError::invalid_state(format!("bad cell size {raw:?}")))?;
    let h: u32 = h
        .trim()
        .parse()
        .map_err(|_| DrawToolError::invalid_state(format!("bad cell size {raw:?}")))?;
    if w == 0 || h == 0 {
        return Err(DrawToolError::out_of_range(
            "cell size must be at least 1x1",
        ));
    }
    Ok((w, h))
}

pub(crate) struct AtlasOptions {
    pub(crate) mode: Option<AtlasMode>,
    pub(crate) columns: Option<u32>,
    pub(crate) rows: Option<u32>,
    pub(crate) cell: Option<(u32, u32)>,
    pub(crate) margin: u32,
    pub(crate) padding: u32,
    pub(crate) original_size: bool,
    pub(crate) preset_all: bool,
    pub(crate) slots: Vec<(String, Vec<String>)>,
}

impl AtlasOptions {
    pub(crate) fn parse(args: &[String]) -> EngineResult<Self> {
        let mut out = Self {
            mode: None,
            columns: None,
            rows: None,
            cell: None,
            margin: 0,
            padding: 0,
            original_size: false,
            preset_all: true,
            slots: Vec::new(),
        };
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--mode" => {
                    let raw = args.get(i + 1).ok_or_else(|| {
                        DrawToolError::invalid_state("--mode needs layers|composed")
                    })?;
                    out.mode = Some(AtlasMode::parse(raw).ok_or_else(|| {
                        DrawToolError::invalid_state(format!("unknown atlas mode {raw:?}"))
                    })?);
                    i += 2;
                }
                "--columns" => {
                    let raw = args
                        .get(i + 1)
                        .ok_or_else(|| DrawToolError::invalid_state("--columns needs a number"))?;
                    out.columns = Some(raw.parse().map_err(|_| {
                        DrawToolError::invalid_state(format!("bad --columns value {raw:?}"))
                    })?);
                    i += 2;
                }
                "--rows" => {
                    let raw = args
                        .get(i + 1)
                        .ok_or_else(|| DrawToolError::invalid_state("--rows needs a number"))?;
                    let rows: u32 = raw.parse().map_err(|_| {
                        DrawToolError::invalid_state(format!("bad --rows value {raw:?}"))
                    })?;
                    if !(1..=ATLAS_MAX_CELLS as u32).contains(&rows) {
                        return Err(DrawToolError::out_of_range(format!(
                            "--rows must be 1..={ATLAS_MAX_CELLS}"
                        )));
                    }
                    out.rows = Some(rows);
                    i += 2;
                }
                "--cell" => {
                    let raw = args
                        .get(i + 1)
                        .ok_or_else(|| DrawToolError::invalid_state("--cell needs N or WxH"))?;
                    out.cell = Some(parse_cell(raw)?);
                    i += 2;
                }
                "--margin" | "--padding" => {
                    let option = args[i].as_str();
                    let raw = args.get(i + 1).ok_or_else(|| {
                        DrawToolError::invalid_state(format!("{option} needs a pixel count"))
                    })?;
                    let value: u32 = raw.parse().map_err(|_| {
                        DrawToolError::invalid_state(format!("bad {option} value {raw:?}"))
                    })?;
                    if value > 4096 {
                        return Err(DrawToolError::out_of_range(format!(
                            "{option} must be 0..=4096"
                        )));
                    }
                    if option == "--margin" {
                        out.margin = value;
                    } else {
                        out.padding = value;
                    }
                    i += 2;
                }
                "--original-size" => {
                    out.original_size = true;
                    i += 1;
                }
                "--no-preset" => {
                    out.preset_all = false;
                    i += 1;
                }
                "--preset-all" => {
                    out.preset_all = true;
                    i += 1;
                }
                "--slot" => {
                    let raw = args
                        .get(i + 1)
                        .ok_or_else(|| DrawToolError::invalid_state("--slot needs NAME=a,b"))?;
                    let (name, selectors) = raw.split_once('=').ok_or_else(|| {
                        DrawToolError::invalid_state(
                            "--slot must be NAME=layer1,layer2 (bottom → top)",
                        )
                    })?;
                    let selectors = selectors
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>();
                    if selectors.is_empty() {
                        return Err(DrawToolError::invalid_state(
                            "--slot needs at least one layer",
                        ));
                    }
                    if selectors.len() > ATLAS_MAX_LAYERS_PER_CELL {
                        return Err(DrawToolError::out_of_range(format!(
                            "a slot holds at most {ATLAS_MAX_LAYERS_PER_CELL} layers"
                        )));
                    }
                    out.slots.push((name.to_string(), selectors));
                    i += 2;
                }
                other => {
                    return Err(DrawToolError::invalid_state(format!(
                        "unknown atlas option {other:?}"
                    )));
                }
            }
        }
        if out.original_size && out.cell.is_some() {
            return Err(DrawToolError::invalid_state(
                "--original-size and --cell are mutually exclusive",
            ));
        }
        Ok(out)
    }
}
