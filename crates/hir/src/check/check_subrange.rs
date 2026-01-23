use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    CallSite,
    check::{
        check_semantic_index::DataTypeCheck,
        errors::{analysis_error::ToIdeDiagnostic, e3_type::TypeError, e8_subrange::SubRangeError},
    },
    hir_def::expressions::spec::{ElementarySpec, SubRange},
    hir_ty::{
        body_inference::BodyInferenceResult, infer::expr::InferExprCtx, resolver::Resolver,
        ty::Type,
    },
};

impl<'db> DataTypeCheck<'db> for SubRange<'db> {
    fn check(&'db self, db: &'db dyn WorkspaceDataBase, errors: &mut Vec<IdeDiagnostic>) {
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

        let resolver = Resolver::for_scope(db, self.lower(db).scope_id(db));
        let mut infer_body = BodyInferenceResult::new(self._type(db).scope_id(db));
        let mut infer = InferExprCtx::new(resolver);

        infer.resolve_expr(db, min, &mut infer_body);
        infer.resolve_expr(db, max, &mut infer_body);
        infer.check_expr(db, min, &mut infer_body);
        infer.check_expr(db, max, &mut infer_body);

        if let Err(err) = infer.coerce_type_with_expr(db, typ, min, &mut infer_body) {
            errors.push(
                TypeError::NotAssignable {
                    base_target: typ,
                    lhs: err.expected,
                    rhs: err.actual,
                    adjustment: err.adjustment,
                    expr: CallSite::from_expr(db, min),
                }
                .to_diagnostic(db),
            )
        }

        if let Err(err) = infer.coerce_type_with_expr(db, typ, max, &mut infer_body) {
            errors.push(
                TypeError::NotAssignable {
                    base_target: typ,
                    lhs: err.expected,
                    rhs: err.actual,
                    adjustment: err.adjustment,
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
