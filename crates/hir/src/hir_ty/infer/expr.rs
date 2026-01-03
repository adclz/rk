use ast::generated::AddOperator;
use auto_lsp::default::db::BaseDatabase;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        body_inference::{BodyInferenceError, TypeError},
    },
    hir_def::expressions::expression::{
        AddOperatorKind, Expr, ExprKind, PrimaryExpr, RefValue, UnaryOperatorKind,
    },
    hir_ty::{
        body_inference::{Adjustment, BodyInferenceResult},
        infer::{coerce::CoerceResult, inference_table::InferenceTable},
        resolver::{Resolver, body::InferenceCtx, func_call::resolve_func_call},
        ty::Type,
    },
};

pub struct InferExprCtx<'db> {
    pub resolver: Resolver<'db>,
}

impl<'db> InferExprCtx<'db> {
    pub fn new(resolver: Resolver<'db>) -> Self {
        Self { resolver }
    }

    pub fn infer_expr(
        &mut self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) {
        match expr.expr(db) {
            ExprKind::AddOperator { left, right, .. }
            | ExprKind::MultOperator { left, right, .. }
            | ExprKind::PowerOperator { left, right }
            | ExprKind::BooleanOperator { left, right, .. }
            | ExprKind::ComparisonOperator { left, right, .. } => {
                self.infer_expr(db, *left, inference_results);
                self.infer_expr(db, *right, inference_results);
            }
            ExprKind::UnaryOperator { expr, .. } => {
                self.infer_expr(db, *expr, inference_results);
            }
            ExprKind::PrimaryExpr(primary) => {
                let primary = self.infer_primary(db, expr, primary, inference_results);
                inference_results.type_of_expr.insert(expr, primary);
            }
        };
    }

    fn infer_primary(
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
                    let typ = self
                        .resolver
                        .resolve_begin_path_expr(db, *adress, inference_result);

                    if let Some(path) = adress.expr(db) {
                        inference_result
                            .path_expr_adjustments
                            .entry(path)
                            .or_insert_with(Vec::new)
                            .push(Adjustment::new_ref(db, typ));
                    };
                    typ
                }
                RefValue::Null => Type::Null,
            },
            PrimaryExpr::ParenthesizedExpr { expr } => {
                self.infer_expr(db, *expr, inference_result);
                inference_result.type_of_expr[expr]
            }
        }
    }

    pub fn check_expr(
        &self,
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
                if let Err(err) = self.coerce_expressions(db, *left, *right, inference_results) {
                    inference_results
                        .errors
                        .push(err.into_non_addable(db, expr, *operator));
                }
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                // check left operand
                if let Err(err) = self.coerce_from_type(
                    db,
                    CallSite::from_expr(db, expr),
                    Type::new_bool(),
                    *left,
                    inference_results,
                ) {
                    inference_results.errors.push(err.into_non_assignable(
                        db,
                        inference_results.type_of_expr[left],
                        CallSite::from_expr(db, *left),
                    ));
                }

                // check right operand
                if let Err(err) = self.coerce_from_type(
                    db,
                    CallSite::from_expr(db, expr),
                    Type::new_bool(),
                    *right,
                    inference_results,
                ) {
                    inference_results.errors.push(err.into_non_assignable(
                        db,
                        inference_results.type_of_expr[right],
                        CallSite::from_expr(db, *right),
                    ));
                }
            }
            ExprKind::ComparisonOperator { left, right, .. } => {
                if let Err(err) = self.coerce_expressions(db, *left, *right, inference_results) {
                    inference_results
                        .errors
                        .push(err.into_non_comparable(db, expr));
                }
            }
            _ => {}
        }
    }

    fn coerce_from_type(
        &self,
        db: &'db dyn BaseDatabase,
        call_site: CallSite<'db>,
        from: Type<'db>,
        right: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) -> CoerceResult<'db> {
        let to = inference_results.type_of_expr[&right];

        let mut table = InferenceTable::new();
        table.set_target_type(db, call_site, from);
        table.add_type(db, right, to, self.resolver);

        table.resolve_completly(db, self.resolver, inference_results);

        from.coerce_with_type(
            db,
            to,
            inference_results.adjustments_of_expr(db, right),
            self.resolver,
        )
    }

    fn coerce_expressions(
        &self,
        db: &'db dyn BaseDatabase,
        left: Expr<'db>,
        right: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) -> CoerceResult<'db> {
        let lhs_ty = inference_results.type_of_expr[&left];
        let rhs_ty = inference_results.type_of_expr[&right];

        let mut table = InferenceTable::new();
        table.add_type(db, left, lhs_ty, self.resolver);
        table.add_type(db, right, rhs_ty, self.resolver);

        table.resolve_completly(db, self.resolver, inference_results);

        let lhs_final = inference_results
            .type_of_expr_with_adjustments(db, left)
            .unwrap_or_default();

        lhs_final.coerce_with_type(
            db,
            rhs_ty,
            inference_results.adjustments_of_expr(db, right),
            self.resolver,
        )
    }
}
