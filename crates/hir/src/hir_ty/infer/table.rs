// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{
        ToIdeDiagnostic,
        e03_type::{InferLiteralError, TypeError},
    },
    hir_def::expressions::{
        expression::{Expr, ExprKind, PrimaryExpr, UnaryOperatorKind},
        spec::ElementarySpec,
    },
    hir_ty::{
        body::BodyInferenceResult,
        infer::{Infer, const_eval},
        resolver::Resolver,
        ty::{InferType, Type},
    },
};

/*
    The inference table is used to resolve inferred types during body inference.

    It replaces all Type::Infer types with concrete types based on the context of their usage.
    If no concrete type can be determined, the inferred type is replaced with [`Type::Never`].

*/

#[derive(Default, Debug, Clone)]
pub struct InferenceTable<'db> {
    /// Current inference mode
    pub current_mode: InferMode<'db>,
    /// Map of paths or expressions to their *non* inferred types
    pub types: FxHashMap<Expr<'db>, Type<'db>>,
}

// Source of the inference, used for error reporting
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InferSource<'db> {
    Type(Type<'db>),
    CallSite(CallSite<'db>),
}

impl<'db> From<Type<'db>> for InferSource<'db> {
    fn from(typ: Type<'db>) -> Self {
        InferSource::Type(typ)
    }
}

impl<'db> From<CallSite<'db>> for InferSource<'db> {
    fn from(callsite: CallSite<'db>) -> Self {
        InferSource::CallSite(callsite)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InferMode<'db> {
    // No inference needed
    #[default]
    NoInfer,
    // Still unresolved
    Unresolved,
    // Resolved to an infer type
    ResolvedInfer {
        ty: Type<'db>,
        expr: Option<InferSource<'db>>,
    },
    // Fully resolved type
    // Resolved has a priority over ResolvedInfer
    Resolved {
        ty: Type<'db>,
        expr: Option<InferSource<'db>>,
    },
}

impl<'db> InferenceTable<'db> {
    pub fn new() -> Self {
        Self {
            current_mode: InferMode::NoInfer,
            types: FxHashMap::default(),
        }
    }

    /// Forces the current inference to a resolved type
    pub fn set_target_type(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expr: Option<InferSource<'db>>,
        ty: Type<'db>,
    ) {
        // only elementary types can be used to resolve the inference
        // the inference table will only attempt to resolve infer variants, it does not check coercion
        let normalized = ty.normalize(db);
        let ty = match normalized {
            Type::SubRange(sub) => sub._type(db).infer(db),
            // we also accept bools ... because 0 and 1 literals can be boolean or numeric
            Type::Elementary(_) if normalized.is_numeric() || normalized.is_boolean() => ty,
            // a bare string literal is the third untyped form: STRING by
            // default, CHAR when the slot is one
            Type::Elementary(ElementarySpec::String | ElementarySpec::Char) => ty,
            _ => return,
        };
        self.current_mode = InferMode::Resolved { ty, expr };
    }

    /// Adds a new type to the inference table
    ///
    /// depending on the current inference mode, it may update the mode
    pub fn add_type(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
        value: Type<'db>,
        resolver: Resolver<'db>,
    ) {
        let value = value.normalize(db);
        match value {
            Type::Infer(infer) => {
                match self.current_mode {
                    // sets the inferred type based on the infer type
                    InferMode::NoInfer | InferMode::Unresolved => {
                        self.current_mode = InferMode::ResolvedInfer {
                            ty: infer.to_ty(db),
                            expr: Some(CallSite::from_scoped(db, &expr).into()),
                        };
                    }
                    // inferred types have no priority over already resolved types
                    // check will be happening at the coercion level
                    InferMode::Resolved { .. } | InferMode::ResolvedInfer { .. } => {}
                }
                self.types.insert(expr, value);
            }
            Type::Elementary(elem) => match &self.current_mode {
                // sets the inferred type based on the concrete type
                // the concrete type takes priority over infer types
                InferMode::NoInfer | InferMode::Unresolved | InferMode::ResolvedInfer { .. } => {
                    if !value.is_numeric()
                        && !value.is_boolean()
                        && !matches!(elem, ElementarySpec::String | ElementarySpec::Char)
                    {
                        return;
                    }
                    self.current_mode = InferMode::Resolved {
                        ty: value,
                        expr: Some(CallSite::from_scoped(db, &expr).into()),
                    };
                }
                // already resolved
                InferMode::Resolved { ty, expr } => {
                    // Promote to the wider of the two *only if* implicit
                    // widening exists between them (`ElementarySpec::wider` —
                    // the shared lattice join, also used for binary-operator
                    // result typing). Using raw byte size misclassified
                    // cross-category pairs (e.g. BYTE + REAL where REAL is
                    // "wider" in bytes but not in the IEC cast table), which
                    // caused `ROR(BYTE#0, 0.8)` - an already invalid call - to
                    // spuriously resolve the return to REAL and produce a
                    // cascading assignment error. Incompatible pairs keep the
                    // earlier resolution in place.
                    if let Type::Elementary(ty_elem) = ty
                        && ty_elem.wider(elem) == Some(elem)
                        && *ty_elem != elem
                    {
                        self.current_mode = InferMode::Resolved {
                            ty: value,
                            expr: *expr,
                        };
                    }
                }
            },
            _ => { /*
                ignore for now:
                other types cannot be used to resolve infer variants.
                We may, however, keep that comment if we decide one day to create a richer inference system
                 */
            }
        }
    }

