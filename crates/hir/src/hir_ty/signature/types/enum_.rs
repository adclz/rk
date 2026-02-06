use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError, e3_type::TypeError,
        e7_enum::EnumError,
    },
    hir_def::expressions::spec::{ElementarySpec, Enum},
    hir_ty::{infer::expr::InferExprCtx, resolver::Resolver, signature::Signature, ty::Type},
};

impl<'db> Signature<'db> {
    pub(crate) fn infer_enum(&mut self, db: &'db dyn WorkspaceDataBase, enm: Enum<'db>) {
        if let Some(spec) = enm.typ(db) {
            let typ = self.infer_spec(db, spec);

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
                    _ => self
                        .errors
                        .push(EnumError::InvalidEnumType { value: spec, typ }.to_diagnostic(db)),
                },
                _ => self
                    .errors
                    .push(EnumError::InvalidEnumType { value: spec, typ }.to_diagnostic(db)),
            }
        }

        let mut seen = FxHashMap::default();
        for variant in &enm.variants(db) {
            // Check duplicate variant names
            match seen.get(&variant.name.ident) {
                Some(prev) => self.errors.push(
                    DuplicateError::EnumVariant {
                        variant1: *prev,
                        variant2: variant.name,
                    }
                    .to_diagnostic(db),
                ),
                None => {
                    seen.insert(variant.name.ident, variant.name);
                }
            }

            // Check variant value type
            if let (Some(value), Some(spec)) = (variant.value, enm.typ(db)) {
                let resolver = Resolver::for_scope(db, value.scope_id(db));
                let mut infer = InferExprCtx::new(resolver);

                let target = self.type_of_specs[&spec];
                infer.resolve_expr(db, value, &mut self.body_infer_result);
                infer.check_expr(db, value, &mut self.body_infer_result);

                if let Err(err) =
                    infer.coerce_type_with_expr(db, target, value, &mut self.body_infer_result)
                {
                    self.errors.push(
                        TypeError::NotAssignable {
                            base_target: target,
                            lhs: err.expected,
                            rhs: err.actual,
                            adjustment: err.adjustment,
                            expr: CallSite::from_expr(db, value),
                        }
                        .to_diagnostic(db),
                    )
                }
            }
        }
    }
}
