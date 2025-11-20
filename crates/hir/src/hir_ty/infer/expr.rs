use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::analysis_error::ToIdeDiagnostic,
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
        infer::ctx::InferCtx,
        inference::{InferenceError, InferenceResult},
        ty2::Type,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum InferError<'db> {
    NotAssignable {
        target: Type<'db>,
        value: Type<'db>,
        expr: Expr<'db>,
    },
    NotComparable {
        lhs: Type<'db>,
        rhs: Type<'db>,
        expr: Expr<'db>,
    },
    NotMultiplicable {
        lhs: Type<'db>,
        rhs: Type<'db>,
        expr: Expr<'db>,
    },
    NotAddable {
        lhs: Type<'db>,
        rhs: Type<'db>,
        expr: Expr<'db>,
    },
    NotABoolean {
        typ: Type<'db>,
        expr: Expr<'db>,
    },
    UnknownVariable {
        name: Ident,
        expr: Expr<'db>,
    },
    Other(String),
}

impl<'db> ToIdeDiagnostic<'db> for InferError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::NotAssignable {
                target,
                value,
                expr,
            } => diag()
                .message(format!(
                    "expected {}, got {}",
                    target.full_type_name(db),
                    value.full_type_name(db)
                ))
                .range(expr.get_span(db))
                .call(),
            _ => todo!(),
        }
    }
}

pub struct InferExprCtx<'db> {
    /// Current scope
    pub scope: ScopeId<'db>,
    /// Current scope type (POu with body, method, etc.)
    pub scope_typ: Type<'db>,
}

impl<'db> InferExprCtx<'db> {
    pub fn new(scope: ScopeId<'db>, scope_typ: Type<'db>) -> Self {
        Self { scope, scope_typ }
    }

    pub fn infer_expr(
        &self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
        infer_result: &mut InferenceResult<'db>,
    ) -> Type<'db> {
        match expr.expr(db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, infer_result);
                let rhs = self.infer_expr(db, *right, infer_result);
                if lhs.coerce_with(db, rhs, self.scope) == false {
                    infer_result
                        .errors
                        .push(InferError::NotAddable { lhs, rhs, expr }.into());
                }
                lhs
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, infer_result);
                if lhs.coerce_with(db, Type::new_bool(), self.scope) == false {
                    infer_result
                        .errors
                        .push(InferError::NotABoolean { typ: lhs, expr }.into());
                }
                let rhs = self.infer_expr(db, *right, infer_result);
                if rhs.coerce_with(db, Type::new_bool(), self.scope) == false {
                    infer_result
                        .errors
                        .push(InferError::NotABoolean { typ: lhs, expr }.into());
                }
                Type::new_bool()
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, infer_result);
                let rhs = self.infer_expr(db, *right, infer_result);
                if lhs.coerce_with(db, rhs, self.scope) == false {
                    infer_result
                        .errors
                        .push(InferError::NotComparable { lhs, rhs, expr }.into());
                }
                Type::new_bool()
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, infer_result);
                let rhs = self.infer_expr(db, *right, infer_result);
                if lhs.coerce_with(db, rhs, self.scope) == false {
                    infer_result
                        .errors
                        .push(InferError::NotMultiplicable { lhs, rhs, expr }.into());
                }
                lhs
            }
            ExprKind::PowerOperator { left, right } => {
                let lhs = self.infer_expr(db, *left, infer_result);
                let rhs = self.infer_expr(db, *right, infer_result);

                if lhs.coerce_with(db, rhs, self.scope) == false {
                    infer_result
                        .errors
                        .push(InferError::NotComparable { lhs, rhs, expr }.into());
                }
                lhs
            }
            ExprKind::UnaryOperator { expr, operator } => match operator {
                UnaryOperatorKind::Not => {
                    let not_expr = self.infer_expr(db, *expr, infer_result);
                    if !not_expr.is_boolean() {
                        infer_result.errors.push(
                            InferError::NotABoolean {
                                typ: not_expr,
                                expr: *expr,
                            }
                            .into(),
                        );
                    }
                    Type::new_bool()
                }
                _ => self.infer_expr(db, *expr, infer_result),
            },
            ExprKind::PrimaryExpr(primary) => self.infer_primary(db, expr, primary, infer_result),
        }
    }

    pub fn infer_primary(
        &self,
        db: &'db dyn BaseDatabase,
        base_expr: Expr<'db>,
        to: &PrimaryExpr<'db>,
        infer_result: &mut InferenceResult<'db>,
    ) -> Type<'db> {
        match to {
            PrimaryExpr::Literal(prim) => (*prim).into(),
            PrimaryExpr::VariableAccess(v) => {
                self.scope_typ.walk_variable_access(db, *v, infer_result)
            }
            PrimaryExpr::FuncCall(call) => {
                InferCtx::resolve_func_call(db, self.scope_typ, *call, infer_result)
            }
            PrimaryExpr::EnumValue { name, variant } => {
                match self.scope_typ.walk_begin_path_expr(db, *name, infer_result) {
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
                    self.scope_typ
                        .walk_begin_path_expr(db, *adress, infer_result)
                }
                RefValue::Null => Type::Null,
            },
            PrimaryExpr::ParenthesizedExpr { expr } => self.infer_expr(db, *expr, infer_result),
        }
    }
}
