use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName, check::{
        check_init_expr::check_init_expr,
        check_semantic_index::DataTypeCheck,
        errors::{
            analysis_error::ToIdeDiagnostic, duplicates::DuplicateError,
        },
    }, hir_def::{
        expressions::spec::{Struct, StructElement},
        interned::identifier::Ident,
    }, hir_ty::ty::Type
};

impl<'db> DataTypeCheck<'db> for Struct<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        let mut seen: FxHashMap<Ident, StructElement> = FxHashMap::default();
        for field in &self.elements(db) {
            match seen.get(&field.get_name_ident(db)) {
                Some(prev) => {
                    errors.push(
                        DuplicateError::StructField {
                            field1: *field,
                            field2: *prev,
                        }
                        .to_diagnostic(db),
                    );
                }
                None => {
                    seen.insert(field.get_name_ident(db), *field);
                }
            }
            //todo: check field type

            if let Some(init_expr) = field.init(db) {
                check_init_expr(
                    db,
                    Type::new_spec(db, field.spec(db)),
                    init_expr,
                    errors,
                );
            }
        }
    }
}
