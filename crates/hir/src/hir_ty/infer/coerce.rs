use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        body_inference::{BodyInferenceError, TypeError},
    },
    hir_def::{
        expressions::{
            expression::{AddOperatorKind, Expr, MultOperatorKind, PathExpr, VariableAccess},
            spec::{ElementarySpec, Spec},
        },
        scope::ScopeId,
    },
    hir_ty::{
        body_inference::BodyInferenceResult,
        infer::expr::InferExprCtx,
        resolver::Resolver,
        ty::{InferType, Type},
    },
};

pub struct CoerceError<'db> {
    pub expected: Type<'db>,
    pub actual: Type<'db>,
}

pub type CoerceResult<'db> = Result<(), CoerceError<'db>>;

impl<'db> Type<'db> {
    // Type coercion check
    #[must_use]
    pub fn coerce_with_type(
        &self,
        db: &'db dyn BaseDatabase,
        to: Type<'db>,
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

        // shallowing here is necessary here to avoid matching on wrapped types
        let lhs = self.normalize(db);
        let to = to.normalize(db);

        match (lhs, &to) {
            // variant is already solved by the resolver
            (Type::Enum(e1), Type::EnumVariant(e2)) => Ok(()),
            // same types are assignable
            (Type::Struct(s1), Type::Struct(s2)) => {
                return match s1.eq(s2) {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                    }),
                };
            }
            // check element spec equality
            (Type::StructElement(elem), rhs) => {
                Type::new_spec(db, elem.spec(db)).coerce_with_type(db, *rhs, resolver)
            }
            // same types are assignable
            (Type::Array(a1), Type::Array(a2)) => {
                return match a1.eq(a2) {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                    }),
                };
            }
            // check array spec equality
            (Type::Array(a1), rhs) => {
                Type::new_spec(db, a1.of_type(db)).coerce_with_type(db, *rhs, resolver)
            }
            // check subrange base type equality
            (Type::SubRange(sub), rhs) => {
                Type::new_spec(db, sub._type(db)).coerce_with_type(db, *rhs, resolver)
            }
            (Type::Elementary(lhs), Type::Elementary(rhs)) => {
                if lhs == *rhs {
                    return Ok(());
                }
                // try implicit conversions in both directions
                match !lhs.implicit_cast(*rhs).is_none() || !rhs.implicit_cast(lhs).is_none() {
                    true => Ok(()),
                    false => Err(CoerceError {
                        expected: *self,
                        actual: to,
                    }),
                }
            }
            (Type::RefTo(_), Type::Null) => Ok(()),
            _ => Err(CoerceError {
                expected: *self,
                actual: to,
            }),
        }
    }

    pub fn coerce_with_expression(
        &self,
        db: &'db dyn BaseDatabase,
        call_site: CallSite<'db>,
        expr: Expr<'db>,
        resolver: Resolver<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> CoerceResult<'db> {
        let mut infer_ctx = InferExprCtx::new(resolver.clone());

        infer_ctx.infer_expr(db, expr, ctx);

        infer_ctx.set_target_type(db, call_site, *self);
        infer_ctx.resolve_completly(db, ctx);

        let rhs_ty = ctx
            .type_of_expr_with_adjustments(db, expr)
            .unwrap_or_default();

        self.coerce_with_type(db, rhs_ty, infer_ctx.resolver)
    }

    pub fn is_assignable(
        &self,
        db: &'db dyn BaseDatabase,
        call_site: CallSite<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        // Additional checks for variable assignments
        if let Type::Variable(variable) = self {
            // a variable of kind INPUT cannot be assigned to
            if variable.is_input(db) {
                ctx.errors.push(
                    BodyInferenceError::IsVarInput {
                        var: *variable,
                        access: call_site,
                    }
                    .to_diagnostic(db),
                );
            }

            // a variable of callable type cannot be assigned to
            if let Some(callable_typ) = Type::new_var(db, *variable).as_callable(db) {
                ctx.errors.push(
                    BodyInferenceError::AssignCallableType {
                        typ: callable_typ,
                        access: call_site,
                    }
                    .to_diagnostic(db),
                );
                return;
            }
        }
        // type is not a variable
        else {
            // function and methods can be assigned IF they are the same
            let ok = match self {
                Type::Function(f) => f.get_scope_id(db) == call_site.get_scope_id(db),
                Type::MethodDecl(m) => m.get_scope_id(db) == call_site.get_scope_id(db),
                _ => false,
            };
            if !ok {
                ctx.errors.push(
                    BodyInferenceError::DirectType {
                        expr: call_site,
                        typ: *self,
                    }
                    .to_diagnostic(db),
                );
                return;
            }
        }
    }
}

impl<'db> Expr<'db> {
    pub fn coerce_with_expression(
        &self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
        resolver: Resolver<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> (InferExprCtx<'db>, CoerceResult<'db>) {
        let mut infer_ctx = InferExprCtx::new(resolver.clone());

        infer_ctx.infer_expr(db, *self, ctx);
        infer_ctx.infer_expr(db, expr, ctx);

        infer_ctx.resolve_completly(db, ctx);

        let self_ty = ctx
            .type_of_expr_with_adjustments(db, *self)
            .unwrap_or_default();

        let rhs_ty = ctx
            .type_of_expr_with_adjustments(db, expr)
            .unwrap_or_default();

        let resolver = infer_ctx.resolver.clone();
        (infer_ctx, self_ty.coerce_with_type(db, rhs_ty, resolver))
    }
}

impl<'db> CoerceError<'db> {
    pub fn into_non_assignable(
        self,
        db: &'db dyn BaseDatabase,
        base_target: Type<'db>,
        expr: Expr<'db>,
    ) -> IdeDiagnostic {
        TypeError::NotAssignable {
            base_target,
            target: self.expected,
            value: self.actual,
            expr: CallSite::from_expr(db, expr),
        }
        .to_diagnostic(db)
    }

    pub fn into_non_comparable(self, db: &'db dyn BaseDatabase, expr: Expr<'db>) -> IdeDiagnostic {
        TypeError::NotComparable {
            lhs: self.expected,
            rhs: self.actual,
            expr,
        }
        .to_diagnostic(db)
    }

    pub fn into_non_addable(
        self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
        operator: AddOperatorKind,
    ) -> IdeDiagnostic {
        TypeError::NotAddable {
            lhs: self.expected,
            operator,
            rhs: self.actual,
            expr,
        }
        .to_diagnostic(db)
    }

    pub fn into_non_multiplicable(
        self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
        operator: MultOperatorKind,
    ) -> IdeDiagnostic {
        TypeError::NotMultiplicable {
            lhs: self.expected,
            operator,
            rhs: self.actual,
            expr,
        }
        .to_diagnostic(db)
    }
}

pub fn unify_var_access<'db>(
    db: &'db dyn BaseDatabase,
    target: Type<'db>,
    var: VariableAccess<'db>,
    resolver: Resolver<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let mut infer_ctx = InferExprCtx::new(resolver.clone());
    let _ = infer_ctx.resolver.resolve_variable_access(db, var, ctx);
    infer_ctx.set_target_type(db, CallSite::from_var_access(db, var), target);
    infer_ctx.resolve_completly(db, ctx);

    let value = ctx
        .type_of_variable_access_with_adjustments(db, var)
        .unwrap_or_default();

    if let Err(err) = target.coerce_with_type(db, value, infer_ctx.resolver) {
        ctx.errors.push(
            TypeError::NotAssignable {
                base_target: target,
                target: err.expected,
                value: err.actual,
                expr: CallSite::from_var_access(db, var),
            }
            .to_diagnostic(db),
        )
    };
}
