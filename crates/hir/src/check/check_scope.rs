use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_semantic_index::Check,
        errors::{analysis_error::ToIdeDiagnostic, stmt::StmtError},
    },
    hir_def::{
        expressions::{
            expression::{Expr, VariableAccess},
            spec::ElementarySpec,
            statement::{Stmt, StmtKind},
        },
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        inference::{InferenceResult, infer_scope},
        ty2::Type,
    },
};

impl<'db> Check<'db> for ScopeId<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        let infer = infer_scope(db, *self);

        for error in infer.errors.iter() {
            errors.push(error.to_diagnostic(db));
        }
    }
}