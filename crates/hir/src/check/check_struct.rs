use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_init_expr::check_init_expr,
        check_semantic_index::Check,
        check_ty::check_ty,
        errors::{duplicates::DuplicateError, sem_errors::AnalysisError},
    },
    hir_def::{expressions::spec::Struct, pous::variable::VariableDecl},
    hir_ty::{
        init_expr_resolver::resolve_init_expr,
        ty::{ty_for_struct_field, ty_for_variable},
    },
};

impl<'db> Check<'db> for Struct<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        let mut seen = FxHashMap::default();
        for field in &self.elements {
            match seen.get(field.name(db)) {
                Some(prev) => {
                    errors.push(
                        DuplicateError::StructField {
                            field1: ty_for_struct_field(db, *field),
                            field2: ty_for_struct_field(db, *prev),
                        }
                        .into(),
                    );
                }
                None => {
                    seen.insert(field.name(db), *field);
                }
            }

            let field_ty = ty_for_struct_field(db, *field);
            check_ty(db, field_ty, errors);
        }
    }
}
