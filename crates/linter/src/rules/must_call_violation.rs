// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HasPragmas, HirNodeInfo,
    hir_def::{
        expressions::expression::{
            ExprKind, PathExpr, PrimaryExpr, RefValue, VariableAccess, VariableAccessKind,
        },
        pous::{
            function_block::FunctionBlock,
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::{ParamBinding, ScopeInference},
        infer::Infer,
        ty::{CallableType, Type},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "must-call-violation";

/// L0006: an instance of a `{must_call}` FUNCTION_BLOCK that is never called.
struct MustCallViolation;

impl ErrorCode for MustCallViolation {
    fn code(&self) -> &'static str {
        "L0006"
    }
}

/// The instances of `{must_call}` blocks a scope declares itself, alone or
/// as an array: an input, an output or an in-out is the caller's.
pub(crate) fn instances<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
) -> Vec<(VariableDecl<'db>, FunctionBlock<'db>)> {
    let variables: &[VariableDecl<'db>] = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => f.variables(db),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => fb.variables(db),
        ScopeKind::Pou(Pou::Class(class)) => class.variables(db),
        ScopeKind::MethodDecl(m) => m.variables(db),
        ScopeKind::Program(program) => program.variables(db),
        _ => return Vec::new(),
    };
    variables
        .iter()
        .filter(|var| matches!(var.kind(db), VariableKind::Var | VariableKind::Temp))
        .filter_map(|var| Some((*var, must_call_block(db, var.spec(db).infer(db))?)))
        .collect()
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let instances = instances(db, scope);
    if instances.is_empty() {
        return;
    }

    // Where they are called: the scope's body, and its methods'.
    let mut scopes = vec![scope];
    if let Some(methods) = scope.method_declarations(db) {
        scopes.extend(methods.iter().map(|m| m.get_scope_id(db)));
    }
    let mut called = FxHashSet::default();
    for scope in scopes {
        let body = scope.inference(db);
        for (call, resolved) in body.resolved_calls() {
            // `t(...)` runs the body; `t.m()` does not.
            if matches!(resolved.callable, CallableType::FunctionBlock(_))
                && let Some(root) = call.path(db).expr(db).and_then(|p| root(db, body, p))
            {
                called.insert(root);
            }
            // Handed on by reference, the instance is its holder's to call.
            for (param, binding) in &resolved.params {
                match binding {
                    ParamBinding::Values(values) if param.is_in_out(db) => {
                        for value in values {
                            if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) =
                                value.expr(db)
                                && let Some(root) = access_root(db, body, *access)
                            {
                                called.insert(root);
                            }
                        }
                    }
                    ParamBinding::Output { variable, .. } => {
                        if let Some(root) = access_root(db, body, *variable) {
                            called.insert(root);
                        }
                    }
                    _ => {}
                }
            }
        }
        // `REF(t)` hands it on too.
        for (expr, _) in body.typed_exprs() {
            if let ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                value: RefValue::Address(begin),
            }) = expr.expr(db)
                && let Some(root) = begin.expr(db).and_then(|p| root(db, body, p))
            {
                called.insert(root);
            }
        }
    }
    // A configuration names a member a task runs, `fb1 WITH T`.
    if matches!(
        get_scope(db, scope).kind,
        ScopeKind::Program(_) | ScopeKind::Pou(Pou::FunctionBlock(_) | Pou::Class(_))
    ) {
        for config in hir::hir_ty::index_graphs::declared_configs(db) {
            called.extend(config.scope_id(db).inference(db).variables_used());
        }
    }

    for (var, block) in instances {
        // `o.t()` from outside the block that declares `t`.
        if called.contains(&var) || hir::hir_ty::calls::reached_through_a_path(db, var) {
            continue;
        }
        diagnostics.push(report(db, var, block));
    }
}

fn report<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    block: FunctionBlock<'db>,
) -> IdeDiagnostic {
    let name = var.name_with_case(db).text(db);
    let block_name = block.get_name_with_case(db).text(db);
    let mut d = diag()
        .message(format!("'{name}' is never called"))
        .desc(&MustCallViolation)
        .range(
            hir::denormalize(db, var.get_scope_id(db).file(db), &var.get_name_span(db))
                .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    if let Some(pragma) = block.must_call_pragma(db) {
        d.with_related(Related::new(
            format!("'{block_name}' is marked {{must_call}} here"),
            pragma.get_scope_id(db).file(db),
            pragma.get_span(db),
        ));
    }
    d.with_note("every instance of a {must_call} block is to be called".to_string());
    d.with_help(format!("call '{name}' at every scan"));
    d
}

/// The `{must_call}` block a variable is an instance of, alone or as the
/// elements of an array.
fn must_call_block<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: Type<'db>,
) -> Option<FunctionBlock<'db>> {
    match ty.normalize(db) {
        Type::FunctionBlock(fb) => fb.must_call_pragma(db).is_some().then_some(fb),
        Type::Array(array) => must_call_block(db, array.of_type(db).infer(db)),
        _ => None,
    }
}

/// The variable a path starts from: `t` for `t`, `cells[i]` and `THIS.t`.
pub(crate) fn root<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    path: PathExpr<'db>,
) -> Option<VariableDecl<'db>> {
    let step = path.flatten(db).first().copied()?;
    body.variable_for_path_expr(step.get_expr(db))
}

pub(crate) fn access_root<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    access: VariableAccess<'db>,
) -> Option<VariableDecl<'db>> {
    match access.kind(db) {
        VariableAccessKind::Symbolic(begin) => root(db, body, begin.expr(db)?),
        VariableAccessKind::Direct(_) => None,
    }
}
