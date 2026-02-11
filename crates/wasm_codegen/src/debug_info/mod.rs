//! DWARF debug information generation for WebAssembly modules.
//!
//! This module generates DWARF 5 debug symbols that enable source-level debugging
//! of IEC 61131-3 code compiled to WebAssembly. Debug information is **embedded
//! as custom sections** within the WASM binary itself.
//!
//! ## Architecture
//!
//! Debug info is collected during the normal codegen process:
//! 1. **Collector** - Gathers function, variable, and type information during module generation
//! 2. **DWARF Generator** - Converts collected info to DWARF 5 sections using gimli
//! 3. **Custom Sections** - Embeds DWARF sections (`.debug_info`, `.debug_line`, etc.) into WASM
//!
//! ## Usage
//!
//! ```rust,ignore
//! let mut codegen = ModuleCodeGen::new(db);
//! // During generation, debug info is automatically collected
//! let wasm_bytes = codegen.compile_module(db, files)?;
//! // WASM binary now contains embedded DWARF sections
//! // Debuggers (LLDB, GDB) automatically recognize these sections
//!
//! // For production builds, disable with --no-default-features
//! ```
//!
//! ## WASM Custom Sections
//!
//! The following DWARF sections are embedded as WASM custom sections:
//! - `.debug_info` - DIEs for compilation units, functions, variables, types
//! - `.debug_abbrev` - Abbreviation tables for compact encoding
//! - `.debug_line` - Line number program mapping WASM offsets to source lines
//! - `.debug_str` - String table for deduplicated strings

pub mod collector;
pub mod types;
pub mod dwarf_gen;
pub mod location;

// Phase 3+ modules (not yet implemented)
// pub mod line_program;

pub use collector::{DebugInfoCollector, FunctionDebugInfo, VariableDebugInfo};
pub use dwarf_gen::DwarfGenerator;
