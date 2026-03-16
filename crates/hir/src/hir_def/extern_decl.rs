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
