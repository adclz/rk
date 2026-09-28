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
        /// Input field writes `(offset, value, field type)`; the type drives the
        /// store shape: scalar store, pointer store for a by-ref VAR_IN_OUT,
        /// `rk.str_assign` for STRING, `memory.copy` for aggregates.
        input_writes: Vec<(u32, MirExpr, MirType)>,
        /// Output reads `(offset, target place, field type, target lane)`, the
        /// same shapes copying field → target; `target_lane` is `Some` when the
        /// `=>` destination is wider than the field.
        output_reads: Vec<(u32, MirPlace, MirType, Option<MirElementary>)>,
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
        /// Where the counter lives: a wasm local or a `ThisField`; the loop reads
        /// and writes it through this place.
        control: crate::expr::MirPlace,
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

    /// Throw a wasm exception carrying a STRING payload
    /// (`throw $rk_exception`); the host catches it, there is no
    /// in-language catch.
    Raise { message: MirExpr },
}

impl MirStmt {
    /// Whether this statement, or one nested in it, reaches a place whose
    /// path runs a call ([`MirExpr::reaches_place_with_call`]).
    pub fn reaches_place_with_call(&self) -> bool {
        let body = |b: &[MirStmt]| b.iter().any(MirStmt::reaches_place_with_call);
        match self {
            MirStmt::Assign { target, value } => {
                target.has_call() || value.reaches_place_with_call()
            }
            MirStmt::Call(call) => call.reaches_place_with_call(),
            MirStmt::FbCall {
                instance,
                input_writes,
                output_reads,
                ..
            } => {
                instance.has_call()
                    || input_writes
                        .iter()
                        .any(|(_, v, _)| v.reaches_place_with_call())
                    || output_reads.iter().any(|(_, p, _, _)| p.has_call())
            }
            MirStmt::If {
                condition,
                then_body,
                else_ifs,
                else_body,
            } => {
                condition.reaches_place_with_call()
                    || body(then_body)
                    || else_ifs
                        .iter()
                        .any(|(c, b)| c.reaches_place_with_call() || body(b))
                    || else_body.as_deref().is_some_and(body)
            }
            MirStmt::Case {
                selector,
                arms,
                else_body,
            } => {
                selector.reaches_place_with_call()
                    || arms.iter().any(|arm| {
                        arm.patterns.iter().any(
                            |p| matches!(p, MirCasePattern::Test(t) if t.reaches_place_with_call()),
                        ) || body(&arm.body)
                    })
                    || else_body.as_deref().is_some_and(body)
            }
            MirStmt::For {
                control,
                start,
                end,
                step,
                body: b,
                ..
            } => {
                control.has_call()
                    || start.reaches_place_with_call()
                    || end.reaches_place_with_call()
                    || step.reaches_place_with_call()
                    || body(b)
            }
            MirStmt::While { condition, body: b } | MirStmt::Repeat { condition, body: b } => {
                condition.reaches_place_with_call() || body(b)
            }
            MirStmt::Raise { message } => message.reaches_place_with_call(),
            MirStmt::Return
            | MirStmt::Exit
            | MirStmt::Continue
            | MirStmt::MemStore { .. }
            | MirStmt::WasmIntrinsic { .. }
            | MirStmt::DebugTrap { .. } => false,
        }
    }
}

impl MirStmt {
    /// Whether `f` holds for an expression of this statement or of one nested
    /// in it: every expression, those in subscripts and call arguments
    /// included ([`MirExpr::any`]).
    pub fn any_expr<F: FnMut(&MirExpr) -> bool>(&self, f: &mut F) -> bool {
        fn body<F: FnMut(&MirExpr) -> bool>(b: &[MirStmt], f: &mut F) -> bool {
            b.iter().any(|s| s.any_expr(f))
        }
        match self {
            MirStmt::Assign { target, value } => target.any_expr(f) || value.any(f),
            MirStmt::Call(call) => call.any_expr(f),
            MirStmt::FbCall {
                instance,
                input_writes,
                output_reads,
                ..
            } => {
                instance.any_expr(f)
                    || input_writes.iter().any(|(_, v, _)| v.any(f))
                    || output_reads.iter().any(|(_, p, _, _)| p.any_expr(f))
            }
            MirStmt::If {
                condition,
                then_body,
                else_ifs,
                else_body,
            } => {
                condition.any(f)
                    || body(then_body, f)
                    || else_ifs.iter().any(|(c, b)| c.any(f) || body(b, f))
                    || else_body.as_deref().is_some_and(|b| body(b, f))
            }
            MirStmt::Case {
                selector,
                arms,
                else_body,
            } => {
                selector.any(f)
                    || arms.iter().any(|arm| {
                        arm.patterns
                            .iter()
                            .any(|p| matches!(p, MirCasePattern::Test(t) if t.any(f)))
                            || body(&arm.body, f)
                    })
                    || else_body.as_deref().is_some_and(|b| body(b, f))
            }
            MirStmt::For {
                control,
                start,
                end,
                step,
                body: b,
                ..
            } => control.any_expr(f) || start.any(f) || end.any(f) || step.any(f) || body(b, f),
            MirStmt::While { condition, body: b } | MirStmt::Repeat { condition, body: b } => {
                condition.any(f) || body(b, f)
            }
            MirStmt::Raise { message } => message.any(f),
            MirStmt::Return
            | MirStmt::Exit
            | MirStmt::Continue
            | MirStmt::MemStore { .. }
            | MirStmt::WasmIntrinsic { .. }
            | MirStmt::DebugTrap { .. } => false,
        }
    }
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
    /// Range: lower..=upper, compared signed or unsigned as the selector's
    /// type is.
    Range {
        lower: MirConstant,
        upper: MirConstant,
        signed: bool,
    },
    /// A label whose test is an expression deciding the arm on its own: a
    /// STRING label, whose test is the `str.byte_cmp` an `=` lowers to.
    Test(MirExpr),
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
