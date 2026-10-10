//! pixel-engine — the pure drawing engine ported from `src/` (docs/native_plan.md §1).
//!
//! Layering, mirroring the JS dependency DAG:
//!
//! ```text
//! constants errors events ← pixel ← blend ← chunkstore ← raster ← kernels
//!                                                    ↓
//!                     composite ← model (commands ← history)
//! ```
//!
//! Nothing here may touch the OS, the GPU or the network. The same rule the JS
//! tree enforced as "DOM-free, importable in Node" (docs/contract.md §4).

pub mod blend;
pub mod chunkstore;
pub mod constants;
pub mod errors;
pub mod events;
pub mod fixtures;
pub mod pixel;
pub mod raster;

#[cfg(test)]
mod tests {
    #[test]
    fn workspace_skeleton_builds() {
        assert_eq!(crate::constants::CHUNK_PX, 32);
    }
}
