use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, body_inference::TypeError},
    hir_def::{
        expressions::{
            expression::{
                ComparisonOperatorKind, Expr, ExprKind, PrimaryExpr, RefValue, UnaryOperatorKind,
            },
            spec::ElementarySpec,
            statement::Stmt,
        },
        interned::identifier::Ident,
        scope::{Scope, ScopeId},
    },
    hir_ty::{
        body_inference::BodyInferenceResult, infer::ctx::InferCtx, resolver::Resolver, ty::Type
    },
};
pub struct InferExprCtx<'db> {
    /// Current scope
    pub scope: ScopeId<'db>,
    /// Current scope type (POu with body, method, etc.)
    pub resolver: Resolver<'db>
}

impl<'db> InferExprCtx<'db> {
    pub fn new(scope: ScopeId<'db>, resolver: Resolver<'db>) -> Self {
        Self { scope, resolver }
    }

    pub fn infer_expr(
        &self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
        errors: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match expr.expr(db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, errors);
                let rhs = self.infer_expr(db, *right, errors);
                if lhs.coerce_with(db, rhs, self.scope) == false {
                    errors
                        .errors
                        .push(TypeError::NotAddable { lhs, operator: *operator, rhs, expr }.into());
                }
                lhs
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, errors);
                if lhs.coerce_with(db, Type::new_bool(), self.scope) == false {
                    errors
                        .errors
                        .push(TypeError::NotABoolean { typ: lhs, expr }.into());
                }
                let rhs = self.infer_expr(db, *right, errors);
                if rhs.coerce_with(db, Type::new_bool(), self.scope) == false {
                    errors
                        .errors
                        .push(TypeError::NotABoolean { typ: lhs, expr }.into());
                }
                Type::new_bool()
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, errors);
                let rhs = self.infer_expr(db, *right, errors);
                if lhs.coerce_with(db, rhs, self.scope) == false {
                    errors
                        .errors
                        .push(TypeError::NotComparable { lhs, rhs, expr }.into());
                }
                Type::new_bool()
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, errors);
                let rhs = self.infer_expr(db, *right, errors);
                if lhs.coerce_with(db, rhs, self.scope) == false {
                    errors
                        .errors
                        .push(TypeError::NotMultiplicable { lhs, operator: *operator, rhs, expr }.into());
                }
                lhs
            }
            ExprKind::PowerOperator { left, right } => {
                let lhs = self.infer_expr(db, *left, errors);
                let rhs = self.infer_expr(db, *right, errors);

                if lhs.coerce_with(db, rhs, self.scope) == false {
                    errors
                        .errors
                        .push(TypeError::NotComparable { lhs, rhs, expr }.into());
                }
                lhs
            }
            ExprKind::UnaryOperator { expr, operator } => match operator {
                UnaryOperatorKind::Not => {
                    let not_expr = self.infer_expr(db, *expr, errors);
                    if !not_expr.is_boolean() {
                        errors.errors.push(
                            TypeError::NotABoolean {
                                typ: not_expr,
                                expr: *expr,
                            }
                            .into(),
                        );
                    }
                    Type::new_bool()
                }
                _ => self.infer_expr(db, *expr, errors),
            },
            ExprKind::PrimaryExpr(primary) => self.infer_primary(db, expr, primary, errors),
        }
    }

    pub fn infer_primary(
        &self,
        db: &'db dyn BaseDatabase,
        base_expr: Expr<'db>,
        to: &PrimaryExpr<'db>,
        infer_result: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match to {
            PrimaryExpr::Literal(prim) => (*prim).into(),
            PrimaryExpr::VariableAccess(v) => {
                self.resolver.resolve_variable_access(db, *v, infer_result)
            }
            PrimaryExpr::FuncCall(call) => {
                InferCtx::resolve_func_call(db, self.resolver, *call, infer_result)
            }
            PrimaryExpr::EnumValue { name, variant } => {
                match self.resolver.resolve_begin_path_expr(db, *name, infer_result) {
                    Type::Enum(enm) => enm
                        .variants(db)
                        .iter()
                        .find(|v| *v.name == **variant)
                        .map(|v| Type::EnumVariant(*v.name))
                        .unwrap_or_else(|| Type::Never),
                    _ => Type::Never,
                }
            }
            PrimaryExpr::RefValue { value } => match value {
                RefValue::Address(adress) => {
                    self.resolver
                        .resolve_begin_path_expr(db, *adress, infer_result)
                }
                RefValue::Null => Type::Null,
            },
            PrimaryExpr::ParenthesizedExpr { expr } => self.infer_expr(db, *expr, infer_result),
        }
    }
}
