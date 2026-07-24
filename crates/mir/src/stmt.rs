use crate::expr::{MirCall, MirConstant, MirExpr, MirPlace};
use crate::types::{MirElementary, MirType};
use compact_str::CompactString;
use hir::hir_def::interned::identifier::Ident;

/// A statement in MIR.
#[derive(Debug, Clone)]
pub enum MirStmt {
    /// Simple assignment: place = expr.
    Assign { target: MirPlace, value: MirExpr },

    /// Function/method call as statement (result discarded if any).
    Call(MirCall),

    /// Function block invocation as statement.
    /// Writes input args to the FB instance's fields, calls __body__(&instance).
    FbCall {
        /// Address of the FB instance in linear memory.
        instance: MirPlace,
        /// The body function name: "FBName$__body__"
        body_func: Ident,
        /// Function index (resolved during module lowering).
        body_func_index: u32,
        /// Input field writes: (field_offset, value, field_type). The field
        /// type drives the store shape: Elementary/Enum/Subrange are typed
        /// scalar stores; Pointer (a by-ref VAR_IN_OUT) stores the address
        /// carried by the value (an `AddrOf`); String copies via
        /// `rk.str_assign` (value pushes `(ptr, len)`); Struct/Array bulk-copy
        /// via `memory.copy` (value is an `AddrOf` of the source aggregate).
        input_writes: Vec<(u32, MirExpr, MirType)>,
        /// Output reads: (field_offset, target_place, field_type). Same
        /// type-driven shapes as `input_writes`, copying field → target.
        output_reads: Vec<(u32, MirPlace, MirType)>,
    },

    /// Return from function.
    Return,

    /// Structured if/elsif/else.
    If {
        condition: MirExpr,
        then_body: Vec<MirStmt>,
        else_ifs: Vec<(MirExpr, Vec<MirStmt>)>,
        else_body: Option<Vec<MirStmt>>,
    },

    /// Case statement.
    Case {
        /// The case selector expression. Always integer-typed.
        selector: MirExpr,
        arms: Vec<MirCaseArm>,
        else_body: Option<Vec<MirStmt>>,
    },

    /// For loop.
    For {
        control_var: Ident,
        control_type: MirElementary,
        start: MirExpr,
        end: MirExpr,
        /// Always present; default 1 is materialized during lowering.
        /// Boxed to keep the variant small (`clippy::large_enum_variant`).
        step: Box<MirExpr>,
        body: Vec<MirStmt>,
    },

    /// While loop.
    While {
        condition: MirExpr,
        body: Vec<MirStmt>,
    },

    /// Repeat-until loop.
    Repeat {
        condition: MirExpr,
        body: Vec<MirStmt>,
    },

    /// Exit (break from innermost loop).
    Exit,

    /// Continue (jump to start of innermost loop).
    Continue,

    /// Direct memory store for flat initialization sequences.
    MemStore { offset: u32, value: MirConstant },

    /// Direct WASM instruction from a `{wasm}` pragma: params pushed in
    /// order, the result popped and stored.
    WasmIntrinsic {
        /// Full WASM instruction (e.g., "i32.shl", "f32.convert_i32_s")
        instruction: compact_str::CompactString,
        /// Variables to push on the stack before the instruction
        params: Vec<Ident>,
        /// Variable to store the result
        result: Option<Ident>,
    },

    /// Debug stop-point marker: no wasm, only the statement's position for
    /// the `debug-lines` table.
    DebugTrap { location: MirSourceLocation },

    /// Throws a wasm-level exception carrying a STRING payload. Lowers
    /// to push `(ptr, len)` from the message expression onto the stack,
    /// then `throw $rk_exception`. The host catches it at the wasm
    /// boundary; there is no in-language catch (no `__TRY`).
    Raise { message: MirExpr },
}

#[derive(Debug, Clone)]
pub struct MirCaseArm {
    /// Match patterns: single values or ranges.
    pub patterns: Vec<MirCasePattern>,
    pub body: Vec<MirStmt>,
}

#[derive(Debug, Clone)]
pub enum MirCasePattern {
    /// Single constant value.
    Value(MirConstant),
    /// Range: lower..=upper.
    Range {
        lower: MirConstant,
        upper: MirConstant,
    },
}

#[derive(Debug, Clone)]
pub struct MirSourceLocation {
    /// Source file URL; its index into the module's file table is resolved
    /// at codegen.
    pub file_url: CompactString,
    /// 0-based source line and column (tree-sitter row/column).
    pub line: u32,
    pub column: u32,
}
