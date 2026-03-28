use compact_str::CompactString;

use crate::hir_def::interned::identifier::SpanIdent;

/// A single extern pragma declaration, mapping a POU to a WASM import.
///
/// Example: `{extern 'math' 'sqrt' (param IN) (result SQRT)}`
///
/// For generic functions, the WASM import name is derived by appending
/// the concrete type: e.g. `math.sqrt.REAL`, `math.sqrt.LREAL`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ExternDecl<'db> {
    /// The WASM module name (e.g., "math")
    pub module: CompactString,
    /// The WASM function name (e.g., "sqrt")
    pub name: CompactString,
    /// Parameter variable references (e.g., `IN`, `a`, `b`)
    pub params: Vec<SpanIdent<'db>>,
    /// Result variable reference (e.g., the function name for return value assignment)
    pub result: Option<SpanIdent<'db>>,
}

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