    pub fn get_final_type(&self) -> Type<'db> {
        match self.current_mode {
            InferMode::Resolved { ty, .. } | InferMode::ResolvedInfer { ty, .. } => ty,
            _ => Type::Never,
        }
    }

    /// Resolves completely all inferred types in the table
    pub fn resolve_completly(
        &self,
        db: &'db dyn WorkspaceDataBase,
        resolver: Resolver<'db>,
        results: &mut BodyInferenceResult<'db>,
    ) {
        // Determine the final concrete type we substitute with
        let (final_ty, source) = match self.current_mode {
            InferMode::Resolved { ty, expr } => (ty.normalize(db), expr),
            InferMode::ResolvedInfer { ty, expr } => (ty.normalize(db), expr),
            _ => {
                // no inference to resolve
                // therefore all types are left as is
                return;
            }
        };

        debug_assert!(
            !final_ty.has_infer(),
            "Final type should not be an infer type"
        );

        match final_ty {
            Type::Elementary(spec) => {
                self.resolve_with_elementary_spec(db, final_ty, source, spec, resolver, results)
            }
            _ => unreachable!("Final type should be an elementary type"),
        };
    }

    fn resolve_with_elementary_spec(
        &self,
        db: &'db dyn WorkspaceDataBase,
        final_ty: Type<'db>,
        source: Option<InferSource<'db>>,
        elem: ElementarySpec,
        resolver: Resolver<'db>,
        results: &mut BodyInferenceResult<'db>,
    ) {
        // Insert for each expression the final resolved type. The
        // inference *does not* use coercion — it just checks that the
        // Infer type can be resolved to the final concrete type.
        for (expr, target_type) in &self.types {
            if let Type::Infer(infer) = target_type {
                // Check the literal value against the target type directly:
                // bare literals adapt to the target, `check_as` validates
                // the value range.
                match check_literal(db, *expr, infer, elem) {
                    Ok(()) => {
                        results.type_of_expr.insert(*expr, Type::Elementary(elem));
                        pin_operands(db, *expr, elem, results);
                    }
                    Err(err) => {
                        results.errors.push(
                            TypeError::InferLiteralError {
                                expr: *expr,
                                source,
                                target: final_ty,
                                err,
                            }
                            .to_diagnostic(db, results.scope.file(db)),
                        );

                        // Necessary: the Infer variant MUST be replaced by Type::Never
                        results.type_of_expr.insert(*expr, Type::Never);
                    }
                }
            }
        }
    }
}

/// [`InferType::check_as`], with a minus written apart from the literal read
/// as part of its value: `-(1)` does not fit a UDINT and `-(128)` fits a
/// SINT, where the literal alone reads 1 and 128.
fn check_literal<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    infer: &InferType,
    elem: ElementarySpec,
) -> Result<(), InferLiteralError> {
    if negated_apart(db, expr)
        && let InferType::Integer(_) = infer
        && let Some(value) = const_eval::spec_value(db, expr)
        && let Some(holds) = const_eval::integer_holds(value, elem)
    {
        let type_name = elem.type_name();
        return if holds {
            Ok(())
        } else if value < 0 && const_eval::integer_holds(-1, elem) == Some(false) {
            Err(InferLiteralError::NegativeUnsigned { type_name })
        } else {
            Err(InferLiteralError::OutOfRange { type_name })
        };
    }
    infer.check_as(db, elem).map(|_| ())
}

/// Whether a minus stands between `expr` and its literal, through
/// parentheses and a plus. Without one the literal reads as written, a radix
/// one as its bit pattern.
fn negated_apart<'db>(db: &'db dyn WorkspaceDataBase, expr: Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::UnaryOperator {
            operator: UnaryOperatorKind::Minus,
            ..
        } => true,
        ExprKind::UnaryOperator {
            expr: inner,
            operator: UnaryOperatorKind::Plus,
        }
        | ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner }) => {
            negated_apart(db, *inner)
        }
        _ => false,
    }
}

/// What wraps a literal takes the type the literal is given, so `-(1)` is
/// an INT all the way down, which lowering reads node by node. A sign on a
/// type with no arithmetic, or NOT on one with no bits, is refused here,
/// where that type is first known.
fn pin_operands<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    elem: ElementarySpec,
    results: &mut BodyInferenceResult<'db>,
) {
    let (inner, operator) = match expr.expr(db) {
        ExprKind::UnaryOperator {
            expr: inner,
            operator,
        } => (*inner, Some(*operator)),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner }) => (*inner, None),
        _ => return,
    };
    if !results
        .type_of_expr
        .get(&inner)
        .is_some_and(|ty| ty.has_infer())
    {
        return;
    }
    let ty = Type::Elementary(elem);
    let refused = match operator {
        Some(UnaryOperatorKind::Not) => {
            (!ty.is_boolean() && !ty.is_binary_integer()).then_some("NOT")
        }
        Some(UnaryOperatorKind::Minus) => (!ty.supports_add(db)).then_some("-"),
        Some(UnaryOperatorKind::Plus) => (!ty.supports_add(db)).then_some("+"),
        None => None,
    };
    if let Some(operator) = refused {
        results.errors.push(
            TypeError::UnsupportedOperator {
                call_site: CallSite::from_scoped(db, &expr),
                typ: ty,
                operator,
            }
            .to_diagnostic(db, results.scope.file(db)),
        );
        results.type_of_expr.insert(expr, Type::Never);
        return;
    }
    results.type_of_expr.insert(inner, ty);
    pin_operands(db, inner, elem, results);
}
