use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::{
        check_semantic_index::DataTypeCheck,
        errors::{analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError},
    },
    hir_def::{
        expressions::spec::{Struct, StructElement},
        interned::identifier::Ident,
    },
};

impl<'db> DataTypeCheck<'db> for Struct<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
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
        }
    }
}
