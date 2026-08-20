use compact_str::CompactString;

use crate::hir_def::interned::identifier::SpanIdent;

/// A wasm intrinsic pragma, emitting a WASM instruction directly.
///
/// Example: `{wasm 'f32.convert_i32_s' (param IN) (result INT_TO_REAL)}`
///
/// Only a limited set of cast/conversion instructions are allowed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct WasmDecl<'db> {
    /// Optional type reference variable — when present, the MIR resolves
    /// the variable's type to determine the WASM instruction prefix
    /// (e.g., `IN` → `i32` → instruction becomes `i32.shl`).
    pub type_ref: Option<SpanIdent<'db>>,
    /// The WASM instruction name (e.g., "shl" or "f32.convert_i32_s")
    pub instruction: CompactString,
    /// Parameter variable references
    pub params: Vec<SpanIdent<'db>>,
    /// Result variable reference
    pub result: Option<SpanIdent<'db>>,
}
