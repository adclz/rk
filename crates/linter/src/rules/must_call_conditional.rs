// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HasPragmas, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{ExprKind, FuncCall, PrimaryExpr, RefValue},
            statement::{Stmt, StmtKind},
        },
        pous::{function_block::FunctionBlock, pou::Pou, variable::VariableDecl},
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
use rustc_hash::{FxHashMap, FxHashSet};

use super::must_call_violation::{access_root, instances, root};

pub const NAME: &str = "must-call-conditional";

/// L0007: an instance of a `{must_call}` FUNCTION_BLOCK called only inside a
/// branch or a loop, so not at every scan.
struct MustCallConditional;

impl ErrorCode for MustCallConditional {
    fn code(&self) -> &'static str {
        "L0007"
    }
}

/// The statement a call sits in, outermost first: none at the top of the
/// body.
#[derive(Clone, Copy, PartialEq)]
enum Within {
    If,
    Case,
    Loop,
}

impl Within {
    fn name(self) -> &'static str {
        match self {
            Within::If => "an IF",
            Within::Case => "a CASE",
            Within::Loop => "a loop",
        }
    }

    fn keyword(self) -> &'static str {
        match self {
            Within::If => "the IF",
            Within::Case => "the CASE",
            Within::Loop => "the loop",
        }
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let statements = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => f.statements(db),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => fb.statements(db),
        ScopeKind::MethodDecl(m) => m.stmts(db),
        ScopeKind::Program(program) => program.statements(db),
        _ => return,
    };
    // One instance, not an array: a FOR over an array calls each element.
    let instances: Vec<(VariableDecl<'db>, FunctionBlock<'db>)> = instances(db, scope)
        .into_iter()
        .filter(|(var, _)| matches!(var.spec(db).infer(db).normalize(db), Type::FunctionBlock(_)))
        .collect();
    if instances.is_empty() {
        return;
    }
    let body = scope.inference(db);

    // An instance that runs elsewhere, at times this body does not show: in
    // a method, handed on, called from outside, named by the configuration.
    let mut elsewhere: FxHashSet<VariableDecl<'db>> = handed_on(db, body);
    if let Some(methods) = scope.method_declarations(db) {
        for method in methods.iter() {
            let method_body = method.get_scope_id(db).inference(db);
            elsewhere.extend(handed_on(db, method_body));
            for (call, resolved) in method_body.resolved_calls() {
                if matches!(resolved.callable, CallableType::FunctionBlock(_))
                    && let Some(root) = call
                        .path(db)
                        .expr(db)
                        .and_then(|p| root(db, method_body, p))
                {
                    elsewhere.insert(root);
                }
            }
        }
    }
    for config in hir::hir_ty::index_graphs::declared_configs(db) {
        elsewhere.extend(config.scope_id(db).inference(db).variables_used());
    }

    // Each call of the body, with the statement it sits in.
    let mut calls: FxHashMap<VariableDecl<'db>, Vec<(FuncCall<'db>, Option<Within>)>> =
        FxHashMap::default();
    walk(db, body, statements, None, &mut calls);

    for (var, block) in instances {
        if elsewhere.contains(&var) || hir::hir_ty::calls::reached_through_a_path(db, var) {
            continue;
        }
        let Some(sites) = calls.get(&var) else {
            continue;
        };
        if sites.iter().any(|(_, within)| within.is_none()) {
            continue;
        }
        let Some((first, Some(within))) = sites.first().copied() else {
            continue;
        };
        diagnostics.push(report(db, var, block, first, within));
    }
}

fn report<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    block: FunctionBlock<'db>,
    call: FuncCall<'db>,
    within: Within,
) -> IdeDiagnostic {
    let name = var.name_with_case(db).text(db);
    let block_name = block.get_name_with_case(db).text(db);
    let site = call.path(db);
    let mut d = diag()
        .message(format!("'{name}' is called only inside {}", within.name()))
        .desc(&MustCallConditional)
        .range(
            hir::denormalize(db, site.get_scope_id(db).file(db), &site.get_span(db))
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
    d.with_note(format!(
        "a scan that skips the call leaves '{name}' as it was"
    ));
    d.with_help(format!(
        "call '{name}' at every scan, outside {}, and pass the condition as an input",
        within.keyword()
    ));
    d
}

/// The instances a body hands on by reference: to a VAR_IN_OUT, an output,
/// `REF()`. Their holder may call them.
fn handed_on<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
) -> FxHashSet<VariableDecl<'db>> {
    let mut out = FxHashSet::default();
    for (_, resolved) in body.resolved_calls() {
        for (param, binding) in &resolved.params {
            match binding {
                ParamBinding::Values(values) if param.is_in_out(db) => {
                    for value in values {
                        if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) =
                            value.expr(db)
                            && let Some(root) = access_root(db, body, *access)
                        {
                            out.insert(root);
                        }
                    }
                }
                ParamBinding::Output { variable, .. } => {
                    if let Some(root) = access_root(db, body, *variable) {
                        out.insert(root);
                    }
                }
                _ => {}
            }
        }
    }
    for (expr, _) in body.typed_exprs() {
        if let ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
            value: RefValue::Address(begin),
        }) = expr.expr(db)
            && let Some(root) = begin.expr(db).and_then(|p| root(db, body, p))
        {
            out.insert(root);
        }
    }
    out
}

/// The body calls of `stmts`, by the instance they run, each with the
/// outermost statement it sits in.
fn walk<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmts: &[Stmt<'db>],
    within: Option<Within>,
    calls: &mut FxHashMap<VariableDecl<'db>, Vec<(FuncCall<'db>, Option<Within>)>>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::FuncCall(call) => {
                if body.resolved_call(*call).is_some_and(|resolved| {
                    matches!(resolved.callable, CallableType::FunctionBlock(_))
                }) && let Some(root) = call.path(db).expr(db).and_then(|p| root(db, body, p))
                {
                    calls.entry(root).or_default().push((*call, within));
                }
            }
            StmtKind::If {
                then,
                else_if,
                else_,
                ..
            } => {
                let inner = within.or(Some(Within::If));
                walk(db, body, then.as_deref().unwrap_or_default(), inner, calls);
                for (_, stmts) in else_if {
                    walk(db, body, stmts, inner, calls);
                }
                walk(db, body, else_.as_deref().unwrap_or_default(), inner, calls);
            }
            StmtKind::Case { cases, else_, .. } => {
                let inner = within.or(Some(Within::Case));
                for (_, stmts) in cases {
                    walk(db, body, stmts, inner, calls);
                }
                walk(db, body, else_.as_deref().unwrap_or_default(), inner, calls);
            }
            StmtKind::For { body: stmts, .. }
            | StmtKind::While { body: stmts, .. }
            | StmtKind::Repeat { body: stmts, .. } => {
                walk(db, body, stmts, within.or(Some(Within::Loop)), calls)
            }
            _ => {}
        }
    }
}
