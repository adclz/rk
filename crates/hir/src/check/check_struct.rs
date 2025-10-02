use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_semantic_index::Check,
        check_ty::check_ty,
        errors::{analysis_error::AnalysisError, duplicates::DuplicateError},
    },
    hir_def::expressions::spec::Struct,
    hir_ty::ty::ty_for_struct_field,
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
