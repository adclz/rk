use auto_lsp::default::db::BaseDatabase;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        body_inference::{BodyInferenceError, TypeError},
    },
    hir_def::expressions::expression::{Expr, ExprKind, PrimaryExpr, RefValue, UnaryOperatorKind},
    hir_ty::{
        body_inference::BodyInferenceResult,
        infer::inference_table::{self, InferenceTable},
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
    ) {
        match expr.expr(db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                let (infer_ctx, result) =
                    left.coerce_with_expression(db, *right, self.resolver, inference_results);

                if let Err(err) = result {
                    inference_results
                        .errors
                        .push(err.into_non_addable(db, expr, *operator));
                }

                let lhs = left.adjust_and_normalize(db, inference_results);
                let rhs = right.adjust_and_normalize(db, inference_results);

                // this might sound redundant because we already checked for coercion
                // but coercion is unaware of the operation being performed

                if !lhs.supports_math(rhs) {
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

                inference_results
                    .type_of_expr
                    .insert(expr, infer_ctx.get_final_type());
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                let (infer_ctx, result) =
                    left.coerce_with_expression(db, *right, self.resolver, inference_results);

                if let Err(err) = result {
                    inference_results
                        .errors
                        .push(err.into_non_multiplicable(db, expr, *operator));
                }

                let lhs = left.adjust_and_normalize(db, inference_results);
                let rhs = right.adjust_and_normalize(db, inference_results);

                if !lhs.supports_math(rhs) {
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

                inference_results
                    .type_of_expr
                    .insert(expr, infer_ctx.get_final_type());
            }
            ExprKind::PowerOperator { left, right } => {
                let (infer_ctx, result) =
                    left.coerce_with_expression(db, *right, self.resolver, inference_results);

                if let Err(err) = result {
                    inference_results
                        .errors
                        .push(err.into_non_comparable(db, expr));
                }

                inference_results
                    .type_of_expr
                    .insert(expr, infer_ctx.get_final_type());
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                // Adds a new inference ctx for boolean operators
                // each side has its own inference context and *must* be boolean
                InferExprCtx::new(self.resolver).infer_expr(db, *left, inference_results);

                let lhs = left.adjust_and_normalize(db, inference_results);
                if let Err(err) = lhs.coerce_with_type(db, Type::new_bool(), self.resolver) {
                    inference_results
                        .errors
                        .push(TypeError::NotABoolean { typ: lhs, expr }.to_diagnostic(db));
                }

                InferExprCtx::new(self.resolver).infer_expr(db, *right, inference_results);

                let rhs = right.adjust_and_normalize(db, inference_results);
                if let Err(err) = rhs.coerce_with_type(db, Type::new_bool(), self.resolver) {
                    inference_results
                        .errors
                        .push(TypeError::NotABoolean { typ: lhs, expr }.to_diagnostic(db));
                }

                // a boolean operator always returns a boolean
                inference_results
                    .type_of_expr
                    .insert(expr, Type::new_bool());
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                // A compare operation has it's own inference context since it returns a boolean
                let (inference, result) =
                    left.coerce_with_expression(db, *right, self.resolver, inference_results);

                if let Err(err) = result {
                    inference_results
                        .errors
                        .push(err.into_non_comparable(db, expr));
                }

                // from the standard:
                /*
                The comparison
                A = B

                would be used to compare the data value of variable A by the value of variable B if both were
                of the same data type or one of the variables can implicitly be converted to the data type of
                the other one.

                If A and B are multi-element variables the data types of A and B shall be the same. In this
                case the values of the elements of the variable A is compared to the values of the elements of
                variable B.

                */
                // In our case coercion will do both strict equality and implicit conversion checks
                // but i'm unsure if this is enough

                inference_results
                    .type_of_expr
                    .insert(expr, Type::new_bool());
            }
            ExprKind::UnaryOperator { expr: unary_expr, operator } => match operator {
                UnaryOperatorKind::Not => {
                    let mut ctx = InferExprCtx::new(self.resolver);

                    ctx.infer_expr(db, *unary_expr, inference_results);
                    let not_expr = unary_expr.adjust_and_normalize(db, inference_results);
                    if !not_expr.is_boolean() {
                        inference_results.errors.push(
                            TypeError::NotABoolean {
                                typ: not_expr,
                                expr: *unary_expr,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    inference_results
                        .type_of_expr
                        .insert(expr, Type::new_bool());
                }
                _ => self.infer_expr(db, *unary_expr, inference_results),
            },
            ExprKind::PrimaryExpr(primary) => {
                let typ = self.infer_primary(db, expr, primary, inference_results);
                inference_results.type_of_expr.insert(expr, typ);
                self.inference_table.add_type(db, expr, typ, self.resolver);
            }
        }
    }

    #[must_use]
    pub fn infer_primary(
        &mut self,
        db: &'db dyn BaseDatabase,
        base_expr: Expr<'db>,
        to: &PrimaryExpr<'db>,
        inference_result: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match to {
            PrimaryExpr::Literal(prim) => (*prim).into(),
            PrimaryExpr::VariableAccess(v) => {
                self.resolver
                    .resolve_variable_access(db, *v, inference_result)
            }
            PrimaryExpr::FuncCall(call) => {
                resolve_func_call(db, self.resolver, *call, inference_result)
            }
            PrimaryExpr::EnumValue { name, variant } => {
                let find_enm = self
                    .resolver
                    .resolve_begin_path_expr(db, *name, inference_result)
                    .normalize(db);

                match find_enm {
                    Type::Enum(enm) => enm
                        .variants(db)
                        .iter()
                        .find(|v| *v.name == **variant)
                        .map(|v| Type::EnumVariant(*v.name))
                        .unwrap_or_else(|| {
                            // variant not found
                            inference_result.errors.push(
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
                        inference_result.errors.push(
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
                        .resolve_begin_path_expr(db, *adress, inference_result)
                }
                RefValue::Null => Type::Null,
            },
            PrimaryExpr::ParenthesizedExpr { expr } => {
                self.infer_expr(db, *expr, inference_result);
                inference_result
                    .type_of_expr
                    .get(expr)
                    .copied()
                    .unwrap_or_default()
            }
        }
    }
}
