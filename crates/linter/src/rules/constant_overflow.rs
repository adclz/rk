// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PrimaryExpr, VariableAccess},
            spec::{ElementarySpec, Spec, SpecKind},
        },
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        infer::const_eval::{Overflow, integer_holds, integer_range, overflow, spec_name_binding},
        ty::Type,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "constant-overflow";

/// L0128: an operation on constants whose result the type it runs at cannot
/// hold, which the program computes wrapped.
struct ConstantOverflow;

impl ErrorCode for ConstantOverflow {
    fn code(&self) -> &'static str {
        "L0128"
    }
}

/// Every operation of the scope: its statements, its initializers, and the
/// sizes and bounds of what it declares.
pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let body = scope.inference(db);
    let in_body = |va: VariableAccess<'db>| match body.type_of_variable_access_adjusted(va) {
        Type::Variable((decl, None)) => Some(decl),
        _ => None,
    };
    let in_declaration = |va| spec_name_binding(db, va);

    let mut found: Vec<(Expr<'db>, Overflow)> = Vec::new();
    let mut seen = FxHashSet::default();
    for (expr, _) in body.typed_exprs() {
        if seen.insert(expr)
            && let Some(overflow) = overflow(db, expr, &in_body)
        {
            found.push((expr, overflow));
        }
    }
    let mut sizes = Vec::new();
    for spec in declared_specs(db, scope) {
        spec_expressions(db, spec, &mut sizes, 0);
    }
    for root in sizes {
        operations(db, root, &mut |expr| {
            if seen.insert(expr)
                && let Some(overflow) = overflow(db, expr, &in_declaration)
            {
                found.push((expr, overflow));
            }
        });
    }
    // In source order: the maps are not.
    found.sort_by_key(|(expr, _)| expr.get_span(db).start_byte);
    for (expr, overflow) in found {
        diagnostics.push(report(db, expr, overflow));
    }
}

fn report<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    overflow: Overflow,
) -> IdeDiagnostic {
    let ty = overflow.ty.type_name();
    let mut d = diag()
        .message(format!(
            "the result, {}, does not fit in {ty}",
            overflow.exact
        ))
        .desc(&ConstantOverflow)
        .range(
            hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    if let Some((min, max)) = integer_range(overflow.ty) {
        d.with_note(format!(
            "{ty} holds {min} to {max}: the program computes {}",
            overflow.held
        ));
    }
    if let Some(wider) = wider_holding(overflow.ty, overflow.exact) {
        let wider = wider.type_name();
        d.with_help(match typed_left_operand(db, expr, wider) {
            Some(rewritten) => format!("compute it at {wider}, '{rewritten}'"),
            None => format!("compute it at {wider}, with an operand of that type"),
        });
    }
    d
}

/// The next type of `ty`'s kind, signed, unsigned or bit string, that holds
/// `value`.
fn wider_holding(ty: ElementarySpec, value: i128) -> Option<ElementarySpec> {
    use ElementarySpec::*;
    let kind: &[ElementarySpec] = match ty {
        SInt | Int | DInt | LInt => &[SInt, Int, DInt, LInt],
        USInt | UInt | UDInt | ULInt => &[USInt, UInt, UDInt, ULInt],
        Byte | Word | DWord | LWord => &[Byte, Word, DWord, LWord],
        _ => return None,
    };
    kind.iter()
        .skip_while(|t| **t != ty)
        .skip(1)
        .find(|t| integer_holds(value, **t) == Some(true))
        .copied()
}

/// The operation with its left operand, an untyped literal, typed `wider`:
/// `200 * 200` is `DINT#200 * 200`, which then runs at DINT.
fn typed_left_operand<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    wider: &str,
) -> Option<String> {
    let (ExprKind::AddOperator { left, .. } | ExprKind::MultOperator { left, .. }) = expr.expr(db)
    else {
        return None;
    };
    if !matches!(
        left.expr(db),
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(_)))
    ) {
        return None;
    }
    let written = expr.as_call_site(db).to_string(db);
    let literal = left.as_call_site(db).to_string(db);
    let rest = written.strip_prefix(literal.as_str())?;
    Some(format!("{wider}#{literal}{rest}"))
}

/// The specs the scope declares: its variables', a TYPE's own.
fn declared_specs<'db>(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Vec<Spec<'db>> {
    let variables: &[VariableDecl<'db>] = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => f.variables(db),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => fb.variables(db),
        ScopeKind::Pou(Pou::Class(class)) => class.variables(db),
        ScopeKind::Pou(Pou::DataType(dt)) => return vec![dt.spec(db)],
        ScopeKind::MethodDecl(m) => m.variables(db),
        ScopeKind::Program(program) => program.variables(db),
        _ => &[],
    };
    variables.iter().map(|var| var.spec(db)).collect()
}

/// The expressions a spec computes a size or a value from: a STRING's
/// length, an array's bounds, a subrange's, an enum's values, those of the
/// specs inside it.
fn spec_expressions<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: Spec<'db>,
    out: &mut Vec<Expr<'db>>,
    depth: u32,
) {
    // A cyclic alias is refused elsewhere; stop regardless.
    if depth > 16 {
        return;
    }
    match spec.kind(db) {
        SpecKind::SizedString(length) => out.push(*length),
        SpecKind::Array(array) => {
            for (lower, upper) in array.subranges(db) {
                out.extend([lower, upper]);
            }
            spec_expressions(db, array.of_type(db), out, depth + 1);
        }
        SpecKind::Subrange(sub) => {
            out.extend([sub.lower(db), sub.upper(db)]);
        }
        SpecKind::Struct(strukt) => {
            for element in strukt.elements(db) {
                spec_expressions(db, element.spec(db), out, depth + 1);
            }
        }
        SpecKind::Enum(enm) => out.extend(enm.variants(db).iter().filter_map(|v| v.value)),
        SpecKind::Ref(inner) => spec_expressions(db, *inner, out, depth + 1),
        _ => {}
    }
}

/// `expr` and every operation inside it.
fn operations<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>, f: &mut impl FnMut(Expr<'db>)) {
    f(expr);
    match expr.expr(db) {
        ExprKind::AddOperator { left, right, .. } | ExprKind::MultOperator { left, right, .. } => {
            operations(db, *left, f);
            operations(db, *right, f);
        }
        ExprKind::UnaryOperator { expr, .. }
        | ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            operations(db, *expr, f)
        }
        _ => {}
    }
}
