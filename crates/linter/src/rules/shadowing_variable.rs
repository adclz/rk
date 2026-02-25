use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::body::infer_body,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

/// W0102: variable name shadows a POU (function, function block, class, etc.)
struct ShadowingVariable;

impl ErrorCode for ShadowingVariable {
    fn code(&self) -> &'static str {
        "W0102"
    }

    fn description(&self) -> &'static str {
        "name shadowing"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // Only scopes with bodies can have shadowing
    let has_body = matches!(
        get_scope(db, scope).kind,
        ScopeKind::Pou(Pou::Function(_))
            | ScopeKind::Pou(Pou::FunctionBlock(_))
            | ScopeKind::MethodDecl(_)
            | ScopeKind::Program(_)
    );
    if !has_body {
        return;
    }

    let body = infer_body(db, scope);

    for (var, pou) in &body.variables_shadowing {
        let var_name = var.get_name_ident(db).text(db);
        let pou_kind = match pou {
            Pou::Function(_) => "function",
            Pou::FunctionBlock(_) => "function block",
            Pou::Class(_) => "class",
            Pou::Interface(_) => "interface",
            Pou::DataType(_) => "data type",
        };

        diagnostics.push(
            diag()
                .message(format!(
                    "variable '{var_name}' shadows {pou_kind} '{var_name}'"
                ))
                .severity(DiagnosticSeverity::INFORMATION)
                .desc(&ShadowingVariable)
                .range(var.get_span(db))
                .call(),
        );
    }
}
