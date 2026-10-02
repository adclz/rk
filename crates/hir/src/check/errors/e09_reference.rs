// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    /// A reference a warm start would restore: a RETAIN variable, or one a
    /// `PROGRAM RETAIN` instance keeps, holding a REF_TO or an interface
    /// value, itself or in a field or member. What it restores is where its
    /// target was in the build that saved it, and a new build may have moved
    /// that target.
    RetainedReference {
        var: VariableDecl<'db>,
        /// The fields and members from `var` to the reference, as written.
        route: Vec<crate::hir_def::interned::identifier::Ident>,
        /// The `PROGRAM RETAIN` instance keeping `var`, when its own
        /// declaration is not RETAIN.
        instance: Option<crate::hir_def::interned::identifier::SpanIdent<'db>>,
    },
}

impl<'db> ErrorCode for ReferenceError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::DerefNonRefType { .. } => "E0901",
            Self::DerefPossiblyNull { .. } => "E0902",
            Self::ReturnsReferenceToLocal { .. } => "E0903",
            Self::RetainedReference { .. } => "E0904",
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
                let name = var.get_name_with_case(db).text(db);
                // The related span may belong to ANOTHER variable: a state
                // propagates through `ptr := ptr_2`, so name whichever
                // variable the span actually declares or assigns.
                let (message, related_msg, site) = match state {
                    NullState::Uninitialized(origin) => {
                        let from = origin.var.get_name_with_case(db).text(db);
                        (
                            format!("'{name}' is dereferenced and never set"),
                            format!("'{from}' is declared without an initial value here"),
                            &origin.site,
                        )
                    }
                    NullState::Null(origin) => {
                        let from = origin.var.get_name_with_case(db).text(db);
                        (
                            format!("'{name}' is dereferenced and is NULL"),
                            format!("'{from}' is set to NULL here"),
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
                        var.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "'{}' is per-call storage, declared here",
                        var.name_with_case(db).text(db),
                    ),
                    var.get_scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                diag.with_help(
                    "return a reference to instance state, or to storage the caller owns (a VAR_IN_OUT)"
                        .into(),
                );
                diag
            }
            Self::RetainedReference {
                var,
                route,
                instance,
            } => {
                let held = std::iter::once(var.name_with_case(db))
                    .chain(route.iter().copied())
                    .map(|name| name.text(db).to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                let (message, range) = match instance {
                    Some(instance) => (
                        format!(
                            "PROGRAM RETAIN '{}' keeps '{held}', a reference: its address does not survive a new build",
                            instance.with_case.text(db)
                        ),
                        instance.get_span(db),
                    ),
                    None => (
                        format!(
                            "'{held}' is a reference in RETAIN storage: its address does not survive a new build"
                        ),
                        var.get_name_span(db),
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &range).unwrap_or_default())
                    .call();
                if instance.is_some() {
                    diag.with_related(Related::new(
                        format!("'{}' is declared here", var.name_with_case(db).text(db)),
                        var.get_scope_id(db).file(db),
                        var.get_name_span(db),
                    ));
                }
                diag.with_note(
                    "a warm start restores the address even where a new build moved its target"
                        .into(),
                );
                diag.with_help(
                    "keep references out of RETAIN (NON_RETAIN on a member) and set them in the first scan"
                        .into(),
                );
                diag
            }
        }
    }
}
