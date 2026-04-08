use auto_lsp::core::span::Span;
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "self-shadowing";

/// L0315: a variable has the same name as the POU or method it is declared in.
struct SelfShadowing;

impl ErrorCode for SelfShadowing {
    fn code(&self) -> &'static str {
        "L0315"
    }

    fn description(&self) -> &'static str {
        "variable shadows its own POU"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let scope_data = get_scope(db, scope);

    // Skip FUNCTION — the function name is the return variable in IEC 61131-3.
    let file = scope.file(db);

    match scope_data.kind {
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => {
            check_variables(
                db,
                fb.name(db).text(db),
                "FUNCTION_BLOCK",
                fb.get_name_span(db),
                fb.variables(db),
                file,
                diagnostics,
            );
        }
        ScopeKind::Pou(Pou::Class(cl)) => {
            check_variables(
                db,
                cl.name(db).text(db),
                "CLASS",
                cl.get_name_span(db),
                cl.variables(db),
                file,
                diagnostics,
            );
        }
        ScopeKind::MethodDecl(m) => {
            check_variables(
                db,
                m.name(db).text(db),
                "METHOD",
                m.get_name_span(db),
                m.variables(db),
                file,
                diagnostics,
            );
        }
        ScopeKind::Program(p) => {
            check_variables(
                db,
                p.name(db).text(db),
                "PROGRAM",
                p.get_name_span(db),
                p.variables(db),
                file,
                diagnostics,
            );
        }
        _ => {}
    }
}

fn check_variables<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou_name: &str,
    pou_kind: &str,
    pou_name_span: Span,
    variables: &[VariableDecl<'db>],
    file: File,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for var in variables {
        if var.name(db).text(db) == pou_name {
            let mut d = diag()
                .message(format!(
                    "variable '{pou_name}' has the same name as its declaring {pou_kind}"
                ))
                .desc(&SelfShadowing)
                .range(var.get_name_span(db))
                .severity(DiagnosticSeverity::WARNING)
                .call();

            d.with_related(Related::new(
                format!("{pou_kind} '{pou_name}' is declared here"),
                file,
                pou_name_span,
            ));

            diagnostics.push(d);
        }
    }
}
