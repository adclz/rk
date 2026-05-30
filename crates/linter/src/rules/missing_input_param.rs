use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{expressions::expression::FuncCall, pous::variable::VariableKind},
    hir_ty::{
        body::BodyInferenceResult,
        ty::{CallableType, Type},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "missing-input-param";

/// L0204: a FUNCTION_BLOCK or PROGRAM call does not pass every declared
/// VAR_INPUT. Per other toolchains this is *not* a hard error — the FB/PROGRAM
/// instance retains the previous value (or compiler-initialised default).
/// We surface it as a lint so the user is notified that not all inputs
/// were wired. FUNCTION/METHOD callsites are covered by `E0233` instead,
/// so this lint deliberately skips them to avoid overlap.
struct MissingInputParam;

impl ErrorCode for MissingInputParam {
    fn code(&self) -> &'static str {
        "L0204"
    }

    fn description(&self) -> &'static str {
        "missing input parameter"
    }
}

pub fn check_func_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    stmt: hir::hir_def::expressions::statement::Stmt<'db>,
    func_call: FuncCall<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let typ = body.type_of_begin_expr_with_adjustments(db, func_call.path(db));
    let callable = match typ {
        Type::CallableType(ct) => ct,
        _ => match typ.as_callable(db) {
            Some(ct) => ct,
            None => return,
        },
    };

    // FUNCTION and METHOD calls are covered by the hard-error E0233 path.
    // Linting them would duplicate that diagnostic.
    if !matches!(callable, CallableType::FunctionBlock(_)) {
        return;
    }

    let has_variadic = callable
        .def_map(db)
        .local_variables
        .values()
        .any(|v| v.variadic(db));
    if has_variadic {
        return;
    }

    let matched_vars: rustc_hash::FxHashSet<_> = func_call
        .params(db)
        .iter()
        .filter_map(|param| body.variable_of_param.get(param).copied())
        .collect();

    let missing: Vec<_> = callable
        .def_map(db)
        .local_variables
        .values()
        .filter(|var| var.kind(db) == VariableKind::Input && !matched_vars.contains(var))
        .copied()
        .collect();

    if missing.is_empty() {
        return;
    }

    let callable_name = callable.get_name_ident(db).text(db);
    let names: Vec<_> = missing
        .iter()
        .map(|v| format!("'{}'", v.name(db).text(db)))
        .collect();

    let mut d = diag()
        .message(format!(
            "call to '{}' is missing {} input parameter{}: {}",
            callable_name,
            missing.len(),
            if missing.len() > 1 { "s" } else { "" },
            names.join(", "),
        ))
        .desc(&MissingInputParam)
        .range(
            hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::HINT)
        .call();

    let file = callable.get_scope_id(db).file(db);
    for var in &missing {
        d.with_related(Related::new(
            format!("'{}' declared here", var.name(db).text(db)),
            file,
            var.get_span(db),
        ));
    }

    diagnostics.push(d);
}
