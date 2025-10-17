use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        errors::{
            analysis_error::DiagnosticDescription,
            utils::{get_candidates, get_def_for_ty},
        },
        recovery::pou::fuzzy_pou_local_items,
    }, hir_def::{
        expressions::expression::PathExpr,
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
    }, hir_ty::{ty::Ty, ty_var_access_resolver::ResolvedAccess, walk::ResolvedPath}, TypeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AccessError<'db> {
    NoItemInScope {
        expr: PathExpr<'db>,
    },
    InvalidTypeAccess {
        access: ResolvedPath<'db>,
    },
    UnknownField {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    TypeHasNoField {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    MissingDeref {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    NotAnArray {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
    NotAReference {
        ty: ResolvedPath<'db>,
        expr: PathExpr<'db>,
    },
}

impl<'db> DiagnosticDescription<'db> for AccessError<'db> {
    fn description(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            AccessError::NoItemInScope { expr } => {
                format!("no item '{}' in scope", expr.ident(db).text(db))
            }
            AccessError::InvalidTypeAccess { access } => {
                format!("invalid type access on '{}'", access.decl_name(db))
            }
            AccessError::UnknownField { ty, expr } => {
                format!("field '{}' not found in '{}'", expr.ident(db).text(db), ty.decl_name(db))
            }
            AccessError::TypeHasNoField { ty, expr } => {
                format!("type '{}' does not have fields", ty.decl_name(db))
            }
            AccessError::MissingDeref { ty, expr } => {
                format!("type '{}' is a reference, maybe you forgot to dereference it ?", ty.decl_name(db))
            }
            AccessError::NotAReference { ty, expr } => {
                format!("type '{}' can not be dereferenced", ty.decl_name(db))
            }
            AccessError::NotAnArray { ty, expr } => {
                format!("type '{}' cannot be indexed", ty.decl_name(db))
            }
        }
    }

    fn note(&self, db: &'db dyn BaseDatabase, diag: &mut IdeDiagnostic) {
        match self {
            AccessError::NoItemInScope { expr } => {
                let scope = expr.scope_id(db);
                let sema = semantic_index(db, expr.scope_id(db).file(db));
                let scope = sema.get_scope(db, scope);
                if let ScopeKind::Pou(pou) = scope
                    .kind
                {
                    diag.with_note(get_candidates(&fuzzy_pou_local_items(
                        db,
                        pou,
                        expr.ident(db).as_str(db),
                    )));
                }
            }
            AccessError::TypeHasNoField { ty, expr } => {
                //get_def_for_ty(db, *ty, diag);
            }
            AccessError::MissingDeref { ty, expr } => {
                //get_def_for_ty(db, *ty, diag);
            }
            AccessError::UnknownField { ty, expr } => {
                //get_def_for_ty(db, *ty, diag);
            }
            AccessError::NotAnArray { ty, expr } => {
                //get_def_for_ty(db, *ty, diag);
            }
            AccessError::NotAReference { ty, expr } => {
                //get_def_for_ty(db, *ty, diag);
            },
            _ => { /* No note for other errors */ }
        }
    }
}
