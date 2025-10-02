use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    TypeInfo,
    check::{
        errors::{
            analysis_error::DiagnosticDescription,
            utils::get_candidates,
        },
        recovery::pou::fuzzy_pou_local_items,
    },
    hir_def::{
        expressions::expression::PathExpr,
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    hir_ty::ty::Ty,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PathResolveError<'db> {
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: FileScopeId<'db>,
    },
    UnknownField {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    UnexpectedIndex {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    NotAnArray {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    NotAReference {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
}

impl<'db> DiagnosticDescription<'db> for PathResolveError<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            PathResolveError::NoItemInScope { expr, scope } => {
                format!("no item '{}' in scope", expr.to_string(db).text(db))
            }
            PathResolveError::UnknownField { ty, expr } => {
                format!("field '{}' not found", expr.to_string(db).text(db))
            }
            PathResolveError::UnexpectedIndex { ty, expr } => {
                format!("'{}' cannot be indexed", ty.type_name(db))
            }
            PathResolveError::NotAReference { ty, expr } => {
                format!("'{}' is not a reference", ty.type_name(db))
            }
            PathResolveError::NotAnArray { ty, expr } => {
                format!("'{}' is not an array", ty.type_name(db))
            }
        }
    }

    fn note(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        if let PathResolveError::NoItemInScope { expr, scope } = self {
            if let ScopeKind::Pou(pou) = semantic_index(db, scope.file(db))
                .get_scope(db, *scope)
                .kind
            {
                diag.with_note(get_candidates(&fuzzy_pou_local_items(
                    db,
                    pou,
                    expr.to_string(db).as_str(db),
                )));
            }
        }
    }
}
