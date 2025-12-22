use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    CallSite, check::{
        check_semantic_index::DataTypeCheck,
        errors::{
            analysis_error::ToIdeDiagnostic,
            body_inference::{BodyInferenceError, TypeError},
            duplicates::DuplicateError,
            enum_::EnumError,
        },
    }, hir_def::expressions::spec::{ElementarySpec, Enum, SpecKind}, hir_ty::{
        body_inference::BodyInferenceResult,
        infer::{expr::InferExprCtx},
        resolver::Resolver,
        ty::Type,
    }
};

impl<'db> DataTypeCheck<'db> for Enum<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        // Check underlying type
        if let Some(spec) = self.typ(db) {
            let typ = Type::new_spec(db, spec);
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
                    _ => errors
                        .push(EnumError::InvalidEnumType { value: spec, typ }.to_diagnostic(db)),
                },
                _ => errors.push(EnumError::InvalidEnumType { value: spec, typ }.to_diagnostic(db)),
            }
        };

        let mut seen = FxHashMap::default();
        for variant in &self.variants(db) {
            // Check duplicate variant names
            match seen.get(&variant.name.ident) {
                Some(prev) => errors.push(
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
            if let (Some(value), Some(typ)) = (variant.value, self.typ(db)) {
                let resolver = Resolver::new(value.scope_id(db), None);
                let mut infer_body = BodyInferenceResult::new(value.scope_id(db));
                let mut infer = InferExprCtx::new(resolver);

                let target = Type::new_spec(db, typ);
                let expr = infer.infer_expr(db, value, &mut infer_body);
                if let Err(err) = target.coerce_with_type(db, expr, resolver) {
                    errors.push(
                        TypeError::NotAssignable {
                            base_target: target,
                            target: err.expected,
                            value: err.actual,
                            expr: CallSite::from_expr(db, value),
                        }
                        .to_diagnostic(db),
                    )
                }
            }
        }
    }
}
