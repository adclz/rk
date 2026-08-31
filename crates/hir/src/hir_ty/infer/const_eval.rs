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
    const_int_guarded(db, expr, body, &mut Vec::new())
}

fn const_int_guarded<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    body: &BodyInferenceResult<'db>,
    // Same overflow-not-diagnostic hazard as the spec flavor: a CONSTANT
    // cycle must answer None, not recurse forever.
    visited: &mut Vec<VariableDecl<'db>>,
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
            if visited.contains(&decl) {
                return None;
            }
            visited.push(decl);
            let v = const_int_guarded(db, constant_init(db, decl)?, body, visited);
            visited.pop();
            v
        }
        ExprKind::AddOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (
                const_int_guarded(db, *left, body, visited)?,
                const_int_guarded(db, *right, body, visited)?,
            );
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
            let (l, r) = (
                const_int_guarded(db, *left, body, visited)?,
                const_int_guarded(db, *right, body, visited)?,
            );
            match operator {
                MultOperatorKind::Mul => l.checked_mul(r),
                MultOperatorKind::Div => l.checked_div(r),
                MultOperatorKind::Mod => l.checked_rem(r),
            }
        }
        _ => None,
    }
}

/// [`const_int`] for a SPEC-context expression — an enum variant value, an
/// array or subrange bound. These are typed by INIT inference, not body
/// inference, so looking in `infer_body` alone found no binding and a
/// CONSTANT-referencing bound failed to fold after the check accepted it.
/// [`const_int`] for a SPEC-context expression — an enum variant value, an
/// array or subrange bound. Query-free: names resolve through the scope
/// chain's declaration maps alone, so this is callable from anywhere — a
/// diagnostic message rendered inside `infer_initialization` included.
pub fn spec_bound<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> Option<i64> {
    const_int_in_spec(db, expr, &mut Vec::new())
}

fn const_int_in_spec<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    // The chain of CONSTANTs already being evaluated: `k1 := k2; k2 := k1`
    // used to recurse to a stack overflow (SIGABRT, no diagnostic) — a cycle
    // simply does not fold.
    visited: &mut Vec<VariableDecl<'db>>,
) -> Option<i64> {
    // Literals, a leading sign and parentheses.
    if let Some(v) = expr.as_const_int_folded(db) {
        return Some(v);
    }

    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) => {
            let decl = spec_name_binding(db, *va)?;
            if visited.contains(&decl) {
                return None;
            }
            visited.push(decl);
            let v = const_int_in_spec(db, constant_init(db, decl)?, visited);
            visited.pop();
            v
        }
        ExprKind::AddOperator {
            left,
            operator,
            right,
        } => {
            let (l, r) = (
                const_int_in_spec(db, *left, visited)?,
                const_int_in_spec(db, *right, visited)?,
            );
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
            let (l, r) = (
                const_int_in_spec(db, *left, visited)?,
                const_int_in_spec(db, *right, visited)?,
            );
            match operator {
                MultOperatorKind::Mul => l.checked_mul(r),
                MultOperatorKind::Div => l.checked_div(r),
                MultOperatorKind::Mod => l.checked_rem(r),
            }
        }
        _ => None,
    }
}

/// The declaration a bare name in a SPEC bound refers to, resolved through
/// the scope chain's declaration maps alone — no inference query, so this is
/// callable from anywhere, a diagnostic message being rendered inside
/// `infer_initialization` included. A namespaced or otherwise non-bare name
/// yields `None` and the bound is refused as non-constant.
pub(crate) fn spec_name_binding<'db>(
    db: &'db dyn WorkspaceDataBase,
    va: crate::hir_def::expressions::expression::VariableAccess<'db>,
) -> Option<crate::hir_def::pous::variable::VariableDecl<'db>> {
    use crate::HirNodeInfo;
    use crate::hir_def::expressions::expression::{PathExprKind, VarAccess, VariableAccessKind};
    use crate::hir_def::semantic_index::get_scope;

    if va.multibits(db).is_some() {
        return None;
    }
    let VariableAccessKind::Symbolic(begin) = va.kind(db) else {
        return None;
    };
    if begin.invocation(db).is_some() {
        return None;
    }
    let path = begin.expr(db)?;
    let PathExprKind::VarAccess(VarAccess::Simple(span_ident)) = path.expr(db) else {
        return None;
    };
    let ident = span_ident.ident;

    let mut scope = Some(va.get_scope_id(db));
    while let Some(sc) = scope {
        if let Some(decl) = sc.def_map(db).global_variables.get(&ident.caseless(db)) {
            return Some(*decl);
        }
        scope = get_scope(db, sc).parent;
    }
    None
}

/// The bare identifier a simple access names, if it is one — the same shape
/// [`spec_name_binding`] resolves, exposed for diagnostics that want to look
/// the name up elsewhere (the app-level global map) to say WHY it did not
/// resolve here.
pub(crate) fn bare_access_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    va: crate::hir_def::expressions::expression::VariableAccess<'db>,
) -> Option<crate::hir_def::interned::identifier::Ident> {
    use crate::hir_def::expressions::expression::{PathExprKind, VarAccess, VariableAccessKind};
    if va.multibits(db).is_some() {
        return None;
    }
    let VariableAccessKind::Symbolic(begin) = va.kind(db) else {
        return None;
    };
    if begin.invocation(db).is_some() {
        return None;
    }
    let path = begin.expr(db)?;
    let PathExprKind::VarAccess(VarAccess::Simple(span_ident)) = path.expr(db) else {
        return None;
    };
    Some(span_ident.ident)
}

