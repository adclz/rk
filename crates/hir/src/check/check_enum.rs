use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_semantic_index::{Check, DataTypeCheck}, check_ty::check_ty, coerce::coerce_ty_with_expr, errors::{analysis_error::AnalysisError, duplicates::DuplicateError, ty::TyError}
    },
    hir_def::expressions::spec::{ElementarySpec, Enum},
    hir_ty::{
        array_resolver::resolve_range,
        expr_resolver::resolve_expr,
        ty::{Ty, TyDecl, TyKind},
    },
};

impl<'db> DataTypeCheck<'db> for Enum<'db> {
    fn check(
        &'db self,
        db: &'db dyn BaseDatabase,
        ty: Ty<'db>,
        errors: &mut Vec<AnalysisError<'db>>,
    ) {
        let enum_typ = match ty.kind(db) {
            TyKind::Enum { typ, .. } => {
                // Check underlying type
                if let Some(typ) = typ {
                    let typ = typ.spec_to_ty(db, ty.decl(db));
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
                                errors.push(TyError::InvalidEnumType { value: typ }.into())
                            }
                        },
                        _ => errors.push(TyError::InvalidEnumType { value: typ }.into()),
                    }
                }
                *typ
            }
            _ => None,
        };

        let mut seen = FxHashMap::default();
        for variant in &self.variants {
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
            match (variant.value, enum_typ) {
                (Some(value), Some(typ)) => {
                    let value_expr = *resolve_expr(db, value);
                    if let Err(err) = coerce_ty_with_expr(db, typ.spec_to_ty(db, ty.decl(db)), value_expr) {
                        errors.push(TyError::InvalidEnumVariantValue { variant: variant.name, err }.into())
                    }
                }
                _ => {}
            }
        }
    }
}
