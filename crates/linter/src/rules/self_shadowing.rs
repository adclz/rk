// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter;
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

/// L0115: a variable has the same name as the POU or method it is declared in.
struct SelfShadowing;

impl ErrorCode for SelfShadowing {
    fn code(&self) -> &'static str {
        "L0115"
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
                &fb,
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
                &cl,
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
                &m,
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
                &p,
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
    pou: &dyn HasName<'db>,
    pou_kind: &str,
    pou_name_span: tree_sitter::Range,
    variables: &[VariableDecl<'db>],
    file: File,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let pou_ident = pou.get_name_ident(db);
    for var in variables {
        if var.name(db) == pou_ident {
            let mut d = diag()
                .message(format!(
                    "variable '{}' has the same name as its declaring {pou_kind}",
                    var.name_with_case(db).text(db)
                ))
                .desc(&SelfShadowing)
                .range(
                    hir::denormalize(db, var.get_scope_id(db).file(db), &var.get_name_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::WARNING)
                .call();

            d.with_related(Related::new(
                format!(
                    "{pou_kind} '{}' is declared here",
                    pou.get_name_with_case(db).text(db)
                ),
                file,
                pou_name_span,
            ));

            diagnostics.push(d);
        }
    }
}
