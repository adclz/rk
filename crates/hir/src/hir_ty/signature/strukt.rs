use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError,
    },
    hir_def::{
        expressions::spec::{SpecKind, Struct, StructElement},
    },
    hir_ty::{signature::Signature, ty::Type},
};

impl<'db> Signature<'db> {
    pub(crate) fn infer_struct(&mut self, db: &'db dyn WorkspaceDataBase, strukt: Struct<'db>) {
        let mut seen= FxHashMap::default();
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
            self.infer_struct_element(db, *field);
        }
    }

    fn infer_struct_element(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        element: StructElement<'db>,
    ) {
        let element_type = Type::new_spec(db, element.spec(db));
        if element_type.is_never() {
            if let SpecKind::Target(target) = element.spec(db).kind(db) {
                self.errors.push(
                    ResolveError::NoNamespaceItemFound {
                        path: target.clone(),
                    }
                    .to_diagnostic(db),
                );
            }
        }
        self.type_of_specs.insert(element.spec(db), element_type);
    }
}
