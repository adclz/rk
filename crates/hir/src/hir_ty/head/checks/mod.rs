use crate::check::errors::ToIdeDiagnostic;
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
            // The length is part of the type — it decides how many bytes the
            // variable takes — so one the compiler cannot work out leaves the
            // layout unknowable. Refused here rather than defaulted to 80,
            // which would size the storage wrongly and say nothing.
            SpecKind::SizedString(length)
                if crate::hir_ty::infer::const_eval::spec_bound(db, *length).is_none() =>
            {
                {
                    self.errors.push(
                        crate::check::errors::e3_type::TypeError::StringLengthNotConstant {
                            length: *length,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
            _ => {}
        }
    }
}
