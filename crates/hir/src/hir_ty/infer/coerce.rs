use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e3_type::TypeError, e10_control_flow::ControlFlowError},
    hir_def::{
        expressions::expression::{AddOperatorKind, MultOperatorKind},
        pous::pou::Pou,
    },
    hir_ty::{
        body::{Adjustment, AdjustmentInfo, BodyInferenceResult},
        infer::Infer,
        polymorphism::{interface_extends, pou_implements_interface},
        resolver::Resolver,
        ty::Type,
    },
};

pub struct CoerceError<'db> {
    pub expected: Type<'db>,
    pub actual: Type<'db>,
    pub adjustment: Option<Adjustment<'db>>,
}

pub type CoerceResult<'db> = Result<(), CoerceError<'db>>;

impl<'db> Type<'db> {
    /// The bounds violation, if `rhs` is a constant that the subrange `self`
    /// cannot hold.
    ///
    /// Assignability for a subrange is not decided by its base type alone: the
    /// declared bounds narrow it further, and other toolchains reports `i := 5000` on
    /// `INT (-4095..4095)` at compile time. Bounds are statically known, so a
    /// constant can be settled here; a non-constant would need a runtime guard,
    /// which IEC leaves optional (other toolchains exposes it separately as
    /// `CheckRangeSigned`/`CheckRangeUnsigned`).
    ///
    /// Returns the error rather than reporting it, like [`Self::coerce_with_type`] —
    /// the caller decides whether to surface it.
    pub fn subrange_violation(
        &self,
        db: &'db dyn WorkspaceDataBase,
        rhs: crate::hir_def::expressions::expression::Expr<'db>,
    ) -> Option<crate::check::errors::e8_subrange::SubRangeError<'db>> {
        let sub = self.as_subrange(db)?;
        // The DECLARED bounds fold through the spec evaluator — literal-only
        // folding silently skipped this check for a CONSTANT-bounded
        // subrange.
        let (lower, upper) = match crate::hir_ty::infer::const_eval::subrange_bounds(db, sub) {
            (Some(lower), Some(upper)) => (lower, upper),
            _ => return None,
        };
        let value = rhs.as_const_int(db)?;
        (value < lower || value > upper).then_some(
            crate::check::errors::e8_subrange::SubRangeError::ValueOutOfRange {
                expr: rhs,
                value,
                lower,
                upper,
            },
        )
    }

    pub fn supports_add(&self, _db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_numeric() || self.is_time()
    }

    pub fn supports_mul(&self, _db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_numeric()
    }

    pub fn supports_div(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.supports_mul(db)
    }

    pub fn supports_mod(&self, _db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_signed_integer() || self.is_unsigned_integer()
    }

    pub fn supports_power(&self, _db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_float()
    }

    pub fn supports_bool_op(&self, _db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_boolean() || self.is_numeric()
    }

    pub fn supports_comparison(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self, Type::Elementary(_))
    }

    pub fn can_be_variadic(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self, Type::Elementary(_))
    }

    // Type coercion check
    // `resolver` is part of the public coercion API and is threaded through
    // recursive calls even though the top level doesn't read it directly.
    #[allow(clippy::only_used_in_recursion)]
    pub fn coerce_with_type(
        &self,
        db: &'db dyn WorkspaceDataBase,
        to: Type<'db>,
        adjustments: Option<&[Adjustment<'db>]>,
        resolver: Resolver<'db>,
    ) -> CoerceResult<'db> {
        // normalizing first to peel Variable/DataType/StructElement wrappers
        let lhs = self.normalize(db);
        let to = to.normalize(db);

        // We return Ok if the lhs or rhs is of type Never.
        // Never variants are already reported by the resolver and we don't want to propagate cascading errors.
        if lhs.is_never() || to.is_never() {
            return Ok(());
        }

        // types *must* not be infer variants during coercion
        debug_assert!(!lhs.has_infer());
        debug_assert!(!to.has_infer());

        // use the adjustments to allow coercions for references,
        // but only if the reference can be dereferenced to the expected type
        if let Some(adjs) = adjustments
            && let Some(typ) = adjs.as_reference()
        {
            match lhs {
                Type::RefTo(spec) => {
                    return match spec.infer(db).coerce_with_type(db, to, None, resolver) {
                        Ok(()) => Ok(()),
                        Err(_) => match spec.infer(db).eq(&to) {
                            true => Ok(()),
                            false => Err(CoerceError {
                                expected: *self,
                                actual: to,
                                adjustment: adjs.iter().last().cloned(),
                            }),
                        },
                    };
                }
                // A function or method NAME on the left is its return slot,
                // which `normalize` leaves alone. Falling through lets the
                // arms below forward to the declared return type, carrying
                // these adjustments — without it `f := REF(x)` was refused
                // for every reference-returning POU, naming the SLOT where
                // the message wanted a type ("expected 'mk'").
                Type::Function(_) | Type::MethodDecl(_) => {}
                _ => {
                    return Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: adjs.iter().last().cloned(),
                    });
                }
            }
        }

        match (lhs, &to) {
            // A variant belongs to exactly one enum, and the type says which
            // TYPE it was written through: normalize peels that down to the
            // enum spec, so an alias's variant still matches its base enum,
            // while a foreign enum's variant is a mismatch, not a pass.
            (Type::Enum(e1), Type::EnumVariant(dt, _)) => {
                match Type::DataType(*dt).normalize(db) {
                    Type::Enum(e2) if e1.eq(&e2) => Ok(()),
                    _ => Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    }),
                }
            }
            (Type::Enum(e1), Type::Enum(e2)) => {
                if e1.eq(e2) {
                    Ok(())
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    })
                }
            }
            // same types are assignable
            (Type::Struct(s1), Type::Struct(s2)) => match s1.eq(s2) {
                true => Ok(()),
                false => Err(CoerceError {
                    expected: *self,
                    actual: to,
                    adjustment: None,
                }),
            },
            // check element spec equality
            (Type::StructElement(elem), rhs) => {
                elem.spec(db)
                    .infer(db)
                    .coerce_with_type(db, *rhs, adjustments, resolver)
            }
            // same types are assignable
            (Type::Array(a1), Type::Array(a2)) => {
                if a1.eq(a2) {
                    return Ok(());
                }
                // Structural comparison: same dimensions, same bounds, coercible element type
                let s1 = a1.subranges(db);
                let s2 = a2.subranges(db);
                if s1.len() != s2.len() {
                    return Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    });
                }
                // Folded, not literal-matched: a CONSTANT bound must compare
                // by its VALUE, and `as_range` also refused every NEGATIVE
                // bound — two separately declared `ARRAY[-1..1]` types were
                // never mutually assignable.
                let d1 = crate::hir_ty::infer::const_eval::array_dimensions(db, a1);
                let d2 = crate::hir_ty::infer::const_eval::array_dimensions(db, *a2);
                for (r1, r2) in d1.iter().zip(d2.iter()) {
                    let bounds_match = r1.0 == r2.0 && r1.1 == r2.1 && r1.0.is_some();
                    if !bounds_match {
                        return Err(CoerceError {
                            expected: *self,
                            actual: to,
                            adjustment: None,
                        });
                    }
                }
                a1.of_type(db).infer(db).coerce_with_type(
                    db,
                    a2.of_type(db).infer(db),
                    None,
                    resolver,
                )
            }
            // check array spec equality
            (Type::Array(a1), rhs) => {
                a1.of_type(db)
                    .infer(db)
                    .coerce_with_type(db, *rhs, adjustments, resolver)
            }
            // No SubRange arms: `normalize` resolves a subrange to its base, so
            // both sides arrive here already peeled — an `INT (0..100)` and an
            // `INT` meet as two INTs. Bounds are enforced by
            // `subrange_violation`, which resolves the subrange itself.
            (Type::Elementary(lhs), Type::Elementary(rhs)) => {
                if lhs == *rhs {
                    return Ok(());
                }
                // try implicit conversions in both directions
                match lhs.implicit_cast(*rhs).is_some() {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    }),
                }
            }
            (Type::RefTo(_), Type::Null) => Ok(()),
            (Type::RefTo(lhs), Type::RefTo(rhs)) => {
                lhs.infer(db)
                    .coerce_with_type(db, rhs.infer(db), None, resolver)
            }
            // Function/Method used as a value — coerce through the return type.
            // LHS (target is the function return variable)
            (Type::Function(f), rhs) => match f.return_type(db) {
                Some(ret) => ret
                    .infer(db)
                    .coerce_with_type(db, *rhs, adjustments, resolver),
                None => Err(CoerceError {
                    expected: Type::Void,
                    actual: to,
                    adjustment: None,
                }),
            },
            (Type::MethodDecl(f), rhs) => match f.return_type(db) {
                Some(ret) => ret
                    .infer(db)
                    .coerce_with_type(db, *rhs, adjustments, resolver),
                None => Err(CoerceError {
                    expected: Type::Void,
                    actual: to,
                    adjustment: None,
                }),
            },
            // RHS (function/method name read as a value)
            (lhs, Type::Function(f)) => match f.return_type(db) {
                Some(ret) => self.coerce_with_type(db, ret.infer(db), adjustments, resolver),
                None => Err(CoerceError {
                    expected: *self,
                    actual: Type::Void,
                    adjustment: None,
                }),
            },
            (lhs, Type::MethodDecl(f)) => match f.return_type(db) {
                Some(ret) => self.coerce_with_type(db, ret.infer(db), adjustments, resolver),
                None => Err(CoerceError {
                    expected: *self,
                    actual: Type::Void,
                    adjustment: None,
                }),
            },
            // An instance passed where its own POU is expected. VAR_IN_OUT binds
            // by reference, so this hands over the instance rather than copying
            // it — the way to share one. Assigning an instance is a different
            // question and stays refused, by the check on the assignment TARGET
            // (E0226), not here.
            (Type::FunctionBlock(expected), Type::FunctionBlock(actual))
                if expected == *actual =>
            {
                Ok(())
            }
            (Type::Class(expected), Type::Class(actual)) if expected == *actual => Ok(()),
            // Interface coercion: class/FB that implements the interface, or sub-interface
            (Type::Interface(target_itf), Type::Class(cls)) => {
                if pou_implements_interface(db, Pou::Class(*cls), target_itf) {
                    Ok(())
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    })
                }
            }
            (Type::Interface(target_itf), Type::FunctionBlock(fb)) => {
                if pou_implements_interface(db, Pou::FunctionBlock(*fb), target_itf) {
                    Ok(())
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    })
                }
            }
            (Type::Interface(target_itf), Type::Interface(src_itf)) => {
                if interface_extends(db, *src_itf, target_itf) {
                    Ok(())
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    })
                }
            }
            _ => Err(CoerceError {
                expected: *self,
                actual: to,
                adjustment: None,
            }),
        }
    }

    /// Check if a type is a direct type that cannot be used as a value
    /// in the body. Returns true if the type is valid (not a direct type),
    /// false and emits an error if it is.
    pub fn check_not_direct_type(
        &self,
        db: &'db dyn WorkspaceDataBase,
        call_site: CallSite<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> bool {
        if self.is_never() {
            return true;
        }

        if !self.is_direct_type() {
            return true;
        }

        // function and methods can be used IF they are in the same scope (self-assignment)
        let self_assign = match self {
            Type::Function(f) => f.get_scope_id(db) == call_site.get_scope_id(db),
            Type::MethodDecl(m) => m.get_scope_id(db) == call_site.get_scope_id(db),
            _ => false,
        };
        if self_assign {
            return true;
        }

        ctx.errors.push(
            ControlFlowError::DirectType {
                expr: call_site,
                typ: *self,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
        false
    }

    /// Whether this target can be assigned to, reporting why if it cannot.
    ///
    /// A `false` return means a diagnostic was already emitted, so the caller
    /// must not go on to type-check the assignment: the target being illegal
    /// says nothing about the value, and checking anyway reported a second,
    /// worse error — `expected 'Worker', got 'Worker'` on top of the real one.
    pub fn check_assignable(
        &self,
        db: &'db dyn WorkspaceDataBase,
        call_site: CallSite<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> bool {
        if self.is_never() {
            return true;
        }

        match self {
            Type::Variable((variable, multibits)) => {
                let mut assignable = true;
                // a variable of callable type cannot be assigned to
                if let Some(callable_typ) = variable.spec(db).infer(db).as_callable(db) {
                    ctx.errors.push(
                        ControlFlowError::AssignCallableType {
                            typ: callable_typ,
                            access: call_site,
                        }
                        .to_diagnostic(db, ctx.scope.file(db)),
                    );
                    assignable = false;
                }
                // a CONSTANT variable cannot be assigned to
                if variable.qualifier(db).contains(crate::Qualifier::CONSTANT) {
                    ctx.errors.push(
                        ControlFlowError::AssignToConstant { access: call_site }
                            .to_diagnostic(db, ctx.scope.file(db)),
                    );
                    assignable = false;
                }
                assignable
            }
            Type::StructElement(_) => true,
            _ => self.check_not_direct_type(db, call_site, ctx),
        }
    }
}

impl<'db> CoerceError<'db> {
    pub fn into_non_assignable(
        self,
        db: &'db dyn WorkspaceDataBase,
        base_target: Type<'db>,
        call_site: CallSite<'db>,
    ) -> IdeDiagnostic {
        TypeError::NotAssignable {
            base_target,
            lhs: self.expected,
            rhs: self.actual,
            adjustment: self.adjustment,
            expr: call_site,
            suggest_cast: true,
        }
        .to_diagnostic(db, call_site.get_scope_id(db).file(db))
    }

    pub fn into_non_comparable(
        self,
        db: &'db dyn WorkspaceDataBase,
        base_target: Type<'db>,
        call_site: CallSite<'db>,
    ) -> IdeDiagnostic {
        TypeError::NotComparable {
            base_target,
            lhs: self.expected,
            rhs: self.actual,
            adjustment: self.adjustment,
            expr: call_site,
        }
        .to_diagnostic(db, call_site.get_scope_id(db).file(db))
    }

    pub fn into_non_addable(
        self,
        db: &'db dyn WorkspaceDataBase,
        base_target: Type<'db>,
        call_site: CallSite<'db>,
        operator: AddOperatorKind,
    ) -> IdeDiagnostic {
        TypeError::NotAddable {
            base_target,
            lhs: self.expected,
            operator,
            rhs: self.actual,
            adjustment: self.adjustment,
            expr: call_site,
        }
        .to_diagnostic(db, call_site.get_scope_id(db).file(db))
    }

    pub fn into_non_multiplicable(
        self,
        db: &'db dyn WorkspaceDataBase,
        base_target: Type<'db>,
        call_site: CallSite<'db>,
        operator: MultOperatorKind,
    ) -> IdeDiagnostic {
        TypeError::NotMultiplicable {
            base_target,
            lhs: self.expected,
            operator,
            rhs: self.actual,
            adjustment: self.adjustment,
            expr: call_site,
        }
        .to_diagnostic(db, call_site.get_scope_id(db).file(db))
    }

}
