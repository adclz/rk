use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_semantic_index::{Check, DataTypeCheck},
        coerce::coerce_ty_with_expr,
        errors::{analysis_error::AnalysisError, duplicates::DuplicateError, subrange::SubRangeError},
    },
    hir_def::expressions::spec::{ElementarySpec, Enum, SpecKind, SubRange},
    hir_ty::{
        array_resolver::resolve_range,
        expr_resolver::resolve_expr,
        ty::{Ty, TyKind},
    },
};

impl<'db> DataTypeCheck<'db> for SubRange<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        match self._type.kind(db) {
            SpecKind::Simple(elementary) => match elementary {
                ElementarySpec::Byte
                | ElementarySpec::Word
                | ElementarySpec::DWord
                | ElementarySpec::LWord
                | ElementarySpec::SInt
                | ElementarySpec::USInt
                | ElementarySpec::Int
                | ElementarySpec::UInt
                | ElementarySpec::DInt
                | ElementarySpec::UDInt
                | ElementarySpec::LInt
                | ElementarySpec::ULInt => {}
                _ => {
                    errors.push(SubRangeError::InvalidSubrangeType { typ: *self._type }.into());
                    return;
                }
            },
            _ => {
                errors.push(SubRangeError::InvalidSubrangeType { typ: *self._type }.into());
                return;
            }
        }

        let min = resolve_expr(db, self.lower);
        let max = resolve_expr(db, self.upper);
        if let Err(err) = coerce_ty_with_expr(db, self._type.spec_to_ty(db), min) {
            errors.push(SubRangeError::InvalidSubrangeStart { expr: min, err }.into())
        }

        if let Err(err) = coerce_ty_with_expr(db, self._type.spec_to_ty(db), max) {
            errors.push(SubRangeError::InvalidSubrangeEnd { expr: max, err }.into())
        }
    }
}
