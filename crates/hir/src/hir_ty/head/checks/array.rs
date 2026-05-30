use db::WorkspaceDataBase;

use crate::{
    check::errors::{ToIdeDiagnostic, e6_array::ArrayError},
    hir_def::expressions::spec::Array,
    hir_ty::{head::init_inference::InitInference, infer::expr::InferExprCtx, resolver::Resolver},
};

impl<'db> InitInference<'db> {
    pub(super) fn check_array(&mut self, db: &'db dyn WorkspaceDataBase, array: Array<'db>) {
        for range in array.subranges(db) {
            let lower = range.0;
            let upper = range.1;

            let resolver = Resolver::for_scope(db, lower.scope_id(db));
            let mut infer = InferExprCtx::new(resolver);

            infer.resolve_expr(db, lower, &mut self.body_infer_result);
            infer.check_expr(db, lower, &mut self.body_infer_result);

            infer.resolve_expr(db, upper, &mut self.body_infer_result);
            infer.check_expr(db, upper, &mut self.body_infer_result);

            match (lower.as_range(db), upper.as_range(db)) {
                (Some(lower_range), Some(upper_range)) => {
                    if lower_range > upper_range {
                        self.errors.push(
                            ArrayError::InferiorUpperBound {
                                lower: lower_range,
                                upper: upper_range,
                                upper_expr: upper,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                }
                (None, _) => {
                    self.errors.push(
                        ArrayError::InvalidArrayLowerValue { value: lower }
                            .to_diagnostic(db, self.scope.file(db)),
                    );
                }
                (_, None) => {
                    self.errors.push(
                        ArrayError::InvalidArrayUpperValue { value: upper }
                            .to_diagnostic(db, self.scope.file(db)),
                    );
                }
            }
        }
    }
}
