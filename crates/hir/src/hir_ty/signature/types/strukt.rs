use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError,
    },
    hir_def::expressions::spec::{Struct, StructElement},
    hir_ty::signature::Signature,
};

impl<'db> Signature<'db> {
    pub(crate) fn infer_struct(&mut self, db: &'db dyn WorkspaceDataBase, strukt: Struct<'db>) {
        let mut seen = FxHashMap::default();
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
        let element_type = self.infer_spec(db, element.spec(db));

        if let Some(init_expr) = element.init(db) {
            self.init_expr_result.resolve_init_expr(
                db,
                init_expr,
                &mut self.body_infer_result,
                element_type,
            );
        }
    }
}
