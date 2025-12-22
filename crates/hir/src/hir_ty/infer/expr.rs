use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    CallSite, HirNodeInfo,
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
        infer::inference_table::InferenceTable,
        resolver::{Resolver, func_call::resolve_func_call},
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

    /// Unifies all inferred types in the table
    ///
    /// /!\ This should be called after setting main constraints
    ///
    /// because it will replace all inferred types with their resolved types or Never
    pub fn resolve_completly(
        &mut self,
        db: &'db dyn BaseDatabase,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        self.inference_table
            .resolve_completly(db, self.resolver, ctx);
    }

    /// Gets the resolved type of the current inference context
    pub fn get_final_type(&self) -> Type<'db> {
        self.inference_table.get_final_type()
    }

    /// Sets the main constraint for the current inference
    ///
    /// That means all non-inferred types will be unified to this type
    pub fn set_target_type(
        &mut self,
        db: &'db dyn BaseDatabase,
        call_site: CallSite<'db>,
        expected: Type<'db>,
    ) {
        self.inference_table
            .set_target_type(db, call_site, expected);
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
                let (infer_ctx, result) =
                    left.coerce_with_expression(db, *right, self.resolver, inference_results);

                result
                    .map_err(|err| {
                        inference_results
                            .errors
                            .push(err.into_non_addable(db, expr, *operator))
                    })
                    .ok();

                let lhs = inference_results
                    .type_of_expr_with_adjustments(db, *left)
                    .unwrap_or_default();

                let rhs = inference_results
                    .type_of_expr_with_adjustments(db, *right)
                    .unwrap_or_default();

                if !lhs.normalize(db).is_numeric() || !rhs.normalize(db).is_numeric() {
                    inference_results.errors.push(
                        TypeError::NotAddable {
                            lhs,
                            operator: *operator,
                            rhs,
                            expr,
                        }
                        .to_diagnostic(db),
                    );
                };

                infer_ctx.get_final_type()
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                let (infer_ctx, result) =
                    left.coerce_with_expression(db, *right, self.resolver, inference_results);

                result
                    .map_err(|err| {
                        inference_results
                            .errors
                            .push(err.into_non_multiplicable(db, expr, *operator))
                    })
                    .ok();

                let lhs = inference_results
                    .type_of_expr_with_adjustments(db, *left)
                    .unwrap_or_default();

                let rhs = inference_results
                    .type_of_expr_with_adjustments(db, *right)
                    .unwrap_or_default();

                if !lhs.normalize(db).is_numeric() || !rhs.normalize(db).is_numeric() {
                    inference_results.errors.push(
                        TypeError::NotMultiplicable {
                            lhs,
                            operator: *operator,
                            rhs,
                            expr,
                        }
                        .to_diagnostic(db),
                    );
                };

                infer_ctx.get_final_type()
            }
            ExprKind::PowerOperator { left, right } => {
                let (infer_ctx, result) =
                    left.coerce_with_expression(db, *right, self.resolver, inference_results);

                result
                    .map_err(|err| {
                        inference_results
                            .errors
                            .push(err.into_non_comparable(db, expr))
                    })
                    .ok();

                infer_ctx.get_final_type()
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                // Adds a new inference ctx for boolean operators

                let lhs = InferExprCtx::new(self.resolver).infer_expr(db, *left, inference_results);
                if let Err(err) = lhs.coerce_with_type(db, Type::new_bool(), self.resolver) {
                    inference_results
                        .errors
                        .push(TypeError::NotABoolean { typ: lhs, expr }.to_diagnostic(db));
                }

                let rhs =
                    InferExprCtx::new(self.resolver).infer_expr(db, *right, inference_results);
                if let Err(err) = rhs.coerce_with_type(db, Type::new_bool(), self.resolver) {
                    inference_results
                        .errors
                        .push(TypeError::NotABoolean { typ: lhs, expr }.to_diagnostic(db));
                }

                // a boolean operator always returns a boolean
                Type::new_bool()
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                // A compare operation has it's own inference context since it returns a boolean
                let mut ctx = InferExprCtx::new(self.resolver);

                ctx.infer_expr(db, *left, inference_results);
                ctx.infer_expr(db, *right, inference_results);

                ctx.resolve_completly(db, inference_results);

                let lhs = inference_results
                    .type_of_expr_with_adjustments(db, *left)
                    .unwrap_or_default();

                let rhs = inference_results
                    .type_of_expr_with_adjustments(db, *right)
                    .unwrap_or_default();

                if let Err(err) = lhs.coerce_with_type(db, rhs, self.resolver) {
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
                    if !not_expr.normalize(db).is_boolean() {
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
                resolve_func_call(db, self.resolver, *call, infer_result)
            }
            PrimaryExpr::EnumValue { name, variant } => {
                let find_enm = self
                    .resolver
                    .resolve_begin_path_expr(db, *name, infer_result)
                    .normalize(db);

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
