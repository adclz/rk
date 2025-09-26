use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::{
        errors::{
            analysis_error::{AnalysisError, ToIdeDiagnostic},
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