/// Each variant of an enum with its ordinal: the declared value where one is
/// written, the previous ordinal plus one where not. `None` marks a declared
/// value that does not fold — the declaration check refuses it (E0704), so a
/// consumer reading ordinals afterwards may treat `None` as unreachable.
pub fn enum_ordinals<'db>(
    db: &'db dyn WorkspaceDataBase,
    enm: crate::hir_def::expressions::spec::Enum<'db>,
) -> Vec<(
    crate::hir_def::expressions::spec::EnumVariant<'db>,
    Option<i64>,
)> {
    enum_ordinals_by(db, enm, |e| const_int_in_spec(db, e, &mut Vec::new()))
}

fn enum_ordinals_by<'db>(
    db: &'db dyn WorkspaceDataBase,
    enm: crate::hir_def::expressions::spec::Enum<'db>,
    fold: impl Fn(Expr<'db>) -> Option<i64>,
) -> Vec<(
    crate::hir_def::expressions::spec::EnumVariant<'db>,
    Option<i64>,
)> {
    let mut out = Vec::new();
    let mut next: i64 = 0;
    for variant in enm.variants(db).iter() {
        let value = match variant.value {
            Some(expr) => fold(expr),
            None => Some(next),
        };
        if let Some(v) = value {
            next = v.wrapping_add(1);
        }
        out.push((*variant, value));
    }
    out
}

/// A subrange's bounds, folded. `None` marks a bound that does not fold —
/// refused at the declaration (E0803), so a consumer reading bounds
/// afterwards may treat it as unreachable.
pub fn subrange_bounds<'db>(
    db: &'db dyn WorkspaceDataBase,
    subrange: crate::hir_def::expressions::spec::SubRange<'db>,
) -> (Option<i64>, Option<i64>) {
    (
        const_int_in_spec(db, subrange.lower(db), &mut Vec::new()),
        const_int_in_spec(db, subrange.upper(db), &mut Vec::new()),
    )
}

/// An array's dimensions, folded — `(lower, upper)` per dimension. `None`
/// marks a bound that does not fold, refused at the declaration
/// (E0601/E0602).
pub fn array_dimensions<'db>(
    db: &'db dyn WorkspaceDataBase,
    array: crate::hir_def::expressions::spec::Array<'db>,
) -> Vec<(Option<i64>, Option<i64>)> {
    let fold = |e: Expr<'db>| const_int_in_spec(db, e, &mut Vec::new());
    array
        .subranges(db)
        .iter()
        .map(|(lo, hi)| (fold(*lo), fold(*hi)))
        .collect()
}

/// Follow a pure chain of `CONSTANT` references to the expression at its
/// end: `:= K` where `K : REAL := 2.5` resolves to the `2.5`. Resolution is
/// the SPEC rule — the scope chain's declaration maps, `VAR_EXTERNAL
/// CONSTANT` links followed — so a constant visible only through app-level
/// linkage must be declared external where it is used, the ordinary ST
/// idiom. Only BARE accesses substitute (arithmetic over a reference is
/// [`spec_bound`]'s job), and a cycle answers `None` rather than looping.
pub fn resolve_constant_ref<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
) -> Option<Expr<'db>> {
    fn walk<'db>(
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
        visited: &mut Vec<VariableDecl<'db>>,
    ) -> Option<Expr<'db>> {
        let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = expr.expr(db) else {
            return Some(expr);
        };
        let decl = spec_name_binding(db, *va)?;
        if visited.contains(&decl) {
            return None;
        }
        visited.push(decl);
        walk(db, constant_init(db, decl)?, visited)
    }
    let out = walk(db, expr, &mut Vec::new())?;
    (out != expr).then_some(out)
}

/// Whether `expr` is a literal-shaped constant: something MIR lowers to a
/// constant with no name resolution — a numeric/bool/time literal (signed,
/// parenthesized), an enum value, a string literal.
fn is_literal_shaped<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(_)) => true,
        ExprKind::PrimaryExpr(PrimaryExpr::EnumValue { .. }) => true,
        // References are exempt from the once-per-type rule on purpose: an
        // address is per-instance BY NATURE (`:= REF(member)` points into
        // whichever instance is being initialized), and `:= NULL` is as
        // constant as it gets. The nullability analysis is built on these
        // being legal member defaults.
        ExprKind::PrimaryExpr(PrimaryExpr::RefValue { .. }) => true,
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            is_literal_shaped(db, *expr)
        }
        ExprKind::UnaryOperator { expr, .. } => is_literal_shaped(db, *expr),
        _ => false,
    }
}

/// The ONE acceptance test for a once-per-type initializer leaf (TYPE
/// defaults, FB/CLASS member defaults, static-host initializers): the check
/// refuses what this rejects, and MIR folds what it accepts — the two agree
/// BY CONSTRUCTION because they both call this.
///
/// Accepted: anything [`spec_bound`] folds (integer arithmetic over literals
/// and CONSTANTs), any literal-shaped value, and a pure CONSTANT-reference
/// chain ending in one.
pub fn init_leaf_is_constant<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> bool {
    if spec_bound(db, expr).is_some() || is_literal_shaped(db, expr) {
        return true;
    }
    matches!(resolve_constant_ref(db, expr), Some(end) if is_literal_shaped(db, end))
}
