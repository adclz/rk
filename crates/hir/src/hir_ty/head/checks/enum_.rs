use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{
        ToIdeDiagnostic, e1_duplicates::DuplicateError, e3_type::TypeError, e7_enum::EnumError,
    },
    hir_def::expressions::spec::{ElementarySpec, Enum},
    hir_ty::{
        head::init_inference::InitInference,
        infer::{Infer, expr::InferExprCtx},
        resolver::Resolver,
        ty::Type,
    },
};

impl<'db> InitInference<'db> {
    pub(crate) fn check_enum(&mut self, db: &'db dyn WorkspaceDataBase, enm: Enum<'db>) {

        if let Some(spec) = enm.typ(db) {
            let typ = spec.infer(db);

            match typ {
                Type::Elementary(
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
                    | ElementarySpec::ULInt,
                ) => {}
                _ => self.errors.push(
                    EnumError::InvalidEnumType { value: spec, typ }
                        .to_diagnostic(db, self.scope.file(db)),
                ),
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
                    .to_diagnostic(db, self.scope.file(db)),
                ),
                None => {
                    seen.insert(variant.name.ident, variant.name);
                }
            }

            // Check variant value type
            if let (Some(value), Some(spec)) = (variant.value, enm.typ(db)) {
                let resolver = Resolver::for_scope(db, value.scope_id(db));
                let mut infer = InferExprCtx::new(resolver);

                let target = spec.infer(db);
                infer.resolve_expr(db, value, &mut self.body_infer_result);
                infer.check_expr(db, value, &mut self.body_infer_result);

                if let Err(err) =
                    infer.coerce_type_with_expr(db, target, value, &mut self.body_infer_result)
                {
                    self.errors.push(
                        TypeError::NotAssignable {
                            suggest_cast: true,
                            base_target: target,
                            lhs: err.expected,
                            rhs: err.actual,
                            adjustment: err.adjustment,
                            expr: CallSite::from_scoped(db, &value),
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    )
                }
            }
        }

        // Every declared value must fold: the ordinal an enum literal lowers
        // to is read from this evaluation, so a value only the runtime knows
        // has no ordinal to give. AFTER the loop above — folding needs the
        // resolutions it recorded.
        for (variant, ordinal) in
            crate::hir_ty::infer::const_eval::enum_ordinals_with(db, enm, &self.body_infer_result)
        {
            if ordinal.is_none()
                && let Some(value) = variant.value
            {
                self.body_infer_result.errors.push(
                    crate::check::errors::e7_enum::EnumError::EnumValueNotConstant { value }
                        .to_diagnostic(db, self.body_infer_result.scope.file(db)),
                );
            }
        }
    }
}
