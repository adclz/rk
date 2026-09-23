use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::check::errors::e04_init::InitError;
use crate::check::errors::e14_config::{ConfigError, InputWriteRoute};
use crate::{
    CallSite, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e03_type::TypeError},
    hir_def::{
        expressions::expression::{AddOperatorKind, MultOperatorKind},
        pous::pou::Pou,
        pous::variable::LocationArea,
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
    /// declared bounds narrow it further, so `i := 5000` on
    /// `INT (-4095..4095)` is refused at compile time. Bounds are statically
    /// known, so a constant can be settled here; a non-constant would need a
    /// runtime guard, which IEC leaves optional.
    ///
    /// Returns the error rather than reporting it, like [`Self::coerce_with_type`] —
    /// the caller decides whether to surface it.
    pub fn subrange_violation(
        &self,
        db: &'db dyn WorkspaceDataBase,
        rhs: crate::hir_def::expressions::expression::Expr<'db>,
    ) -> Option<crate::check::errors::e07_subrange::SubRangeError<'db>> {
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
            crate::check::errors::e07_subrange::SubRangeError::ValueOutOfRange {
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
                // Reference binding is INVARIANT: the pointee must be the
                // declared type exactly. The value table has no say here —
                // widening a REF(int) into a REF_TO REAL reads a 2-byte slot
                // as 4 and typed the load wrong (invalid wasm at exit 0).
                Type::RefTo(spec) => {
                    return match same_type(db, spec.infer(db).normalize(db), to) {
                        true => Ok(()),
                        false => Err(CoerceError {
                            expected: *self,
                            actual: to,
                            adjustment: adjs.iter().last().cloned(),
                        }),
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
            (Type::Enum(e1), Type::EnumVariant(dt, _)) => match Type::DataType(*dt).normalize(db) {
                Type::Enum(e2) if e1.eq(&e2) => Ok(()),
                _ => Err(CoerceError {
                    expected: *self,
                    actual: to,
                    adjustment: None,
                }),
            },
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
                // The element type must be the SAME, not merely coercible: an
                // array copy moves bytes, it does not convert them, so an
                // `ARRAY OF INT` into an `ARRAY OF REAL` checked clean and
                // read back garbage (b[1] was not 2.0).
                match same_type(
                    db,
                    a1.of_type(db).infer(db).normalize(db),
                    a2.of_type(db).infer(db).normalize(db),
                ) {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    }),
                }
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
            // Same invariance for a reference bound from a reference.
            (Type::RefTo(lhs), Type::RefTo(rhs)) => {
                match same_type(db, lhs.infer(db).normalize(db), rhs.infer(db).normalize(db)) {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                        adjustment: None,
                    }),
                }
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
            // (E0318), not here.
            (Type::FunctionBlock(expected), Type::FunctionBlock(actual)) if expected == *actual => {
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
            TypeError::DirectType {
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
                        TypeError::AssignCallableType {
                            typ: callable_typ,
                            access: call_site,
                        }
                        .to_diagnostic(db, ctx.scope.file(db)),
                    );
                    assignable = false;
                }
                // A CLASS may be assigned, but one holding a variable
                // declared `AT %I*` would copy where that variable points
                // along with its values (E1427).
                if let Type::Class(class) = variable.spec(db).infer(db).normalize(db)
                    && let Some(path) = crate::hir_ty::head::inheritance::partly_located_members(
                        db,
                        Pou::Class(class),
                    )
                    .first()
                {
                    ctx.errors.push(
                        ConfigError::PartlyLocatedOverwritten {
                            site: call_site,
                            member: compact_str::CompactString::from(
                                path.iter()
                                    .map(|m| m.name(db).text(db).to_string())
                                    .collect::<Vec<_>>()
                                    .join("."),
                            ),
                            address: path
                                .last()
                                .and_then(|m| m.location(db))
                                .map(|dv| compact_str::CompactString::from(dv.to_address(db)))
                                .unwrap_or_default(),
                            how: crate::check::errors::e14_config::PartlyOverwrite::Copy {
                                ty: class.name(db).text(db).clone(),
                            },
                        }
                        .to_diagnostic(db, ctx.scope.file(db)),
                    );
                    assignable = false;
                }
                // a CONSTANT variable cannot be assigned to
                if variable.qualifier(db).contains(crate::Qualifier::CONSTANT) {
                    ctx.errors.push(
                        InitError::AssignToConstant { access: call_site }
                            .to_diagnostic(db, ctx.scope.file(db)),
                    );
                    assignable = false;
                }
                // The host owns the input band: it copies the process image
                // in before every scan, so a write the program makes is gone
                // before anything can read it.
                if let Some(dv) = crate::hir_ty::index_graphs::effective_location(db, *variable)
                    && dv.area(db) == Some(LocationArea::Input)
                {
                    ctx.errors.push(
                        ConfigError::WriteToInputLocation {
                            site: call_site,
                            address: compact_str::CompactString::from(dv.to_address(db)),
                            via: InputWriteRoute::Assignment,
                        }
                        .to_diagnostic(db, ctx.scope.file(db)),
                    );
                    assignable = false;
                }
                assignable
            }
            // A bare address is storage too, and `%I` is the host's: the
            // copy-in before the next scan overwrites whatever a program
            // stored there (E1419).
            Type::DirectVariable((dv, _)) => {
                if dv.area(db) != Some(LocationArea::Input) {
                    return true;
                }
                ctx.errors.push(
                    ConfigError::WriteToInputLocation {
                        site: call_site,
                        address: compact_str::CompactString::from(dv.to_address(db)),
                        via: InputWriteRoute::Assignment,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
                false
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

    /// As [`into_non_assignable`](Self::into_non_assignable), for an
    /// initializer. No cast is offered: an initializer takes a constant, so
    /// a conversion cannot be called there.
    pub fn into_non_assignable_init(
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
            suggest_cast: false,
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

/// Structural "is exactly this type" for reference pointees, on NORMALIZED
/// types. Identity for nominal types (structs, enums, POUs); shape for
/// arrays and references, whose specs are distinct salsa values even when
/// spelled identically. Never consults the value-coercion table.
pub(crate) fn same_type<'db>(db: &'db dyn WorkspaceDataBase, a: Type<'db>, b: Type<'db>) -> bool {
    match (a, b) {
        (Type::Elementary(x), Type::Elementary(y)) => x == y,
        (Type::RefTo(x), Type::RefTo(y)) => {
            same_type(db, x.infer(db).normalize(db), y.infer(db).normalize(db))
        }
        (Type::Array(x), Type::Array(y)) => {
            if x.eq(&y) {
                return true;
            }
            let dx = crate::hir_ty::infer::const_eval::array_dimensions(db, x);
            let dy = crate::hir_ty::infer::const_eval::array_dimensions(db, y);
            dx.len() == dy.len()
                && dx
                    .iter()
                    .zip(dy.iter())
                    .all(|(r1, r2)| r1.0.is_some() && r1.0 == r2.0 && r1.1 == r2.1)
                && same_type(
                    db,
                    x.of_type(db).infer(db).normalize(db),
                    y.of_type(db).infer(db).normalize(db),
                )
        }
        _ => a.eq(&b),
    }
}
