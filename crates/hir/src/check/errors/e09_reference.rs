use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::PathExpr;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_ty::body::NullState;
use crate::hir_ty::ty::Type;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ReferenceError<'db> {
    DerefNonRefType {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    DerefPossiblyNull {
        var: VariableDecl<'db>,
        expr: PathExpr<'db>,
        state: NullState<'db>,
    },
    /// A reference to the POU's own per-call storage, handed back to the
    /// caller. The storage is reused by the next invocation, so the reference
    /// silently reads whatever that call leaves behind — it does not fault,
    /// which is what makes it worth refusing at compile time.
    ReturnsReferenceToLocal {
        var: VariableDecl<'db>,
        site: CallSite<'db>,
    },
}

impl<'db> ErrorCode for ReferenceError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::DerefNonRefType { .. } => "E0901",
            Self::DerefPossiblyNull { .. } => "E0902",
            Self::ReturnsReferenceToLocal { .. } => "E0903",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::DerefNonRefType { .. } => "invalid operation",
            Self::DerefPossiblyNull { .. } => "possibly null dereference",
            Self::ReturnsReferenceToLocal { .. } => "reference outlives its storage",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for ReferenceError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::DerefNonRefType { expr, ty } => diag()
                .message(format!(
                    "cannot dereference non-reference type '{}'",
                    ty.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::DerefPossiblyNull { var, expr, state } => {
                let name = var.get_name_ident(db).text(db);
                // The related span may belong to ANOTHER variable: a state
                // propagates through `ptr := ptr_2`, so name whichever
                // variable the span actually declares or assigns.
                let (message, related_msg, site) = match state {
                    NullState::Uninitialized(origin) => {
                        let from = origin.var.get_name_ident(db).text(db);
                        (
                            format!("dereference of reference '{name}' which is never initialized"),
                            format!("'{from}' declared without initializer here"),
                            &origin.site,
                        )
                    }
                    NullState::Null(origin) => {
                        let from = origin.var.get_name_ident(db).text(db);
                        (
                            format!("dereference of reference '{name}' which is null"),
                            format!("'{from}' set to NULL here"),
                            &origin.site,
                        )
                    }
                    _ => unreachable!(),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    related_msg,
                    site.scope.file(db),
                    site.get_span(db),
                ));
                diag
            }
            Self::ReturnsReferenceToLocal { var, site } => {
                let mut diag = diag()
                    .message(format!(
                        "reference to '{}' outlives the call that owns it",
                        var.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "'{}' is per-call storage, declared here",
                        var.name(db).text(db),
                    ),
                    var.get_scope_id(db).file(db),
                    var.get_span(db),
                ));
                diag.with_note(
                    "return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)"
                        .into(),
                );
                diag
            }
        }
    }
}
