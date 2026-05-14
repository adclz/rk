use crate::expr::{MirCall, MirConstant, MirExpr, MirPlace};
use crate::types::MirElementary;
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
        /// Input field writes: (field_offset, value, field_type).
        input_writes: Vec<(u32, MirExpr, MirElementary)>,
        /// Output reads: (field_offset, target_place, field_type).
        output_reads: Vec<(u32, MirPlace, MirElementary)>,
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
        step: MirExpr,
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

    /// Debug trap point (optional, only when debug mode enabled).
    DebugTrap {
        trap_id: u32,
        location: MirSourceLocation,
    },

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
    pub file_id: u32,
    pub line: u32,
    pub column: u32,
}
