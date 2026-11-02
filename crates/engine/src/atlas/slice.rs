//! Grid slicing for ordinary, metadata-free sprite sheets.

use crate::errors::{DrawToolError, EngineResult};
use crate::io::png::PngImage;

use super::ATLAS_MAX_CELLS;

/// Pixel geometry for importing a regular sprite-sheet grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridSliceOptions {
    pub cell_w: u32,
    pub cell_h: u32,
    pub columns: u32,
    pub rows: u32,
    pub offset_x: u32,
    pub offset_y: u32,
    pub spacing_x: u32,
    pub spacing_y: u32,
    /// Keep transparent cells so frame order and grid positions stay stable.
    pub keep_empty: bool,
}

/// A copied cell in row-major sheet order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlicedAtlasCell {
    pub index: usize,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub is_empty: bool,
}

/// Cut a PNG into a validated regular grid.
///
/// Every requested rectangle must fit completely in the image. This avoids
/// silently clipping a last row or column when the entered cell size is wrong.
pub fn slice_png_grid(
    image: &PngImage,
    options: GridSliceOptions,
) -> EngineResult<Vec<SlicedAtlasCell>> {
    let expected = u64::from(image.width)
        .checked_mul(u64::from(image.height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| DrawToolError::out_of_range("PNG dimensions overflow"))?;
    if image.width == 0 || image.height == 0 || expected != image.rgba.len() as u64 {
        return Err(DrawToolError::invalid_state(
            "PNG pixel buffer does not match its dimensions",
        ));
    }
    if options.cell_w == 0 || options.cell_h == 0 {
        return Err(DrawToolError::out_of_range(
            "atlas cell size must be at least 1x1",
        ));
    }
    if !(1..=ATLAS_MAX_CELLS as u32).contains(&options.columns)
        || !(1..=ATLAS_MAX_CELLS as u32).contains(&options.rows)
    {
        return Err(DrawToolError::out_of_range(format!(
            "grid rows and columns must be 1..={ATLAS_MAX_CELLS}"
        )));
    }
    let capacity = u64::from(options.columns) * u64::from(options.rows);
    if capacity > ATLAS_MAX_CELLS as u64 {
        return Err(DrawToolError::out_of_range(format!(
            "atlas grid holds at most {ATLAS_MAX_CELLS} cells"
        )));
    }

    let stride_x = options
        .cell_w
        .checked_add(options.spacing_x)
        .ok_or_else(|| DrawToolError::out_of_range("atlas grid width overflow"))?;
    let stride_y = options
        .cell_h
        .checked_add(options.spacing_y)
        .ok_or_else(|| DrawToolError::out_of_range("atlas grid height overflow"))?;
    let mut cells = Vec::with_capacity(capacity as usize);
    for row in 0..options.rows {
        let y = options
            .offset_y
            .checked_add(
                row.checked_mul(stride_y)
                    .ok_or_else(|| DrawToolError::out_of_range("atlas row position overflow"))?,
            )
            .ok_or_else(|| DrawToolError::out_of_range("atlas row position overflow"))?;
        let bottom = y
            .checked_add(options.cell_h)
            .ok_or_else(|| DrawToolError::out_of_range("atlas cell bounds overflow"))?;
        if bottom > image.height {
            return Err(DrawToolError::out_of_range(format!(
                "grid row {} ends at {bottom}px but the PNG is {}px high",
                row + 1,
                image.height
            )));
        }
        for column in 0..options.columns {
            let x =
                options
                    .offset_x
                    .checked_add(column.checked_mul(stride_x).ok_or_else(|| {
                        DrawToolError::out_of_range("atlas column position overflow")
                    })?)
                    .ok_or_else(|| DrawToolError::out_of_range("atlas column position overflow"))?;
            let right = x
                .checked_add(options.cell_w)
                .ok_or_else(|| DrawToolError::out_of_range("atlas cell bounds overflow"))?;
            if right > image.width {
                return Err(DrawToolError::out_of_range(format!(
                    "grid column {} ends at {right}px but the PNG is {}px wide",
                    column + 1,
                    image.width
                )));
            }

            let mut rgba = vec![0; options.cell_w as usize * options.cell_h as usize * 4];
            let source_stride = image.width as usize * 4;
            let cell_stride = options.cell_w as usize * 4;
            let source_x = x as usize * 4;
            let mut is_empty = true;
            for dy in 0..options.cell_h as usize {
                let src = (y as usize + dy) * source_stride + source_x;
                let dst = dy * cell_stride;
                rgba[dst..dst + cell_stride].copy_from_slice(&image.rgba[src..src + cell_stride]);
                if is_empty
                    && rgba[dst..dst + cell_stride]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|pixel| pixel[3] != 0)
                {
                    is_empty = false;
                }
            }
            if options.keep_empty || !is_empty {
                cells.push(SlicedAtlasCell {
                    index: (row * options.columns + column) as usize,
                    x,
                    y,
                    width: options.cell_w,
                    height: options.cell_h,
                    rgba,
                    is_empty,
                });
            }
        }
    }
    if cells.is_empty() {
        return Err(DrawToolError::invalid_state(
            "grid contains no non-transparent cells",
        ));
    }
    Ok(cells)
}
