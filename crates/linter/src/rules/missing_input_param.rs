use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{expressions::expression::FuncCall, pous::variable::VariableKind},
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "missing-input-param";

/// L0204: a function call does not pass all required VAR_INPUT parameters.
struct MissingInputParam;

impl ErrorCode for MissingInputParam {
    fn code(&self) -> &'static str {
        "L0204"
    }

    fn description(&self) -> &'static str {
        "missing input parameter"
    }
}

/// Check a FuncCall for missing VAR_INPUT parameters.
/// Called by the unified visitor.
pub fn check_func_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    stmt: hir::hir_def::expressions::statement::Stmt<'db>,
    func_call: FuncCall<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // Resolve the callable type
    let typ = body.type_of_begin_expr_with_adjustments(db, func_call.path(db));
    let callable = match typ {
        Type::CallableType(ct) => ct,
        _ => match typ.as_callable(db) {
            Some(ct) => ct,
            None => return,
        },
    };

    // Skip variadic functions - they have flexible parameter counts
    let has_variadic = callable
        .def_map(db)
        .local_variables
        .values()
        .any(|v| v.variadic(db));
    if has_variadic {
        return;
    }

    // Collect the set of variables that were matched by the call's params
    let matched_vars: rustc_hash::FxHashSet<_> = func_call
        .params(db)
        .iter()
        .filter_map(|param| body.variable_of_param.get(param).copied())
        .collect();

    // Find missing VAR_INPUT parameters
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
    let missing_names: Vec<_> = missing
        .iter()
        .map(|v| v.name(db).text(db).to_string())
        .collect();

    let mut d = diag()
        .message(format!(
            "call to '{}' is missing {} input parameter{}: {}",
            callable_name,
            missing.len(),
            if missing.len() > 1 { "s" } else { "" },
            missing_names.join(", "),
        ))
        .desc(&MissingInputParam)
        .range(stmt.get_span(db))
        .severity(DiagnosticSeverity::HINT)
        .call();

    // Add related info pointing to the declaration of each missing param
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
