use db::WorkspaceDataBase;

use crate::{
    CallSite,
    check::errors::{analysis_error::ToIdeDiagnostic, e7_enum::EnumError},
    hir_def::{
        expressions::expression::{
            Expr, ExprKind, PrimaryExpr, RefValue, UnaryOperatorKind, VariableAccess,
        },
        pous::variable::VariableDecl,
    },
    hir_ty::{
        body::{Adjustment, BodyInferenceResult},
        infer::{coerce::CoerceResult, table::InferenceTable},
        resolver::{Resolver, func_call::resolve_func_call},
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

    pub fn resolve_expr(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        curr_expr: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match curr_expr.expr(db) {
            ExprKind::AddOperator { left, right, .. }
            | ExprKind::MultOperator { left, right, .. }
            | ExprKind::PowerOperator { left, right }
            | ExprKind::BooleanOperator { left, right, .. }
            | ExprKind::ComparisonOperator { left, right, .. } => {
                let lhs = self.resolve_expr(db, *left, inference_results);
                let rhs = self.resolve_expr(db, *right, inference_results);

                let ty = match (lhs.has_infer(), rhs.has_infer()) {
                    (true, false) => rhs,
                    (false, true) => lhs,
                    (true, true) => {
                        // since both expression are infer types, we need to unify early
                        let mut table = InferenceTable::new();
                        table.add_type(db, *left, lhs, self.resolver);
                        table.add_type(db, *right, rhs, self.resolver);
                        table.resolve_completly(db, self.resolver, inference_results);
                        table.get_final_type()
                    }
                    _ => lhs,
                };

                inference_results.type_of_expr.insert(curr_expr, ty);
                ty
            }
            ExprKind::UnaryOperator { expr, .. } => {
                let ty = self.resolve_expr(db, *expr, inference_results);
                inference_results.type_of_expr.insert(curr_expr, ty);
                ty
            }
            ExprKind::PrimaryExpr(primary) => {
                let primary = self.infer_primary(db, primary, inference_results);
                inference_results.type_of_expr.insert(curr_expr, primary);
                inference_results.type_of_expr[&curr_expr]
            }
        }
    }

    fn infer_primary(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        to: &PrimaryExpr<'db>,
        inference_result: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match to {
            PrimaryExpr::Literal(prim) => (*prim).into(),
            PrimaryExpr::VariableAccess(v) => {
                self.resolver
                    .resolve_variable_access(db, *v, inference_result);

                inference_result.get_type_of_variable_access(db, *v)
            }
            PrimaryExpr::FuncCall(call) => {
                resolve_func_call(db, self.resolver, *call, inference_result);
                inference_result.get_type_of_begin_path_expr(db, call.path(db))
            }
            PrimaryExpr::EnumValue { name, variant } => {
                self.resolver
                    .resolve_begin_path_expr(db, *name, None, inference_result);

                let find_enm = inference_result.type_of_begin_expr_with_adjustments(db, *name);

                // normalizing here is necessary, but the type itself should be stored as is
                match find_enm.normalize(db) {
                    Type::Enum(enm) => enm
                        .enum_variants(db)
                        .get(variant)
                        .map(|v| Type::EnumVariant(*v.name))
                        .unwrap_or_else(|| {
                            // variant not found
                            inference_result.errors.push(
                                EnumError::EnumVariantNotFound {
                                    enum_: enm,
                                    variant_name: *variant,
                                }
                                .to_diagnostic(db),
                            );
                            find_enm
                        }),
                    _ => {
                        // not an enum
                        inference_result.errors.push(
                            EnumError::NotAnEnum {
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
                        .resolve_begin_path_expr(db, *adress, None, inference_result);

                    let typ = inference_result.type_of_begin_expr_with_adjustments(db, *adress);

                    if let Some(path) = adress.expr(db) {
                        inference_result
                            .path_expr_adjustments
                            .entry(path)
                            .or_default()
                            .push(Adjustment::new_ref(db, typ));
                    };
                    typ
                }
                RefValue::Null => Type::Null,
            },
            PrimaryExpr::ParenthesizedExpr { expr } => {
                self.resolve_expr(db, *expr, inference_result);
                inference_result.type_of_expr[expr]
            }
        }
    }

    pub fn check_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) {
        match expr.expr(db) {
            ExprKind::AddOperator { left, right, .. }
            | ExprKind::MultOperator { left, right, .. }
            | ExprKind::PowerOperator { left, right }
            | ExprKind::BooleanOperator { left, right, .. }
            | ExprKind::ComparisonOperator { left, right, .. } => {
                self.check_expr(db, *left, inference_results);
                self.check_expr(db, *right, inference_results);
            }
            ExprKind::UnaryOperator { expr, .. } => {
                self.check_expr(db, *expr, inference_results);
            }
            ExprKind::PrimaryExpr(_) => {}
        }

        match expr.expr(db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                if let Err(err) = self.coerce_expressions(db, *left, *right, inference_results) {
                    inference_results.errors.push(err.into_non_addable(
                        db,
                        inference_results.type_of_expr[left],
                        CallSite::from_scoped(db, right),
                        *operator,
                    ));
                }
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                if let Err(err) = self.coerce_expressions(db, *left, *right, inference_results) {
                    inference_results.errors.push(err.into_non_multiplicable(
                        db,
                        inference_results.type_of_expr[left],
                        CallSite::from_scoped(db, right),
                        *operator,
                    ));
                }
            }
            ExprKind::PowerOperator { left, right } => {
                if let Err(err) = self.coerce_expressions(db, *left, *right, inference_results) {
                    inference_results.errors.push(err.into_non_powerable(
                        db,
                        inference_results.type_of_expr[left],
                        CallSite::from_scoped(db, right),
                    ));
                }
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                if let Err(err) = self.coerce_expressions(db, *left, *right, inference_results) {
                    inference_results.errors.push(err.into_non_comparable(
                        db,
                        inference_results.type_of_expr[left],
                        CallSite::from_scoped(db, right),
                    ));
                }
            }
            ExprKind::ComparisonOperator {
                left,
                right,
                operator,
            } => {
                if let Err(err) = self.coerce_expressions(db, *left, *right, inference_results) {
                    inference_results.errors.push(err.into_non_comparable(
                        db,
                        inference_results.type_of_expr[left],
                        CallSite::from_scoped(db, right),
                    ));
                }
                // the return type of a comparison expression is bool
                inference_results
                    .type_of_expr
                    .insert(expr, Type::new_bool());
            }
            ExprKind::UnaryOperator { expr, operator } => {
                match operator {
                    UnaryOperatorKind::Not => {
                        if let Err(err) = self.coerce_type_with_expr(
                            db,
                            Type::new_bool(),
                            *expr,
                            inference_results,
                        ) {
                            inference_results.errors.push(err.into_non_assignable(
                                db,
                                inference_results.type_of_expr[expr],
                                CallSite::from_scoped(db, expr),
                            ));
                        }
                    }
                    _ => {
                        // numeric-only, reuse existing coercion rules
                    }
                }
            }
            ExprKind::PrimaryExpr(_) => { /* already checked in infer_expr */ }
        }
    }

    pub fn coerce_var_decl_with_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var: VariableDecl<'db>,
        rhs: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) -> CoerceResult<'db> {
        let lhs = Type::new_var(db, var);
        let to = inference_results.type_of_expr[&rhs];

        let mut table = InferenceTable::new();
        table.set_target_type(db, Some(lhs.into()), lhs);
        table.add_type(db, rhs, to, self.resolver);
        table.resolve_completly(db, self.resolver, inference_results);

        lhs.coerce_with_type(
            db,
            inference_results.type_of_expr_with_adjustments(db, rhs),
            inference_results.adjustments_of_expr(db, rhs),
            self.resolver,
        )
    }

    pub fn coerce_var_access_with_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var: VariableAccess<'db>,
        rhs: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) -> CoerceResult<'db> {
        let lhs = inference_results.type_of_variable_access_with_adjustments(db, var);
        let to = inference_results.type_of_expr[&rhs];

        let mut table = InferenceTable::new();
        table.set_target_type(
            db,
            Some(
                inference_results
                    .get_type_of_variable_access(db, var)
                    .into(),
            ),
            lhs,
        );
        table.add_type(db, rhs, to, self.resolver);
        table.resolve_completly(db, self.resolver, inference_results);

        let rhs_ = rhs.expr(db);

        lhs.coerce_with_type(
            db,
            inference_results.type_of_expr_with_adjustments(db, rhs),
            inference_results.adjustments_of_expr(db, rhs),
            self.resolver,
        )
    }

    pub fn coerce_type_with_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        lhs: Type<'db>,
        rhs: Expr<'db>,
        inference_results: &mut BodyInferenceResult<'db>,
    ) -> CoerceResult<'db> {
        let to = inference_results.type_of_expr[&rhs];

        let mut table = InferenceTable::new();
        table.set_target_type(db, None, lhs);
        table.add_type(db, rhs, to, self.resolver);

        table.resolve_completly(db, self.resolver, inference_results);

        lhs.coerce_with_type(
            db,
            inference_results.type_of_expr_with_adjustments(db, rhs),
            inference_results.adjustments_of_expr(db, rhs),
            self.resolver,
        )
    }

    fn coerce_expressions(
        &self,
        db: &'db dyn WorkspaceDataBase,
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

        inference_results
            .type_of_expr_with_adjustments(db, left)
            .coerce_with_type(
                db,
                inference_results.type_of_expr_with_adjustments(db, right),
                inference_results.adjustments_of_expr(db, right),
                self.resolver,
            )
    }
}
