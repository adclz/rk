use crate::check::errors::ToIdeDiagnostic;
use db::WorkspaceDataBase;

use crate::{
    hir_def::expressions::spec::{Spec, SpecKind},
    hir_ty::head::init_inference::InitInference,
};

pub mod array;
pub mod enum_;
pub mod functions;
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
            SpecKind::SizedString(length) => {
                self.check_string_length(db, *length);
            }
            _ => {}
        }
    }

    /// The length is part of the type — it decides how many bytes the
    /// variable takes — so one the compiler cannot work out leaves the layout
    /// unknowable. Refused here rather than defaulted, which would size the
    /// storage wrongly and say nothing.
    ///
    /// Resolving it is what records its type, the way an array's bounds are
    /// resolved: without that the number in `STRING[20]` hovered as unknown.
    fn check_string_length(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        length: crate::hir_def::expressions::expression::Expr<'db>,
    ) {
        let resolver = crate::hir_ty::resolver::Resolver::for_scope(db, length.scope_id(db));
        let mut infer = crate::hir_ty::infer::expr::InferExprCtx::new(resolver);
        infer.resolve_expr(db, length, &mut self.body_infer_result);
        infer.check_expr(db, length, &mut self.body_infer_result);

        if crate::hir_ty::infer::const_eval::spec_bound(db, length).is_none() {
            self.errors.push(
                crate::check::errors::e03_type::TypeError::StringLengthNotConstant { length }
                    .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }
}
