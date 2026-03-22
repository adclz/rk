use compact_str::CompactString;
use hir::hir_def::interned::identifier::Ident;

use crate::expr::MirExpr;
use crate::stmt::MirStmt;
use crate::types::MirType;

/// A lowered function, ready for codegen.
#[derive(Debug, Clone)]
pub struct MirFunction {
    /// Unique function name. For methods: "FB1$Run".
    /// For monomorphized: "ABS.INT", "ABS.REAL", etc.
    pub name: Ident,

    /// Original function name before mangling/monomorphization.
    pub origin_name: Ident,

    /// Pre-assigned function index in the WASM module.
    pub index: u32,

    /// Function parameters.
    pub params: Vec<MirParam>,

    /// Return type (None = void).
    pub return_type: Option<MirType>,

    /// Local variables (non-parameter).
    pub locals: Vec<MirLocal>,

    /// Function body as a list of statements.
    pub body: Vec<MirStmt>,

    /// How this function should appear externally.
    pub linkage: MirLinkage,

    /// Optional qualified export name (e.g. "Std.Bits.Test.test_shl_byte").
    /// When set, WASM codegen uses this instead of `name` for the export.
    pub export_name: Option<CompactString>,
}

/// An imported (extern) function declaration.
#[derive(Debug, Clone)]
pub struct MirExternFunction {
    pub name: Ident,
    /// Function index (imports come first in WASM).
    pub index: u32,
    /// Import module name.
    pub module: CompactString,
    /// Import field name.
    pub import_name: CompactString,
    pub params: Vec<MirParam>,
    pub return_type: Option<MirType>,
    /// If this was monomorphized from an ANY_* function.
    pub monomorphized_from: Option<Ident>,
}

#[derive(Debug, Clone)]
pub struct MirParam {
    pub name: Ident,
    pub ty: MirType,
    pub kind: MirParamKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirParamKind {
    /// By-value input parameter.
    Input,
    /// By-reference parameter (VAR_IN_OUT).
    InOut,
    /// Output parameter (VAR_OUTPUT).
    Output,
    /// Implicit 'this' pointer for methods.
    This,
}

/// A local variable within a function.
#[derive(Debug, Clone)]
pub struct MirLocal {
    pub name: Ident,
    pub ty: MirType,
    /// Optional initializer expression.
    pub init: Option<MirExpr>,
    pub kind: MirLocalKind,
    /// Pre-computed storage decision.
    pub storage: MirStorage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirLocalKind {
    Var,
    Temp,
    Output,
}

/// Storage decision for a variable — computed by MIR, consumed by codegen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirStorage {
    /// Fits in a WASM local (scalars, enums, subranges, pointers).
    Scalar {
        /// WASM local index.
        local_index: u32,
    },
    /// Must be allocated in linear memory (structs, arrays, FBs, strings).
    Memory {
        /// Absolute address in linear memory.
        address: u32,
        size: u32,
        align: u32,
    },
}

/// Function linkage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MirLinkage {
    /// Exported function (callable from host).
    Export,
    /// Internal function (only callable within the module).
    Internal,
}
