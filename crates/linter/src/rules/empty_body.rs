use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::HasPragmas;
use hir::{
    HirNodeInfo,
    hir_def::{
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "empty-body";

/// L0206: function or method has an empty body.
struct EmptyBody;

impl ErrorCode for EmptyBody {
    fn code(&self) -> &'static str {
        "L0206"
    }

    fn description(&self) -> &'static str {
        "empty body"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let scope_data = get_scope(db, scope);

    let (statements, name, kind_str, span) = match &scope_data.kind {
        ScopeKind::Pou(Pou::Function(f)) => {
            // An extern FUNCTION's body is empty BY DEFINITION — the WASM
            // import runs in its place (E0243 refuses statements outright).
            {
                if f.extern_pragma(db).is_some() {
                    return;
                }
            }
            (
                f.statements(db),
                f.name(db).text(db),
                "FUNCTION",
                f.get_span(db),
            )
        }
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => (
            fb.statements(db),
            fb.name(db).text(db),
            "FUNCTION_BLOCK",
            fb.get_span(db),
        ),
        ScopeKind::MethodDecl(m) => (m.stmts(db), m.name(db).text(db), "METHOD", m.get_span(db)),
        ScopeKind::Program(p) => (
            p.statements(db),
            p.name(db).text(db),
            "PROGRAM",
            p.get_span(db),
        ),
        _ => return,
    };

    if statements.is_empty() {
        diagnostics.push(
            diag()
                .message(format!("{kind_str} '{name}' has an empty body"))
                .desc(&EmptyBody)
                .range(hir::denormalize(db, scope.file(db), &span).unwrap_or_default())
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}
