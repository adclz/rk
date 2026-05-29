use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{ToIdeDiagnostic, e1_duplicates::DuplicateError},
    hir_def::expressions::spec::Struct,
    hir_ty::{head::init_inference::InitInference, infer::Infer},
};

impl<'db> InitInference<'db> {
    pub(crate) fn check_struct(&mut self, db: &'db dyn WorkspaceDataBase, strukt: Struct<'db>) {
        let mut seen = FxHashMap::default();

        for field in &strukt.elements(db) {
            match seen.get(&field.get_name_ident(db)) {
                Some(prev) => {
                    self.errors.push(
                        DuplicateError::StructField {
                            field1: *field,
                            field2: *prev,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
                None => {
                    seen.insert(field.get_name_ident(db), *field);
                }
            }

            let element_type = field.spec(db).infer(db);
            if let Some(init_expr) = field.init(db) {
                self.init_expr_result.resolve_init_expr(
                    db,
                    init_expr,
                    &mut self.body_infer_result,
                    element_type,
                );
            }
        }
    }
}
