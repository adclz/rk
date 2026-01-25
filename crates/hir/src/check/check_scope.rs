use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HirNodeInfo,
    check::{
        check_inheritance::check_inheritance,
        check_semantic_index::{Check, DataTypeCheck},
    },
    hir_def::{
        expressions::spec::SpecKind,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body_inference::infer_body_scope, signature::infer_signature},
};

impl<'db> Check<'db> for ScopeId<'db> {
    fn check(&self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        let scope = get_scope(db, *self);

        if let ScopeKind::Pou(pou) = get_scope(db, *self).kind {
            check_inheritance(db, pou, errors);
        }

        // check usings
        scope.usings.iter().for_each(|u| {
            u.check(db, errors);
        });

        let signature = infer_signature(db, *self);
        for error in signature.errors.iter() {
            errors.push(error.clone());
        }

        // check pous
        if let Some(pous) = self.pous(db) {
            pous.iter().for_each(|pou| {
                pou.get_scope_id(db).check(db, errors);
            });
        }

        if let Some(methods) = self.method_prototypes(db) {
            methods.check(db, errors);
        }

        // check methods
        if let Some(methods) = self.method_declarations(db) {
            methods.check(db, errors);
        }

        // check body
        let infer = infer_body_scope(db, *self);
        for error in infer.errors.iter() {
            errors.push(error.clone());
        }
    }
}
