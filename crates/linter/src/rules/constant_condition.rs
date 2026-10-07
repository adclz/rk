// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{ComparisonOperatorKind, Elementary, Expr, ExprKind, PrimaryExpr},
            statement::{Stmt, StmtKind},
        },
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::ScopeInference,
        infer::const_eval::{held_as, integer_range, spec_value, subrange_bounds},
        ty::Type,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "constant-condition";

/// L0103: condition is always true or always false.
struct ConstantCondition;

impl ErrorCode for ConstantCondition {
    fn code(&self) -> &'static str {
        "L0103"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let statements = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => match pou {
            Pou::Function(f) => f.statements(db),
            Pou::FunctionBlock(fb) => fb.statements(db),
            _ => return,
        },
        ScopeKind::MethodDecl(m) => m.stmts(db),
        ScopeKind::Program(program) => program.statements(db),
        _ => return,
    };

    check_statements(db, statements, diagnostics);
}

fn check_statements<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
                ..
            } => {
                check_condition(db, condition, "IF", diagnostics);
                if let Some(stmts) = then {
                    check_statements(db, stmts, diagnostics);
                }
                for (cond, stmts) in else_if {
                    check_condition(db, cond, "ELSIF", diagnostics);
                    check_statements(db, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, stmts, diagnostics);
                }
            }
            StmtKind::While {
                condition, body, ..
            } => {
                check_condition(db, condition, "WHILE", diagnostics);
                check_statements(db, body, diagnostics);
            }
            StmtKind::Repeat {
                condition, body, ..
            } => {
                check_condition(db, condition, "UNTIL", diagnostics);
                check_statements(db, body, diagnostics);
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, stmts) in cases {
                    check_statements(db, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, stmts, diagnostics);
                }
            }
            StmtKind::For { body, .. } => {
                check_statements(db, body, diagnostics);
            }
            _ => {}
        }
    }
}

pub fn check_condition<'db>(
    db: &'db dyn WorkspaceDataBase,
    condition: &Expr<'db>,
    keyword: &str,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let value_str = match is_boolean_literal(db, condition) {
        Some(true) => "TRUE",
        Some(false) => "FALSE",
        None => return,
    };

    diagnostics.push(
        diag()
            .message(format!("{keyword} condition is always {value_str}",))
            .desc(&ConstantCondition)
            .range(
                hir::denormalize(
                    db,
                    condition.get_scope_id(db).file(db),
                    &condition.get_span(db),
                )
                .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}

/// Returns `Some(true)` for TRUE, `Some(false)` for FALSE, `None` for non-boolean-literal.
fn is_boolean_literal<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> Option<bool> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(literal @ Elementary::Bool(_))) => {
            literal.as_bool(db)
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            is_boolean_literal(db, expr)
        }
        _ => None,
    }
}

/// A comparison of a value with a constant, which the value's type decides:
/// `u < 0` on an unsigned integer is always FALSE, `s <= 127` on a SINT
/// always TRUE, `l > 10` on an `INT (0..10)` always FALSE.
pub fn check_comparison<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let ExprKind::ComparisonOperator {
        left,
        operator,
        right,
    } = expr.expr(db)
    else {
        return;
    };
    // The constant on either side, the operator read with the value first:
    // `0 > u` is `u < 0`. Two constants are a configuration, `DEBUG > 2`,
    // which a CONSTANT is there to switch.
    let (value, operator, constant) = match (spec_value(db, *left), spec_value(db, *right)) {
        (None, Some(constant)) => (left, operator, constant),
        (Some(constant), None) => (right, mirrored(operator), constant),
        _ => return,
    };
    let declared = body.type_of_expr_adjusted(*value);
    // What the value can hold: a subrange its bounds, an integer its type's.
    let (low, high) = match declared.as_subrange(db) {
        Some(sub) => match subrange_bounds(db, sub) {
            (Some(low), Some(high)) => (i128::from(low), i128::from(high)),
            _ => return,
        },
        None => match declared.normalize(db) {
            Type::Elementary(spec) => match integer_range(spec) {
                Some(range) => range,
                None => return,
            },
            _ => return,
        },
    };
    // The constant as the comparison holds it: `16#FFFF` against an INT is -1.
    let compared_at = body
        .comparison_operand_type(*expr)
        .unwrap_or_else(|| declared.normalize(db));
    let constant = held_as(db, constant, compared_at);
    let always = match operator {
        ComparisonOperatorKind::Lt if high < constant => true,
        ComparisonOperatorKind::Lt if low >= constant => false,
        ComparisonOperatorKind::Le if high <= constant => true,
        ComparisonOperatorKind::Le if low > constant => false,
        ComparisonOperatorKind::Gt if low > constant => true,
        ComparisonOperatorKind::Gt if high <= constant => false,
        ComparisonOperatorKind::Ge if low >= constant => true,
        ComparisonOperatorKind::Ge if high < constant => false,
        ComparisonOperatorKind::Eq if constant < low || constant > high => false,
        ComparisonOperatorKind::Ne if constant < low || constant > high => true,
        _ => return,
    };
    let mut d = diag()
        .message(format!(
            "the comparison is always {}",
            if always { "TRUE" } else { "FALSE" }
        ))
        .desc(&ConstantCondition)
        .range(
            hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    d.with_note(format!("{} holds {low} to {high}", declared.type_name(db)));
    diagnostics.push(d);
}

/// The operator with its operands swapped: `0 > u` is `u < 0`.
fn mirrored(operator: &ComparisonOperatorKind) -> &ComparisonOperatorKind {
    match operator {
        ComparisonOperatorKind::Lt => &ComparisonOperatorKind::Gt,
        ComparisonOperatorKind::Le => &ComparisonOperatorKind::Ge,
        ComparisonOperatorKind::Gt => &ComparisonOperatorKind::Lt,
        ComparisonOperatorKind::Ge => &ComparisonOperatorKind::Le,
        same => same,
    }
}
