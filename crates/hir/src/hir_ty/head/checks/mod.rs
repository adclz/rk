use db::WorkspaceDataBase;

use crate::{
    hir_def::expressions::spec::{Spec, SpecKind},
    hir_ty::head::init_inference::InitInference,
};

pub mod array;
pub mod enum_;
pub mod methods;
pub mod strukt;
pub mod subrange;
pub mod usings;
pub mod variables;
pub mod generics;

impl<'db> InitInference<'db> {
    pub fn check_spec(&mut self, db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) {
        match spec.kind(db) {
            SpecKind::Array(arr) => {
                self.check_array(db, *arr);
            }
            SpecKind::Enum(enm) => {
                self.check_enum(db, *enm);
            }
            SpecKind::Subrange(subrange) => {
                self.check_subrange(db, *subrange);
            }
            SpecKind::Struct(strukt) => {
                self.check_struct(db, *strukt);
            }
            _ => {}
        }
    }
}
