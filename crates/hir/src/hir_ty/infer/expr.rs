use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        body_inference::{BodyInferenceError, TypeError},
    },
    hir_def::{
        expressions::{
            expression::{
                ComparisonOperatorKind, Elementary, Expr, ExprKind, PrimaryExpr, RefValue,
                UnaryOperatorKind,
            },
            spec::ElementarySpec,
            statement::Stmt,
        },
        interned::identifier::Ident,
        scope::{Scope, ScopeId},
    },
    hir_ty::{
        body_inference::BodyInferenceResult,
        infer::{coerce::InferenceTable, ctx::InferCtx},
        resolver::Resolver,
        ty::Type,
    },
};
pub struct InferExprCtx<'db> {
    pub resolver: Resolver<'db>,
    pub inference_table: InferenceTable<'db>,
}

impl<'db> InferExprCtx<'db> {
    pub fn new(resolver: Resolver<'db>) -> Self {
        Self {
            resolver,
            inference_table: InferenceTable::new(),
        }
    }

    pub fn infer_expr(
        &mut self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match expr.expr(db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, inference_results);
                let rhs = self.infer_expr(db, *right, inference_results);

                self.inference_table
                    .resolve_completly(db, self.resolver, inference_results);

                let lhs = inference_results
                    .type_of_expr
                    .get(&expr)
                    .copied()
                    .unwrap_or_default();
                let rhs = inference_results
                    .type_of_expr
                    .get(&expr)
                    .copied()
                    .unwrap_or_default();
                if let Err(err) = lhs.coerce_with(db, rhs, self.resolver) {
                    inference_results.errors.push(
                        TypeError::NotAddable {
                            lhs: err.expected,
                            operator: *operator,
                            rhs: err.actual,
                            expr,
                        }
                        .to_diagnostic(db),
                    );
                }
                lhs
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                let lhs = self.infer_expr(db, *left, inference_results);
                let rhs = self.infer_expr(db, *right, inference_results);
                if let Err(err) = lhs.coerce_with(db, rhs, self.resolver) {
                    inference_results.errors.push(
                        TypeError::NotMultiplicable {
                            lhs: err.expected,
                            operator: *operator,
                            rhs: err.actual,
                            expr,
                        }
                        .to_diagnostic(db),
                    );
                }
                lhs
            }
            ExprKind::PowerOperator { left, right } => {
                let lhs = self.infer_expr(db, *left, inference_results);
                let rhs = self.infer_expr(db, *right, inference_results);

                if let Err(err) = lhs.coerce_with(db, rhs, self.resolver) {
                    inference_results.errors.push(
                        TypeError::NotComparable {
                            lhs: err.expected,
                            rhs: err.actual,
                            expr,
                        }
                        .to_diagnostic(db),
                    );
                }
                lhs
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                // Adds a new inference ctx for boolean operators

                let lhs = InferExprCtx::new(self.resolver).infer_expr(db, *left, inference_results);
                if let Err(err) = lhs.coerce_with(db, Type::new_bool(), self.resolver) {
                    inference_results
                        .errors
                        .push(TypeError::NotABoolean { typ: lhs, expr }.to_diagnostic(db));
                }

                let rhs =
                    InferExprCtx::new(self.resolver).infer_expr(db, *right, inference_results);
                if let Err(err) = rhs.coerce_with(db, Type::new_bool(), self.resolver) {
                    inference_results
                        .errors
                        .push(TypeError::NotABoolean { typ: lhs, expr }.to_diagnostic(db));
                }
                Type::new_bool()
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                // A compare operation has it's own inference context since it returns a boolean
                let mut ctx = InferExprCtx::new(self.resolver);

                let lhs = ctx.infer_expr(db, *left, inference_results);
                let rhs = ctx.infer_expr(db, *right, inference_results);

                if let Err(err) = lhs.coerce_with(db, rhs, self.resolver) {
                    inference_results.errors.push(
                        TypeError::NotComparable {
                            lhs: err.expected,
                            rhs: err.actual,
                            expr,
                        }
                        .to_diagnostic(db),
                    );
                }
                Type::new_bool()
            }
            ExprKind::UnaryOperator { expr, operator } => match operator {
                UnaryOperatorKind::Not => {
                    let mut ctx = InferExprCtx::new(self.resolver);

                    let not_expr = ctx.infer_expr(db, *expr, inference_results);
                    if !not_expr.is_boolean() {
                        inference_results.errors.push(
                            TypeError::NotABoolean {
                                typ: not_expr,
                                expr: *expr,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    Type::new_bool()
                }
                _ => self.infer_expr(db, *expr, inference_results),
            },
            ExprKind::PrimaryExpr(primary) => {
                let typ = self.infer_primary(db, expr, primary, inference_results);
                inference_results.type_of_expr.insert(expr, typ);
                self.inference_table.add_type(db, expr, typ, self.resolver);
                typ
            }
        }
    }

    pub fn infer_primary(
        &mut self,
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
                let find_enm = self
                    .resolver
                    .resolve_begin_path_expr(db, *name, infer_result)
                    .shallow(db);

                match find_enm {
                    Type::Enum(enm) => enm
                        .variants(db)
                        .iter()
                        .find(|v| *v.name == **variant)
                        .map(|v| Type::EnumVariant(*v.name))
                        .unwrap_or_else(|| {
                            // variant not found
                            infer_result.errors.push(
                                BodyInferenceError::EnumVariantNotFound {
                                    enum_: enm,
                                    variant_name: *variant,
                                }
                                .to_diagnostic(db),
                            );
                            Type::Never
                        }),
                    _ => {
                        // not an enum
                        infer_result.errors.push(
                            BodyInferenceError::NotAnEnum {
                                expr: *name,
                                item: find_enm,
                            }
                            .to_diagnostic(db),
                        );
                        Type::Never
                    }
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
