//! Input event façade for the native tool pipeline.
//!
//! A pure state machine: `crates/app/src/input/**` must never name egui, winit
//! or eframe (`docs/contract.md` §4). The adapter that turns host events into
//! these types is `crate::input_adapter` (W8).

mod build_event;
mod controller;
mod cursor;
mod event;
mod keyboard;
mod wheel;

#[cfg(test)]
mod tests;

pub use build_event::{RawPointer, build_tool_event};
pub use controller::{HAND_TOOL_ID, InputController, InputCtx, InputOutcome, ToolCall};
pub use cursor::{CursorInputs, DEFAULT_CURSOR, PAN_CURSOR, SPACE_CURSOR, resolve_cursor};
pub use event::*;
pub use keyboard::{SPACE_CODE, SPACE_KEY, is_space, text_target_swallows_key};
pub use wheel::{WHEEL_PX_PER_STEP, WheelAccumulator, WheelMode};
