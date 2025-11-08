use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_init_expr::check_init_expr,
        check_semantic_index::DataTypeCheck,
        errors::{
            analysis_error::ToIdeDiagnostic, duplicates::DuplicateError,
        },
    },
    hir_def::{
        expressions::spec::{Struct, StructElement},
        interned::identifier::Ident,
    },
    hir_ty::{
        init_expr_resolver::resolve_init_expr,
        ty::TyKind,
    },
};

impl<'db> DataTypeCheck<'db> for Struct<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        let mut seen: FxHashMap<Ident, StructElement> = FxHashMap::default();
        for field in &self.elements(db) {
            match seen.get(field.name(db)) {
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
                    seen.insert(*field.name(db), *field);
                }
            }

            if let TyKind::Err(err) = field.spec(db).to_ty(db).kind(db) {
                errors.push(err.to_diagnostic(db));
            }

            if let Some(init_expr) = field.init(db) {
                check_init_expr(
                    db,
                    field.spec(db).to_ty(db),
                    *resolve_init_expr(db, field.spec(db).to_ty(db), init_expr),
                    errors,
                );
            }
        }
    }
}
