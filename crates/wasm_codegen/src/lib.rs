//! WebAssembly code generation for IEC 61131-3 programs.
//!
//! Uses the MIR pipeline: HIR → MIR → WASM (via `from_mir`).

pub mod debug;
pub mod from_mir;

#[cfg(test)]
pub mod tests;
 