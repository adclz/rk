use db::WorkspaceDataBase;

use crate::{
    check::errors::{analysis_error::ToIdeDiagnostic, e6_array::ArrayError},
    hir_def::expressions::spec::Array,
    hir_ty::{signature::Signature, ty::Type},
};

impl<'db> Signature<'db> {
    pub(crate) fn infer_array(&mut self, db: &'db dyn WorkspaceDataBase, array: Array<'db>) {
        let array_spec = Type::new_spec(db, array.of_type(db));
        self.type_of_specs.insert(array.of_type(db), array_spec);

        for range in array.subranges(db) {
            let lower = range.0;
            let upper = range.1;

            match (lower.as_range(db), upper.as_range(db)) {
                (Some(lower_range), Some(upper_range)) => {
                    if lower_range > upper_range {
                        self.errors.push(
                            ArrayError::InferiorUpperBound {
                                lower: lower_range,
                                upper: upper_range,
                                upper_expr: upper,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
                (None, _) => {
                    self.errors.push(
                        ArrayError::InvalidArrayLowerValue { value: lower }.to_diagnostic(db),
                    );
                }
                (_, None) => {
                    self.errors.push(
                        ArrayError::InvalidArrayUpperValue { value: upper }.to_diagnostic(db),
                    );
                }
            }
        }
    }
}
