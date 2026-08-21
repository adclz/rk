//! Evaluating an expression to a value known before the program runs.
//!
//! "Constant" in IEC is a semantic property, not a shape: `Constant_Expr :
//! Expression`, with the constraint that it evaluate at compile time. A CASE
//! label, an array bound and a TASK period all state that same requirement,
//! so they ask the same question here rather than each deciding for itself
//! what counts — which is how they came to disagree, a label MIR could not
//! fold aborting codegen on source `rk check` had called clean.
//!
//! Two layers, because not every caller has a body:
//!
//! - [`Expr::as_const_int_folded`](crate::hir_def::expressions::expression::Expr::as_const_int_folded)
//!   folds what is written out — literals, a leading sign, parentheses.
//! - [`const_int`] extends that over resolved NAMES, so a `CONSTANT` and
//!   arithmetic over one evaluate too. It needs the body's inference result
//!   to know what a name bound to, which is why it lives here and not on
//!   `Expr`.

use db::WorkspaceDataBase;

use crate::{
    Qualifier,
    hir_def::{
        expressions::expression::{
            AddOperatorKind, Expr, ExprKind, InitExprKind, MultOperatorKind, PrimaryExpr,
        },
        pous::variable::VariableDecl,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use crate::HirNodeInfo;
use crate::hir_ty::body::infer_body;

/// The expression a `CONSTANT` declaration is fixed to, if it is one.
///
/// `VAR_EXTERNAL CONSTANT K : INT;` names a global and holds no value itself,
/// so the link is followed to the declaration that does. Anything not marked
/// `CONSTANT` yields `None` — an ordinary variable may be written between now
/// and whenever the value would be needed, so it is not knowable here.
pub fn constant_init<'db>(
    db: &'db dyn WorkspaceDataBase,
    decl: VariableDecl<'db>,
) -> Option<Expr<'db>> {
    if !decl.qualifier(db).contains(Qualifier::CONSTANT) {
        return None;
    }
    let decl = if decl.is_external(db) {
        crate::hir_ty::index_graphs::external_var_lookup(db, decl.name(db))?
    } else {
        decl
    };
    match decl.init(db)?.kind(db) {
        InitExprKind::ConstantExpr(init) => Some(init),
        _ => None,
    }
}

/// The compile-time integer value of `expr`, or `None` if it has none.
///
/// Overflow yields `None` rather than a wrapped value: a number the compiler
/// cannot represent is not a number it knows.
pub fn const_int<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    body: &BodyInferenceResult<'db>,
) -> Option<i64> {
    // Literals, a leading sign and parentheses.
    if let Some(v) = expr.as_const_int_folded(db) {
        return Some(v);
    }

    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) => {
            // The binding HIR resolved for this access. NOT normalized:
            // normalize peels the `Variable` wrapper down to the underlying
            // type, and the binding is exactly what is needed.
            let Type::Variable((decl, None)) =
                body.type_of_variable_access_with_adjustments(db, *va)
            else {
                return None;
            };
            const_int(db, constant_init(db, decl)?, body)
        }
        ExprKind::AddOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (const_int(db, *left, body)?, const_int(db, *right, body)?);
            match operator {
                AddOperatorKind::Plus => l.checked_add(r),
                AddOperatorKind::Minus => l.checked_sub(r),
            }
        }
        ExprKind::MultOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (const_int(db, *left, body)?, const_int(db, *right, body)?);
            match operator {
                MultOperatorKind::Mul => l.checked_mul(r),
                MultOperatorKind::Div => l.checked_div(r),
                MultOperatorKind::Mod => l.checked_rem(r),
            }
        }
        _ => None,
    }
}

/// Each variant of an enum with its ordinal: the declared value where one is
/// written, the previous ordinal plus one where not. `None` marks a declared
/// value that does not fold — the declaration check refuses it (E0704), so a
/// consumer reading ordinals afterwards may treat `None` as unreachable.
pub fn enum_ordinals<'db>(
    db: &'db dyn WorkspaceDataBase,
    enm: crate::hir_def::expressions::spec::Enum<'db>,
) -> Vec<(crate::hir_def::expressions::spec::EnumVariant<'db>, Option<i64>)> {

    let mut out = Vec::new();
    let mut next: i64 = 0;
    for variant in enm.variants(db).iter() {
        let value = match variant.value {
            Some(expr) => const_int(db, expr, infer_body(db, expr.get_scope_id(db))),
            None => Some(next),
        };
        if let Some(v) = value {
            next = v.wrapping_add(1);
        }
        out.push((*variant, value));
    }
    out
}
