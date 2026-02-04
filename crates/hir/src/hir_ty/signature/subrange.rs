use db::WorkspaceDataBase;

use crate::{
    CallSite,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e3_type::TypeError, e8_subrange::SubRangeError,
    },
    hir_def::expressions::spec::{ElementarySpec, SubRange},
    hir_ty::{infer::expr::InferExprCtx, resolver::Resolver, signature::Signature, ty::Type},
};

impl<'db> Signature<'db> {
    pub(crate) fn infer_subrange(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        subrange: SubRange<'db>,
    ) {
        let typ = Type::new_spec(db, subrange._type(db));

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
                    self.errors.push(
                        SubRangeError::InvalidSubrangeType {
                            spec: subrange._type(db),
                            typ,
                        }
                        .to_diagnostic(db),
                    );
                    return;
                }
            },
            _ => {
                self.errors.push(
                    SubRangeError::InvalidSubrangeType {
                        spec: subrange._type(db),
                        typ,
                    }
                    .to_diagnostic(db),
                );
                return;
            }
        }

        let min = subrange.lower(db);
        let max = subrange.upper(db);

        let resolver = Resolver::for_scope(db, subrange.lower(db).scope_id(db));
        let mut infer = InferExprCtx::new(resolver);

        infer.resolve_expr(db, min, &mut self.body_infer_result);
        infer.resolve_expr(db, max, &mut self.body_infer_result);
        infer.check_expr(db, min, &mut self.body_infer_result);
        infer.check_expr(db, max, &mut self.body_infer_result);

        if let Err(err) = infer.coerce_type_with_expr(db, typ, min, &mut self.body_infer_result) {
            self.errors.push(
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

        if let Err(err) = infer.coerce_type_with_expr(db, typ, max, &mut self.body_infer_result) {
            self.errors.push(
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
    }
}
