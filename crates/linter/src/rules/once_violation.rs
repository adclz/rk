// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasPragmas, HirNodeInfo,
    hir_def::{expressions::expression::PathExpr, pous::variable::VariableDecl},
    hir_ty::{
        body::ScopeInference,
        oop::MethodRef,
        ty::{CallableType, Type},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashMap;

pub const NAME: &str = "once-violation";

struct OnceViolation;

impl ErrorCode for OnceViolation {
    fn code(&self) -> &'static str {
        "L0004"
    }

    fn description(&self) -> &'static str {
        "calling a {once} function more than once"
    }
}

struct OnceCallInfo<'db> {
    calls: Vec<PathExpr<'db>>,
    callable: CallableType<'db>,
}

/// Check body for multiple calls to the same `{once}` function.
pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // A call is grouped by what it runs, not by how it is written: the
    // callable and the variable each step of its path names, so `inst()` and
    // `INST()` are one instance, `a1.fb()` and `a2.fb()` two.
    let mut once_calls: FxHashMap<(CallableType<'db>, Vec<VariableDecl<'db>>), OnceCallInfo<'db>> =
        FxHashMap::default();

    for (path_expr, typ) in body.typed_path_exprs() {
        let callable = match typ {
            Type::CallableType(ct) => ct,
            _ => continue,
        };

        let has_once = match &callable {
            CallableType::Function(f) => f.is_once(db),
            CallableType::FunctionBlock(fb) => fb.is_once(db),
            CallableType::MethodDecl(m) => match m {
                MethodRef::Declared(d) => d.is_once(db),
                _ => false,
            },
        };

        if !has_once {
            continue;
        }

        let instance = path_expr
            .flatten(db)
            .iter()
            .filter_map(|step| body.variable_for_path_expr(step.get_expr(db)))
            .collect();
        once_calls
            .entry((callable, instance))
            .or_insert_with(|| OnceCallInfo {
                calls: vec![],
                callable,
            })
            .calls
            .push(path_expr);
    }

    for info in once_calls.values() {
        if info.calls.len() < 2 {
            continue;
        }

        // Sort calls by source position so first/second is deterministic
        let mut sorted_calls = info.calls.clone();
        sorted_calls.sort_by_key(|c| c.get_span(db).start_byte);
        // Named as the first call writes it.
        let name = sorted_calls[0].as_call_site(db).to_string(db);

        // Get the {once} pragma span from the callable
        let once_span = match &info.callable {
            CallableType::Function(f) => f.once_pragma(db).map(|s| s.get_span(db)),
            CallableType::FunctionBlock(fb) => fb.once_pragma(db).map(|s| s.get_span(db)),
            CallableType::MethodDecl(m) => match m {
                MethodRef::Declared(d) => d.once_pragma(db).map(|s| s.get_span(db)),
                _ => None,
            },
        };
        let decl_file = info.callable.get_scope_id(db).file(db);

        for call in &sorted_calls[1..] {
            let mut d = diag()
                .message(format!(
                    "'{name}' is marked {{once}} but is called more than once in this body"
                ))
                .desc(&OnceViolation)
                .range(
                    hir::denormalize(db, call.get_scope_id(db).file(db), &call.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::INFORMATION)
                .call();

            d.with_related(Related::new(
                "first call here".to_string(),
                sorted_calls[0].get_scope_id(db).file(db),
                sorted_calls[0].get_span(db),
            ));

            if let Some(pragma_span) = once_span {
                d.with_related(Related::new(
                    "{once} pragma declared here".to_string(),
                    decl_file,
                    pragma_span,
                ));
            }

            diagnostics.push(d);
        }
    }
}
