use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_init_expr::check_init_expr,
        check_semantic_index::Check,
        check_ty::check_ty,
        errors::{duplicates::DuplicateError, sem_errors::AnalysisError, ty::TyError},
    },
    hir_def::{
        expressions::spec::{Array, Struct},
        pous::variable::VariableDecl,
    },
    hir_ty::{
        array_resolver::resolve_range,
        expr_resolver::resolve_expr,
        init_expr_resolver::resolve_init_expr,
        ty::{ty_for_struct_field, ty_for_variable},
    },
};

impl<'db> Check<'db> for Array<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        for range in &self.subranges {
            let lower = resolve_expr(db, range.0);
            let upper = resolve_expr(db, range.1);

            match (resolve_range(db, range.0), resolve_range(db, range.1)) {
                (Some(lower_range), Some(upper_range)) => {
                    if lower_range > upper_range {
                        errors.push(
                            TyError::InferiorUpperBound {
                                lower: lower_range,
                                upper: upper_range,
                                upper_expr: *upper,
                            }
                            .into(),
                        );
                    }
                }
                (None, _) => {
                    errors.push(TyError::InvalidArrayLowerValue { value: *lower }.into());
                }
                (_, None) => {
                    errors.push(TyError::InvalidArrayUpperValue { value: *upper }.into());
                }
            }
        }
    }
}
