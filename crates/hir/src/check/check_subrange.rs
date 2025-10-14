use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_semantic_index::{Check, DataTypeCheck},
        check_ty::check_ty,
        coerce::coerce_ty_with_expr,
        errors::{analysis_error::AnalysisError, duplicates::DuplicateError, ty::TyError},
    },
    hir_def::expressions::spec::{ElementarySpec, Enum, SubRange},
    hir_ty::{
        array_resolver::resolve_range,
        expr_resolver::resolve_expr,
        ty::{Ty, TyKind},
    },
};

impl<'db> DataTypeCheck<'db> for SubRange<'db> {
    fn check(
        &'db self,
        db: &'db dyn BaseDatabase,
        ty: Ty<'db>,
        errors: &mut Vec<AnalysisError<'db>>,
    ) {
        match ty.kind(db) {
            TyKind::SubRange(subrange) => {
                let typ = subrange._type.spec_to_ty(db);
                check_ty(db, typ, errors);
                match typ.kind(db) {
                    TyKind::Simple(elementary) => match elementary {
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
                            errors.push(TyError::InvalidSubrangeType { value: typ }.into());
                            return;
                        },
                    },
                    _ => {
                        errors.push(TyError::InvalidSubrangeType { value: typ }.into());
                        return;
                    }
                }

                let min = resolve_expr(db, subrange.lower);
                let max = resolve_expr(db, subrange.upper);
                if let Err(err) = coerce_ty_with_expr(db, typ, min) {
                    errors.push(TyError::InvalidSubrangeStart { expr: min, err }.into())
                }

                if let Err(err) = coerce_ty_with_expr(db, typ, max) {
                    errors.push(TyError::InvalidSubrangeEnd { expr: max, err }.into())
                }
            }
            _ => unreachable!(""),
        };
    }
}
