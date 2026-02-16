use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    check::errors::{analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError},
    hir_ty::{head::init_inference::InitInference, infer::Infer},
};


impl<'db> InitInference<'db> {
    pub(crate) fn check_generics(&mut self, db: &'db dyn WorkspaceDataBase) {
        let generics = match self.scope.generics(db) {
            Some(vars) => vars,
            None => return,
        };

        let mut seen = FxHashMap::default();

        for var in generics {
            match seen.get(&var.get_name_ident(db)) {
                Some(prev) => {
                    self.errors.push(
                        DuplicateError::Generic {
                            param1: *var,
                            param2: *prev,
                        }
                        .to_diagnostic(db),
                    );
                }
                None => {
                    seen.insert(var.get_name_ident(db), *var);
                }
            }
        }
    }
}
