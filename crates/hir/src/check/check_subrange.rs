use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    CallSite, check::{
        check_semantic_index::DataTypeCheck,
        errors::{
            analysis_error::ToIdeDiagnostic, body_inference::TypeError, subrange::SubRangeError,
        },
    }, hir_def::expressions::spec::{ElementarySpec, SpecKind, SubRange}, hir_ty::{
        body_inference::BodyInferenceResult, infer::expr::InferExprCtx, resolver::Resolver,
        ty::Type,
    }
};

impl<'db> DataTypeCheck<'db> for SubRange<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        let typ = Type::new_spec(db, self._type(db));

        match typ {
            Type::Elementary(elementary) => match elementary {
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
                    errors.push(
                        SubRangeError::InvalidSubrangeType {
                            spec: self._type(db),
                            typ,
                        }
                        .to_diagnostic(db),
                    );
                    return;
                }
            },
            _ => {
                errors.push(
                    SubRangeError::InvalidSubrangeType {
                        spec: self._type(db),
                        typ,
                    }
                    .to_diagnostic(db),
                );
                return;
            }
        }

        let min = self.lower(db);
        let max = self.upper(db);

        let resolver = Resolver::new(self.lower(db).scope_id(db), None);
        let mut infer_body = BodyInferenceResult::new(self._type(db).scope_id(db));
        let mut infer = InferExprCtx::new(resolver);

        infer.infer_expr(db, min, &mut infer_body);
        infer.infer_expr(db, max, &mut infer_body);

        infer.inference_table.set_target_type(db, CallSite::from_spec(db, self._type(db)), typ);

        infer
            .inference_table
            .resolve_completly(db, resolver, &mut infer_body);

        let min_typ = infer_body
            .type_of_expr_with_adjustments(db, min)
            .unwrap_or_default();

        if let Err(err) = min_typ.coerce_with_type(db, typ, resolver) {
            errors.push(
                TypeError::NotAssignable {
                    base_target: typ,
                    target: err.expected,
                    value: err.actual,
                    expr: CallSite::from_expr(db, min),
                }
                .to_diagnostic(db),
            )
        }

        let max_typ = infer_body
            .type_of_expr_with_adjustments(db, max)
            .unwrap_or_default();

        if let Err(err) = typ.coerce_with_type(db, max_typ, resolver) {
            errors.push(
                TypeError::NotAssignable {
                    base_target: typ,
                    target: err.expected,
                    value: err.actual,
                    expr: CallSite::from_expr(db, max),
                }
                .to_diagnostic(db),
            )
        }

        for error in infer_body.errors {
            errors.push(error);
        }
    }
}
