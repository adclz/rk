use auto_lsp::default::db::BaseDatabase;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        check_semantic_index::DataTypeCheck,
        errors::{analysis_error::ToIdeDiagnostic, array::ArrayError},
    },
    hir_def::expressions::spec::Array,
};

impl<'db> DataTypeCheck<'db> for Array<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
        for range in &self.subranges(db) {
            let lower = range.0;
            let upper = range.1;

            match (lower.as_range(db), upper.as_range(db)) {
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
                    errors.push(
                        ArrayError::InvalidArrayLowerValue { value: lower }.to_diagnostic(db),
                    );
                }
                (_, None) => {
                    errors.push(
                        ArrayError::InvalidArrayUpperValue { value: upper }.to_diagnostic(db),
                    );
                }
            }
        }
    }
}
