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
    pub fn supports_add(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_numeric()
            || self.is_time()
            || match self {
                Type::Generic(generic) => generic.is_numeric(db) || generic.is_time(db),
                _ => false,
            }
    }

    pub fn supports_mul(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_numeric()
            || match self {
                Type::Generic(generic) => generic.is_numeric(db),
                _ => false,
            }
    }

    pub fn supports_div(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_numeric()
            || match self {
                Type::Generic(generic) => generic.is_numeric(db),
                _ => false,
            }
    }

    pub fn supports_mod(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_signed_integer()
            || self.is_unsigned_integer()
            || match self {
                Type::Generic(generic) => {
                    generic.is_signed_integer(db) || generic.is_unsigned_integer(db)
                }
                _ => false,
            }
    }

    pub fn supports_power(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_float()
            || match self {
                Type::Generic(generic) => generic.is_float(db),
                _ => false,
            }
    }

    pub fn supports_bool_op(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.is_boolean()
            || self.is_numeric()
            || match self {
                Type::Generic(generic) => generic.is_numeric(db),
                _ => false,
            }
    }

    pub fn supports_comparison(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self, Type::Elementary(_)) || matches!(self, Type::Generic(_))
    }

    pub fn can_be_variadic(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        matches!(self, Type::Elementary(_))
            || match self {
                Type::Generic(generic) => generic.is_numeric(db) || generic.is_time(db),
                _ => false,
            }
    }

    // Type coercion check
    pub fn coerce_with_type(
        &self,
        db: &'db dyn WorkspaceDataBase,
        to: Type<'db>,
        adjustments: Option<&[Adjustment<'db>]>,
        resolver: Resolver<'db>,
    ) -> CoerceResult<'db> {
        // We return true if the lhs or rhs is of type never.
        // that's because Never variants are already reported by the resolver and we don't want to propagate too many errors

        if self.is_never() || to.is_never() {
            return Ok(());
        }

        // types *must* not be infer variants during coercion
        debug_assert!(!self.has_infer());
        debug_assert!(!to.has_infer());

        // normalizing here is necessary here to avoid matching on wrapped types
        let lhs = self.normalize(db);
        let to = to.normalize(db);

        // use the adjustments to allow coercions for references,
        // but only if the reference can be dereferenced to the expected type
        if let Some(adjs) = adjustments
            && let Some(typ) = adjs.as_reference()
        {
            if let Type::RefTo(spec) = lhs {
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
            } else {
                return Err(CoerceError {
                    expected: *self,
                    actual: to,
                    adjustment: adjs.iter().last().cloned(),
                });
            }
        }

        match (lhs, &to) {
            // variant is already solved by the resolver
            (Type::Enum(e1), Type::EnumVariant(e2)) => Ok(()),
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
            (Type::Array(a1), Type::Array(a2)) => match a1.eq(a2) {
                true => Ok(()),
                false => Err(CoerceError {
                    expected: *self,
                    actual: to,
                    adjustment: None,
                }),
            },
            // check array spec equality
            (Type::Array(a1), rhs) => {
                a1.of_type(db)
                    .infer(db)
                    .coerce_with_type(db, *rhs, adjustments, resolver)
            }
            // check subrange base type equality
            (Type::SubRange(sub), rhs) => {
                sub._type(db)
                    .infer(db)
                    .coerce_with_type(db, *rhs, adjustments, resolver)
            }
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
            // Generic types: check constraint compatibility with concrete types
            (Type::Elementary(elem), Type::Generic(generic)) => {
                if let Some(any) = generic.as_builtin_generic(db)
                    && any.contains(elem)
                {
                    return Ok(());
                }
                Err(CoerceError {
                    expected: *self,
                    actual: to,
                    adjustment: None,
                })
            }
            (Type::Generic(generic), Type::Elementary(elem)) => {
                if let Some(any) = generic.as_builtin_generic(db)
                    && any.contains(*elem)
                {
                    return Ok(());
                }
                Err(CoerceError {
                    expected: *self,
                    actual: to,
                    adjustment: None,
                })
            }
            // Generic ↔ Generic: always allow within bodies
            // (actual compatibility checked at instantiation/call site)
            (Type::Generic(_), Type::Generic(_)) => Ok(()),
            (Type::RefTo(_), Type::Null) => Ok(()),
            (Type::RefTo(lhs), Type::RefTo(rhs)) => {
                lhs.infer(db)
                    .coerce_with_type(db, rhs.infer(db), None, resolver)
            }
            // self-assignments
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

    /// Check that an assignment attempt (?=) is valid.
    /// LHS must be REF_TO, RHS must be REF_TO or Interface.
    pub fn coerce_assign_attempt(
        &self,
        db: &'db dyn WorkspaceDataBase,
        rhs: Type<'db>,
    ) -> CoerceResult<'db> {
        if self.is_never() || rhs.is_never() {
            return Ok(());
        }

        let lhs = self.normalize(db);
        let rhs = rhs.normalize(db);

        // LHS must be REF_TO (already checked at the call site with a dedicated error)
        // RHS must be REF_TO or Interface
        match (&lhs, &rhs) {
            (Type::RefTo(_), Type::RefTo(_)) => Ok(()),
            (Type::RefTo(_), Type::Interface(_)) => Ok(()),
            _ => Err(CoerceError {
                expected: *self,
                actual: rhs,
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
            .to_diagnostic(db),
        );
        false
    }

    pub fn check_assignable(
        &self,
        db: &'db dyn WorkspaceDataBase,
        call_site: CallSite<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        if self.is_never() {
            return;
        }

        match self {
            Type::Variable((variable, multibits)) => {
                // a variable of kind INPUT cannot be assigned to
                if variable.is_input(db) {
                    ctx.errors.push(
                        ControlFlowError::IsVarInput {
                            var: *variable,
                            access: call_site,
                        }
                        .to_diagnostic(db),
                    );
                }

                // a variable of callable type cannot be assigned to
                if let Some(callable_typ) = variable.spec(db).infer(db).as_callable(db) {
                    ctx.errors.push(
                        ControlFlowError::AssignCallableType {
                            typ: callable_typ,
                            access: call_site,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
            Type::StructElement(element) => (),
            _ => {
                self.check_not_direct_type(db, call_site, ctx);
            }
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
        }
        .to_diagnostic(db)
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
        .to_diagnostic(db)
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
        .to_diagnostic(db)
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
        .to_diagnostic(db)
    }

    pub fn into_non_powerable(
        self,
        db: &'db dyn WorkspaceDataBase,
        base_target: Type<'db>,
        call_site: CallSite<'db>,
    ) -> IdeDiagnostic {
        TypeError::NotPowerable {
            base_target,
            lhs: self.expected,
            rhs: self.actual,
            adjustment: self.adjustment,
            expr: call_site,
        }
        .to_diagnostic(db)
    }
}
