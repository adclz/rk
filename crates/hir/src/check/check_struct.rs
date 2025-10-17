use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_init_expr::check_init_expr, check_semantic_index::{Check, DataTypeCheck}, errors::{analysis_error::AnalysisError, duplicates::DuplicateError, init_expr}
    },
    hir_def::{expressions::spec::{Struct, StructElement}, interned::identifier::Ident},
    hir_ty::{init_expr_resolver::resolve_init_expr, ty::Ty},
};

impl<'db> DataTypeCheck<'db> for Struct<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        let mut seen: FxHashMap<Ident, StructElement> = FxHashMap::default();
        for field in &self.elements {
            match seen.get(field.name(db)) {
                Some(prev) => {
                    errors.push(
                        DuplicateError::StructField {
                            field1: *field,
                            field2: *prev
                        }
                        .into(),
                    );
                }
                None => {
                    seen.insert(*field.name(db), *field);
                }
            }

            let field_ty = field.spec(db).spec_to_ty(db);
            if let Some(init_expr) = field.init(db) {
                check_init_expr(db, field_ty, *resolve_init_expr(db, field_ty, init_expr), errors);
            }
        }
    }
}
