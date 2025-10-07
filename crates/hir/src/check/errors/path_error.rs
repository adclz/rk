use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    TypeInfo,
    check::{
        errors::{
            analysis_error::DiagnosticDescription,
            utils::{get_candidates, get_decl_and_def_for_ty},
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
    NoField {
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
                format!("no item '{}' in scope", expr.ident(db).text(db))
            }
            PathResolveError::UnknownField { ty, expr } => {
                format!("field '{}' not found", expr.ident(db).text(db))
            }
            PathResolveError::NoField { ty, expr } => {
                format!("type '{}' does not have fields", ty.type_name(db))
            }
            PathResolveError::NotAReference { ty, expr } => {
                format!("type '{}' is not a reference", ty.type_name(db))
            }
            PathResolveError::NotAnArray { ty, expr } => {
                format!("type '{}' cannot be indexed", ty.type_name(db))
            }
        }
    }

    fn note(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        match self {
            PathResolveError::NoItemInScope { expr, scope } => {
                if let ScopeKind::Pou(pou) = semantic_index(db, scope.file(db))
                    .get_scope(db, *scope)
                    .kind
                {
                    diag.with_note(get_candidates(&fuzzy_pou_local_items(
                        db,
                        pou,
                        expr.ident(db).as_str(db),
                    )));
                }
            }
            PathResolveError::NoField { ty, expr } => {
                get_decl_and_def_for_ty(db, *ty, diag);
            }
            PathResolveError::UnknownField { ty, expr } => {
                get_decl_and_def_for_ty(db, *ty, diag);
            }
            PathResolveError::NotAnArray { ty, expr } => {
                get_decl_and_def_for_ty(db, *ty, diag);
            }
            PathResolveError::NotAReference { ty, expr } => {
                get_decl_and_def_for_ty(db, *ty, diag);
            }
        }
    }
}
