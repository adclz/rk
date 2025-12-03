use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_inheritance::check_inheritance, check_semantic_index::Check, errors::{analysis_error::ToIdeDiagnostic}
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
        body_inference::{BodyInferenceResult, infer_body_scope},
        ty::Type,
    },
};

impl<'db> Check<'db> for ScopeId<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        let scope = get_scope(db, *self);

        // check usings
        scope.usings.iter().for_each(|u| {
            u.check(db, errors);
        });

        // check pous
        self.pous(db).map(|pous| {
            pous.iter().for_each(|pou| {
                pou.scope_id(db).check(db, errors);
            });
        });

        // check methods
        self.methods(db).map(|methods| {
            methods.check(db, errors); 
        });
        if let ScopeKind::Pou(pou) = get_scope(db, *self).kind {
            check_inheritance(db, pou, errors);
        }

        // check variables
        self.variables(db).map(|variables| {
            variables.check(db, errors);
        });

        // check body
        let infer = infer_body_scope(db, *self);
        for error in infer.errors.iter() {
            errors.push(error.to_diagnostic(db));
        }
    }
}
