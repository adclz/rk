//! Pre-compiled WASM math intrinsics, extracted from `wasm_builtins.wasm`.
//!
//! Everything below is re-exported from `generated`, which is itself
//! produced by this crate's `build.rs` (and gitignored). Do not put any
//! hand-written code into `src/generated.rs` — it gets overwritten.

mod generated;

pub use generated::*;
