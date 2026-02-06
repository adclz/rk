use db::WorkspaceDataBase;

use crate::{
    check::errors::{analysis_error::ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::expressions::spec::{Spec, SpecKind},
    hir_ty::{signature::Signature, ty::Type},
};

pub mod array;
pub mod enum_;
pub mod strukt;
pub mod subrange;

impl<'db> Signature<'db> {
    #[must_use]
    pub fn infer_spec(&mut self, db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Type<'db> {
        let typ = Type::new_spec(db, spec);

        match spec.kind(db) {
            SpecKind::Array(arr) => {
                self.infer_array(db, *arr);
            }
            SpecKind::Enum(enm) => {
                self.infer_enum(db, *enm);
            }
            SpecKind::Subrange(subrange) => {
                self.infer_subrange(db, *subrange);
            }
            SpecKind::Struct(strukt) => {
                self.infer_struct(db, *strukt);
            }
            _ => {}
        }

        match typ {
            Type::Never => {
                if let SpecKind::Target(target) = spec.kind(db) {
                    self.errors.push(
                        ResolveError::NoNamespaceItemFound {
                            path: target.clone(),
                        }
                        .to_diagnostic(db),
                    );
                }
            }
            Type::Function(_) => {
                self.errors.push(
                    ResolveError::FunctionAsVariableType {
                        expr: spec,
                        ty: typ,
                    }
                    .to_diagnostic(db),
                );
            }
            _ => (),
        };
        self.type_of_specs.insert(spec, typ);
        typ
    }
}
