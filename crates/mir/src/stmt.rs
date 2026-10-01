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
        /// A subrange counter's `(lower, upper)`: a loop that is ending does
        /// not step the counter out of them. Boxed like `step`.
        control_range: Option<Box<(i64, i64)>>,
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

/// Every statement of `stmts`, those nested in another included.
pub fn for_each_stmt(stmts: &[MirStmt], f: &mut impl FnMut(&MirStmt)) {
    for stmt in stmts {
        f(stmt);
        match stmt {
            MirStmt::If {
                then_body,
                else_ifs,
                else_body,
                ..
            } => {
                for_each_stmt(then_body, f);
                for (_, body) in else_ifs {
                    for_each_stmt(body, f);
                }
                for_each_stmt(else_body.as_deref().unwrap_or_default(), f);
            }
            MirStmt::Case {
                arms, else_body, ..
            } => {
                for arm in arms {
                    for_each_stmt(&arm.body, f);
                }
                for_each_stmt(else_body.as_deref().unwrap_or_default(), f);
            }
            MirStmt::For { body, .. }
            | MirStmt::While { body, .. }
            | MirStmt::Repeat { body, .. } => for_each_stmt(body, f),
            _ => {}
        }
    }
}

/// Every call `stmts` make, each once: a call statement, and a call in any
/// expression, those in subscripts, assignment targets, arguments and
/// aggregate snapshots included.
pub fn for_each_call(stmts: &[MirStmt], f: &mut impl FnMut(&MirCall)) {
    // `any_expr` reaches the expressions of the statements nested in each.
    for stmt in stmts {
        stmt.any_expr(&mut |expr| {
            if let MirExpr::Call(call) = expr {
                f(call);
            }
            false
        });
    }
    // A call statement is no expression.
    for_each_stmt(stmts, &mut |stmt| {
        if let MirStmt::Call(call) = stmt {
            f(call);
        }
    });
}

/// [`for_each_stmt`], mutably.
pub fn for_each_stmt_mut(stmts: &mut [MirStmt], f: &mut impl FnMut(&mut MirStmt)) {
    for stmt in stmts {
        f(stmt);
        match stmt {
            MirStmt::If {
                then_body,
                else_ifs,
                else_body,
                ..
            } => {
                for_each_stmt_mut(then_body, f);
                for (_, body) in else_ifs {
                    for_each_stmt_mut(body, f);
                }
                if let Some(body) = else_body {
                    for_each_stmt_mut(body, f);
                }
            }
            MirStmt::Case {
                arms, else_body, ..
            } => {
                for arm in arms {
                    for_each_stmt_mut(&mut arm.body, f);
                }
                if let Some(body) = else_body {
                    for_each_stmt_mut(body, f);
                }
            }
            MirStmt::For { body, .. }
            | MirStmt::While { body, .. }
            | MirStmt::Repeat { body, .. } => for_each_stmt_mut(body, f),
            _ => {}
        }
    }
}

/// [`for_each_call`], mutably.
pub fn for_each_call_mut(stmts: &mut [MirStmt], f: &mut impl FnMut(&mut MirCall)) {
    for stmt in stmts.iter_mut() {
        stmt.exprs_mut(&mut |expr| {
            if let MirExpr::Call(call) = expr {
                f(call);
            }
        });
    }
    for_each_stmt_mut(stmts, &mut |stmt| {
        if let MirStmt::Call(call) = stmt {
            f(call);
        }
    });
}

/// Every function `stmts` call: a call, and an FB body invoked.
pub(crate) fn callees(stmts: &[MirStmt], out: &mut Vec<Ident>) {
    for_each_call(stmts, &mut |call| out.push(call.callee));
    for_each_stmt(stmts, &mut |stmt| {
        if let MirStmt::FbCall { body_func, .. } = stmt {
            out.push(*body_func);
        }
    });
}

impl MirStmt {
    /// [`MirExpr::exprs_mut`] over every expression of this statement and of
    /// those nested in it: [`MirStmt::any_expr`], mutably.
    pub fn exprs_mut(&mut self, f: &mut impl FnMut(&mut MirExpr)) {
        match self {
            MirStmt::Assign { target, value } => {
                target.exprs_mut(f);
                value.exprs_mut(f);
            }
            MirStmt::Call(call) => call.exprs_mut(f),
            MirStmt::FbCall {
                instance,
                input_writes,
                output_reads,
                ..
            } => {
                instance.exprs_mut(f);
                for (_, value, _) in input_writes {
                    value.exprs_mut(f);
                }
                for (_, place, _, _) in output_reads {
                    place.exprs_mut(f);
                }
            }
            MirStmt::If {
                condition,
                then_body,
                else_ifs,
                else_body,
            } => {
                condition.exprs_mut(f);
                for stmt in then_body {
                    stmt.exprs_mut(f);
                }
                for (cond, body) in else_ifs {
                    cond.exprs_mut(f);
                    for stmt in body {
                        stmt.exprs_mut(f);
                    }
                }
                for stmt in else_body.iter_mut().flatten() {
                    stmt.exprs_mut(f);
                }
            }
            MirStmt::Case {
                selector,
                arms,
                else_body,
            } => {
                selector.exprs_mut(f);
                for arm in arms {
                    for pattern in &mut arm.patterns {
                        if let MirCasePattern::Test(test) = pattern {
                            test.exprs_mut(f);
                        }
                    }
                    for stmt in &mut arm.body {
                        stmt.exprs_mut(f);
                    }
                }
                for stmt in else_body.iter_mut().flatten() {
                    stmt.exprs_mut(f);
                }
            }
            MirStmt::For {
                control,
                start,
                end,
                step,
                body,
                ..
            } => {
                control.exprs_mut(f);
                start.exprs_mut(f);
                end.exprs_mut(f);
                step.exprs_mut(f);
                for stmt in body {
                    stmt.exprs_mut(f);
                }
            }
            MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
                condition.exprs_mut(f);
                for stmt in body {
                    stmt.exprs_mut(f);
                }
            }
            MirStmt::Raise { message } => message.exprs_mut(f),
            MirStmt::Return
            | MirStmt::Exit
            | MirStmt::Continue
            | MirStmt::MemStore { .. }
            | MirStmt::WasmIntrinsic { .. }
            | MirStmt::DebugTrap { .. } => {}
        }
    }

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
