use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_semantic_index::DataTypeCheck,
        errors::{analysis_error::{AnalysisError, ToIdeDiagnostic}, array::ArrayError},
    },
    hir_def::expressions::spec::Array,
    hir_ty::array_resolver::resolve_range,
};

impl<'db> DataTypeCheck<'db> for Array<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        for range in &self.subranges(db) {
            let lower = range.0;
            let upper = range.1;

            match (resolve_range(db, range.0), resolve_range(db, range.1)) {
                (Some(lower_range), Some(upper_range)) => {
                    if lower_range > upper_range {
                        errors.push(
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
                    errors.push(ArrayError::InvalidArrayLowerValue { value: lower }.to_diagnostic(db));
                }
                (_, None) => {
                    errors.push(ArrayError::InvalidArrayUpperValue { value: upper }.to_diagnostic(db));
                }
            }
        }
    }
}
