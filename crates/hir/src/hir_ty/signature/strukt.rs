
use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError,
    },
    hir_def::{
        expressions::spec::{Struct, StructElement},
        interned::identifier::Ident,
    },
    hir_ty::signature::Signature,
};

impl<'db> Signature<'db> {
    pub 
    fn infer_struct(&mut self, db: &'db dyn WorkspaceDataBase, strukt: Struct<'db>) {
        let mut seen: FxHashMap<Ident, StructElement> = FxHashMap::default();
        for field in &strukt.elements(db) {
            match seen.get(&field.get_name_ident(db)) {
                Some(prev) => {
                    self.errors.push(
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