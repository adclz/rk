use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        pous::{pou::Pou, pragma::Pragma},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "invalid-pragma";

struct InvalidPragma;

impl ErrorCode for InvalidPragma {
    fn code(&self) -> &'static str {
        "L0003"
    }

    fn description(&self) -> &'static str {
        "invalid pragma for this POU"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let scope_data = get_scope(db, scope);
    let kind = &scope_data.kind;

    let pragmas = match kind {
        ScopeKind::Pou(Pou::Function(f)) => f.pragmas(db),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => fb.pragmas(db),
        ScopeKind::MethodDecl(m) => m.pragmas(db),
        ScopeKind::Program(p) => p.pragmas(db),
        _ => return,
    };

    for pragma in pragmas {
        let (invalid, reason) = match pragma {
            Pragma::Test(_) => match kind {
                ScopeKind::Pou(Pou::Function(_)) => (false, ""),
                ScopeKind::Pou(Pou::FunctionBlock(_)) => {
                    (true, "{test} is not valid on FUNCTION_BLOCK")
                }
                ScopeKind::MethodDecl(_) => (true, "{test} is not valid on METHOD"),
                ScopeKind::Program(_) => (true, "{test} is not valid on PROGRAM"),
                _ => (false, ""),
            },
            Pragma::Once(_) => match kind {
                ScopeKind::Program(_) => (true, "{once} is not valid on PROGRAM"),
                _ => (false, ""),
            },
            Pragma::Warn(_, _) => (false, ""),
            // Position legality for {extern} is a compiler ERROR (E0244),
            // not lint advice — a misplaced import must not merely warn.
            Pragma::Extern(_, _) => (false, ""),
            // {allow} is valid on every POU kind; its NAMES are what gets
            // checked, by the unknown-allow rule.
            Pragma::Allow(_, _) => (false, ""),
        };

        if invalid {
            diagnostics.push(
                diag()
                    .message(reason.to_string())
                    .desc(&InvalidPragma)
                    .range(
                        hir::denormalize(
                            db,
                            pragma.span_ident().get_scope_id(db).file(db),
                            &pragma.span_ident().get_span(db),
                        )
                        .unwrap_or_default(),
                    )
                    .severity(DiagnosticSeverity::WARNING)
                    .call(),
            );
        }
    }
}
