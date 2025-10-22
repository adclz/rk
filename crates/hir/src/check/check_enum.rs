use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_semantic_index::DataTypeCheck,
        coerce::coerce_ty_with_expr,
        errors::{analysis_error::AnalysisError, duplicates::DuplicateError, enum_::EnumError},
    },
    hir_def::expressions::spec::{ElementarySpec, Enum, SpecKind},
    hir_ty::expr_resolver::resolve_expr,
};

impl<'db> DataTypeCheck<'db> for Enum<'db> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        // Check underlying type
        if let Some(typ) = self.typ(db) {
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
                    _ => errors.push(EnumError::InvalidEnumType { value: typ }.into()),
                },
                _ => errors.push(EnumError::InvalidEnumType { value: typ }.into()),
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
                    .into(),
                ),
                None => {
                    seen.insert(variant.name.ident, variant.name);
                }
            }

            // Check variant value type
            if let (Some(value), Some(typ)) = (variant.value, self.typ(db)) {
                let value_expr = resolve_expr(db, value);
                if let Err(err) = coerce_ty_with_expr(db, typ.to_ty(db), value_expr) {
                    errors.push(
                        EnumError::InvalidEnumVariantValue {
                            variant: variant.name,
                            err,
                        }
                        .into(),
                    )
                }
            }
        }
    }
}
