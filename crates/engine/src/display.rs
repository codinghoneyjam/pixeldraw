//! Display framebuffer scaffolding for Phase 2.
//!
//! This stub keeps the engine-side API surface in place while the real CPU
//! display pipeline is implemented in the native shell tasks.

/// Placeholder framebuffer type used by the display layer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framebuffer {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            rgba: vec![0; (width as usize) * (height as usize) * 4],
        }
    }

    pub fn resize_clear(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.rgba
            .resize((width as usize) * (height as usize) * 4, 0);
    }
}
