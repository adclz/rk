use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::{
        check_semantic_index::DataTypeCheck,
        coerce::coerce_ty_with_expr,
        errors::{
            analysis_error::AnalysisError, subrange::SubRangeError,
        },
    },
    hir_def::expressions::spec::{ElementarySpec, SpecKind, SubRange},
};

impl<'db> DataTypeCheck<'db> for SubRange<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        let typ = self._type(db);

        match typ.kind(db) {
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
                    errors.push(SubRangeError::InvalidSubrangeType { typ }.into());
                    return;
                }
            },
            _ => {
                errors.push(SubRangeError::InvalidSubrangeType { typ }.into());
                return;
            }
        }

        let min = self.lower(db);
        let max = self.upper(db);
        if let Err(err) = coerce_ty_with_expr(db, typ.to_ty(db), min) {
            errors.push(SubRangeError::InvalidSubrangeStart { expr: min, err }.into())
        }

        if let Err(err) = coerce_ty_with_expr(db, typ.to_ty(db), max) {
            errors.push(SubRangeError::InvalidSubrangeEnd { expr: max, err }.into())
        }
    }
}
