use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
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

/// L0205: function or method has an empty body.
struct EmptyBody;

impl ErrorCode for EmptyBody {
    fn code(&self) -> &'static str {
        "L0205"
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
            (f.statements(db), f.name(db).text(db), "FUNCTION", f.get_span(db))
        }
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => {
            (fb.statements(db), fb.name(db).text(db), "FUNCTION_BLOCK", fb.get_span(db))
        }
        ScopeKind::MethodDecl(m) => {
            (m.stmts(db), m.name(db).text(db), "METHOD", m.get_span(db))
        }
        ScopeKind::Program(p) => {
            (p.statements(db), p.name(db).text(db), "PROGRAM", p.get_span(db))
        }
        _ => return,
    };

    if statements.is_empty() {
        diagnostics.push(
            diag()
                .message(format!("{kind_str} '{name}' has an empty body"))
                .desc(&EmptyBody)
                .range(span)
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}
