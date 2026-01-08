use ast::generated::RefSpec;
use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    HirNodeInfo,
    check::{
        check_inheritance::check_inheritance,
        check_semantic_index::{Check, DataTypeCheck},
        errors::analysis_error::ToIdeDiagnostic,
    },
    hir_def::{
        expressions::{
            expression::{Expr, VariableAccess},
            spec::{ElementarySpec, SpecKind},
            statement::{Stmt, StmtKind},
        },
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body_inference::{BodyInferenceResult, infer_body_scope},
        init_inference::infer_data_type,
        ty::Type,
    },
};

impl<'db> Check<'db> for ScopeId<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        let scope = get_scope(db, *self);

        match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => {
                check_inheritance(db, pou, errors);
                if let Pou::DataType(dt) = pou {
                    match dt.spec(db).kind(db) {
                        SpecKind::Array(arr) => arr.check(db, errors),
                        SpecKind::Enum(enu) => enu.check(db, errors),
                        SpecKind::Struct(struc) => struc.check(db, errors),
                        SpecKind::Subrange(sub) => sub.check(db, errors),
                        _ => {}
                    }
                    if let Some(init) = dt.init(db) {
                        let result = infer_data_type(db, dt);
                        for error in result.errors.iter() {
                            errors.push(error.clone());
                        }

                        for error in result.body_infer_result.errors.iter() {
                            errors.push(error.clone());
                        }
                    }
                }
            }
            _ => {}
        }

        // check usings
        scope.usings.iter().for_each(|u| {
            u.check(db, errors);
        });

        // check pous
        self.pous(db).map(|pous| {
            pous.iter().for_each(|pou| {
                pou.get_scope_id(db).check(db, errors);
            });
        });

        self.method_prototypes(db).map(|methods| {
            methods.check(db, errors);
        });

        // check methods
        self.method_declarations(db).map(|methods| {
            methods.check(db, errors);
        });

        // check variables
        self.variables(db).map(|variables| {
            variables.check(db, errors);
        });

        // check body
        let infer = infer_body_scope(db, *self);
        for error in infer.errors.iter() {
            errors.push(error.clone());
        }
    }
}
