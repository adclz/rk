// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::hir_ty::infer::normalize::{declared_capacity_of, string_capacity};
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::check::errors::e04_init::InitError;
use crate::check::errors::e14_config::{ConfigError, InputWriteRoute};
use crate::{
    CallSite, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e03_type::TypeError},
    hir_def::{
        expressions::expression::{AddOperatorKind, MultOperatorKind},
        pous::variable::LocationArea,
    },
    hir_ty::{
        body::{Adjustment, AdjustmentInfo, BodyInferenceResult},
        infer::Infer,
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
        // As the base holds it: `16#FF` on a SINT subrange is -1.
        let base = sub._type(db).infer(db);
        let value =
            crate::hir_ty::infer::const_eval::held_as(db, i128::from(rhs.as_const_int(db)?), base)
                as i64;
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

    /// AND, OR and XOR: logic on BOOL, bitwise on integers. A float has no
    /// bits to combine.
    pub fn supports_bool_op(&self, _db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_boolean() || (self.is_numeric() && !self.is_float())
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
        // (what they declared, a STRING's capacity, is read off `written`)
        let lhs = self.normalize(db);
        let written = to;
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
                // A STRING's capacity is part of that: a write through a
                // `REF_TO STRING` reaching a `STRING[4]` ran 76 bytes past it.
                Type::RefTo(spec) => {
                    let capacity_matches = declared_capacity_of(db, written)
                        .is_none_or(|c| string_capacity(db, spec) == Some(c));
                    return match same_type(db, spec.infer(db).normalize(db), to) && capacity_matches
                    {
                        true => Ok(()),
                        false => Err(CoerceError {
                            expected: *self,
                            actual: written,
                            adjustment: adjs.iter().last().cloned(),
                        }),
                    };
                }
                _ => {
                    return Err(CoerceError {
                        expected: *self,
                        actual: written,
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
                    actual: written,
                    adjustment: None,
                }),
            },
            (Type::Enum(e1), Type::Enum(e2)) => {
                if e1.eq(e2) {
                    Ok(())
                } else {
                    Err(CoerceError {
                        expected: *self,
                        actual: written,
                        adjustment: None,
                    })
                }
            }
            // same types are assignable
            (Type::Struct(s1), Type::Struct(s2)) => match s1.eq(s2) {
                true => Ok(()),
                false => Err(CoerceError {
                    expected: *self,
                    actual: written,
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
                        actual: written,
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
                            actual: written,
                            adjustment: None,
                        });
                    }
                }
                // The element type must be the SAME, not merely coercible: an
                // array copy moves bytes, it does not convert them, so an
                // `ARRAY OF INT` into an `ARRAY OF REAL` checked clean and
                // read back garbage (b[1] was not 2.0). A STRING element's
                // capacity is its size, so `STRING[4]` and `STRING` differ,
                // and so does a subrange: the copy checks no element, so an
                // `ARRAY OF INT` into an `ARRAY OF INT (0..10)` stored 50.
                match crate::hir_ty::head::checks::variables::same_storage_type(
                    db,
                    Type::Array(a1),
                    Type::Array(*a2),
                ) {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: written,
                        adjustment: None,
                    }),
                }
            }
            // No arm takes a value of the element type for the array: it
            // used to, and `a := 5` wrote `a[0]`, `y + a` reached lowering,
            // and an element read bare, `(a[1])`, passed for the array it
            // is in. An array takes an array of its storage type, above.
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
                        actual: written,
                        adjustment: None,
                    }),
                }
            }
            (Type::RefTo(_), Type::Null) => Ok(()),
            // Same invariance for a reference bound from a reference.
            (Type::RefTo(lhs), Type::RefTo(rhs)) => {
                match same_type(db, Type::RefTo(lhs), Type::RefTo(*rhs)) {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: written,
                        adjustment: None,
                    }),
                }
            }
            // An instance where its own POU is expected: a VAR_IN_OUT binds
            // it by reference, an assignment copies it whole. A reference it
            // holds is copied as it is, still pointing at the original
            // target.
            (Type::FunctionBlock(expected), Type::FunctionBlock(actual)) if expected == *actual => {
                Ok(())
            }
            (Type::Class(expected), Type::Class(actual)) if expected == *actual => Ok(()),
            // An FB or CLASS that implements the interface, itself or through
            // a base, or an interface that extends it.
            (
                Type::Interface(target),
                Type::Class(_) | Type::FunctionBlock(_) | Type::Interface(_),
            ) if to
                .as_pou(db)
                .is_some_and(|pou| crate::hir_ty::oop::ancestry(db, pou).implements(target)) =>
            {
                Ok(())
            }
            _ => Err(CoerceError {
                expected: *self,
                actual: written,
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
                // a CONSTANT variable cannot be assigned to
                if variable.qualifier(db).contains(crate::Qualifier::CONSTANT) {
                    ctx.errors.push(
                        InitError::AssignToConstant {
                            access: call_site,
                            constant: Some(*variable),
                        }
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
            // The callable's own variable.
            Type::ReturnValue(_) => true,
            // The own name of a callable with no return type, which has no
            // variable of that name to store into.
            Type::Function(_) | Type::MethodDecl(_)
                if let Some(callable) = self.as_callable(db)
                    && callable.return_type(db).is_none()
                    && callable.get_scope_id(db) == call_site.get_scope_id(db) =>
            {
                ctx.errors.push(
                    TypeError::AssignVoidResult {
                        callable,
                        access: call_site,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
                false
            }
            // A FUNCTION or METHOD named anywhere else: a callable has no
            // storage to write (E0318). An instance is a variable, copied
            // like any other.
            Type::Function(_) | Type::MethodDecl(_) => {
                let Some(callable) = self.as_callable(db) else {
                    return true;
                };
                ctx.errors.push(
                    TypeError::AssignFunctionOrMethod {
                        typ: callable,
                        access: call_site,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
                false
            }
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
/// spelled identically. Never consults the value-coercion table. A STRING
/// target or element keeps its capacity here: it is the size of what a
/// reference points at, and of each element.
pub(crate) fn same_type<'db>(db: &'db dyn WorkspaceDataBase, a: Type<'db>, b: Type<'db>) -> bool {
    match (a, b) {
        (Type::Elementary(x), Type::Elementary(y)) => x == y,
        (Type::RefTo(x), Type::RefTo(y)) => {
            same_type(db, x.infer(db).normalize(db), y.infer(db).normalize(db))
                && string_capacity(db, x) == string_capacity(db, y)
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
                && string_capacity(db, x.of_type(db)) == string_capacity(db, y.of_type(db))
        }
        (Type::ArrayConformand(x), Type::ArrayConformand(y)) => {
            x.rank(db) == y.rank(db)
                && match (x.of_type(db), y.of_type(db)) {
                    (Some(x), Some(y)) => {
                        same_type(db, x.infer(db).normalize(db), y.infer(db).normalize(db))
                    }
                    (None, None) => true,
                    _ => false,
                }
        }
        _ => a.eq(&b),
    }
}

/// An array as an `ARRAY[*]` parameter sees what a call binds to it: a whole
/// array, or a row of one (`m[i]` of a two-dimensional `m`).
#[derive(Clone, Debug)]
pub(crate) struct ArrayArgument<'db> {
    pub rank: usize,
    /// The element's declaration, none for an `ARRAY[*]` of any type.
    pub of_type: Option<crate::hir_def::expressions::spec::Spec<'db>>,
    /// The bounds it declares, each dimension's. None for an `ARRAY[*]`,
    /// whose bounds were checked where it was bound.
    pub bounds: Vec<(Option<i64>, Option<i64>)>,
}

impl<'db> ArrayArgument<'db> {
    /// A whole array, `None` for anything else.
    pub(crate) fn of(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<Self> {
        match ty.normalize(db) {
            Type::Array(array) => Some(ArrayArgument {
                rank: array.subranges(db).len(),
                of_type: Some(array.of_type(db)),
                bounds: crate::hir_ty::infer::const_eval::array_dimensions(db, array),
            }),
            Type::ArrayConformand(conformand) => Some(ArrayArgument {
                rank: conformand.rank(db),
                of_type: conformand.of_type(db),
                bounds: Vec::new(),
            }),
            _ => None,
        }
    }

    /// The row a bracket leaving dimensions names: the dimensions after
    /// the ones it subscripts.
    pub(crate) fn row(
        db: &'db dyn WorkspaceDataBase,
        indexed: crate::hir_ty::body::IndexedArray<'db>,
    ) -> Self {
        let bounds = match indexed.array {
            crate::hir_ty::body::IndexedType::Array(array) => {
                crate::hir_ty::infer::const_eval::array_dimensions(db, array)
                    .into_iter()
                    .skip(indexed.through)
                    .collect()
            }
            crate::hir_ty::body::IndexedType::Conformand(_) => Vec::new(),
        };
        ArrayArgument {
            rank: indexed.array.rank(db) - indexed.through,
            of_type: indexed.array.of_type(db),
            bounds,
        }
    }

    /// Whether every bound it declares fits the DINT `LOWER_BOUND` and
    /// `UPPER_BOUND` return.
    pub(crate) fn bounds_fit_dint(&self) -> bool {
        self.bounds.iter().all(|(lower, upper)| {
            [lower, upper]
                .iter()
                .all(|bound| bound.is_none_or(|bound| i32::try_from(bound).is_ok()))
        })
    }
}

/// Whether `arg` binds to the `ARRAY[*]` parameter `conformand`: an array of
/// as many dimensions, whatever their bounds, of the same element type. The
/// element's subrange counts, since a VAR_IN_OUT writes the caller's
/// elements. A STRING element's capacity does not: the parameter declares
/// none, and each call works on the one it was given. One of any type takes
/// any array. Either way, the bounds fit a DINT.
pub(crate) fn binds_conformand<'db>(
    db: &'db dyn WorkspaceDataBase,
    conformand: crate::hir_def::expressions::spec::ArrayConformand<'db>,
    arg: &ArrayArgument<'db>,
) -> bool {
    if !arg.bounds_fit_dint() {
        return false;
    }
    let Some(of_type) = conformand.of_type(db) else {
        return true;
    };
    arg.rank == conformand.rank(db)
        && arg.of_type.is_some_and(|arg_of_type| {
            crate::hir_ty::head::checks::variables::same_storage_type(
                db,
                Type::resolve_spec(db, of_type),
                Type::resolve_spec(db, arg_of_type),
            )
        })
}
