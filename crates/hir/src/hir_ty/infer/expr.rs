use db::WorkspaceDataBase;

use crate::{
    CallSite, HirNodeInfo, check::errors::{ToIdeDiagnostic, e3_type::TypeError, e7_enum::EnumError}, hir_def::{
        expressions::expression::{
            Expr, ExprKind, FoldOperatorKind, MultOperatorKind, PrimaryExpr, RefValue, UnaryOperatorKind, VariableAccess,
        }, pous::variable::VariableDecl,
    }, hir_ty::{
        body::{Adjustment, BodyInferenceResult},
        infer::{Infer, coerce::CoerceResult, table::InferenceTable},
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
            | ExprKind::BooleanOperator { left, right, .. } => {
                self.resolve_expr(db, *left, inference_results);
                self.resolve_expr(db, *right, inference_results);

                // Use adjusted types to account for array indexing, deref, etc.
                // Kept UNPEELED so a diagnostic can still point at the operand's
                // declaration; subranges are peeled only where the decision
                // needs the base type.
                let lhs = inference_results.type_of_expr_with_adjustments(db, *left);
                let rhs = inference_results.type_of_expr_with_adjustments(db, *right);

                let mut ty = match (lhs.has_infer(), rhs.has_infer()) {
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
                    // Both concrete: the result is the two operands' join in the
                    // implicit-widening lattice (`ElementarySpec::wider`, the
                    // same join the inference table promotes with). Taking
                    // `lhs` unconditionally made arithmetic order-dependent —
                    // `REAL * INT` yielded REAL but `INT * REAL` yielded INT,
                    // and the coercion check then rejected the latter. Pairs
                    // with no common widening (e.g. `BOOL + REAL`) keep `lhs`
                    // so the coercion check below reports them.
                    // Subranges join through their base type (`INT (0..100)`
                    // adds like an `INT`).
                    _ => match (lhs.normalize(db), rhs.normalize(db)) {
                        (Type::Elementary(l), Type::Elementary(r)) => {
                            l.wider(r).map(Type::Elementary).unwrap_or(lhs)
                        }
                        _ => lhs,
                    },
                };

                // When a function/method name is used in an operator expression,
                // resolve to its return type for operator support checks.
                // Operator support is decided on the base type: a subrange
                // supports whatever its base supports.
                let normalized_ty = match ty.with_return_type(db) {
                    Some(ret) => ret.normalize(db),
                    None => ty.normalize(db),
                };
                let (supported, operator) = match curr_expr.expr(db) {
                    ExprKind::AddOperator { operator, .. } => {
                        (normalized_ty.supports_add(db), operator.as_str())
                    }
                    ExprKind::MultOperator { operator, .. } => (
                        match operator {
                            // RHS part of a Modulo operation has to be an integer.
                            MultOperatorKind::Mod => normalized_ty.supports_mod(db),
                            _ => normalized_ty.supports_mul(db),
                        },
                        operator.as_str(),
                    ),
                    ExprKind::BooleanOperator { operator, .. } => {
                        (normalized_ty.supports_bool_op(db), operator.as_str())
                    }
                    _ => unreachable!(),
                };

                if !supported && !ty.is_never() {
                    inference_results.errors.push(
                        TypeError::UnsupportedOperator {
                            call_site: curr_expr.as_call_site(db),
                            typ: ty,
                            operator,
                        }
                        .to_diagnostic(db, inference_results.scope.file(db)),
                    );
                    ty = Type::Never;
                }

                inference_results.type_of_expr.insert(curr_expr, ty);
                ty
            }
            // `**` does not join its operands. IEC types it `IN1 : ANY_REAL`,
            // `IN2 : ANY_NUM`, result = IN1's type — the exponent is
            // independent, so the 2 in `x ** 2` is an integer and stays one
            // (MIR casts it to the base at the call). Sharing the arithmetic
            // arm unified them, which took the result from the join
            // (`REAL ** LREAL` yielded LREAL) and asked the exponent to BE the
            // base: `i ** 3.0` with `i : INT` reported "cannot infer <float>
            // to INT" about a literal that never had to be an INT.
            ExprKind::PowerOperator { left, right } => {
                self.resolve_expr(db, *left, inference_results);
                self.resolve_expr(db, *right, inference_results);

                // Each operand resolves in ITS OWN context: a literal takes
                // its default (`2.0` -> REAL, `2` -> INT) — nothing about one
                // operand decides the other. So `2 ** 3.0` rejects the base
                // as an INT, with no complaint about the exponent.
                for operand in [left, right] {
                    let ty = inference_results.type_of_expr_with_adjustments(db, *operand);
                    if ty.has_infer() {
                        let mut table = InferenceTable::new();
                        table.add_type(db, *operand, ty, self.resolver);
                        table.resolve_completly(db, self.resolver, inference_results);
                    }
                }
                let lhs = inference_results.type_of_expr_with_adjustments(db, *left);

                // The result is IN1's type.
                let base = match lhs.with_return_type(db) {
                    Some(ret) => ret.normalize(db),
                    None => lhs.normalize(db),
                };
                let mut ty = lhs;
                if !base.is_never() && !base.supports_power(db) {
                    inference_results.errors.push(
                        TypeError::UnsupportedOperator {
                            call_site: curr_expr.as_call_site(db),
                            typ: lhs,
                            operator: "**",
                        }
                        .to_diagnostic(db, inference_results.scope.file(db)),
                    );
                    ty = Type::Never;
                }

                inference_results.type_of_expr.insert(curr_expr, ty);
                ty
            }
            ExprKind::ComparisonOperator { left, right, .. } => {
                // resolve operands for their side effects (populating type_of_expr)
                self.resolve_expr(db, *left, inference_results);
                self.resolve_expr(db, *right, inference_results);

                // Record the type the OPERANDS are compared at — their join in
                // the widening lattice, the same `ElementarySpec::wider` used
                // for arithmetic result typing. The comparison itself is BOOL,
                // so without this the operand type is lost and consumers
                // (codegen picking the machine comparison and inserting operand
                // casts) would have to re-derive it.
                let lhs = inference_results
                    .type_of_expr_with_adjustments(db, *left)
                    .normalize(db);
                let rhs = inference_results
                    .type_of_expr_with_adjustments(db, *right)
                    .normalize(db);
                if let (Type::Elementary(l), Type::Elementary(r)) =
                    (lhs.normalize(db), rhs.normalize(db))
                    && let Some(common) = l.wider(r)
                {
                    inference_results
                        .comparison_operand_type
                        .insert(curr_expr, Type::Elementary(common));
                }

                // comparison operators always return BOOL
                let ty = Type::new_bool();
                inference_results.type_of_expr.insert(curr_expr, ty);
                ty
            }
            ExprKind::UnaryOperator { expr, .. } => {
                let ty = self.resolve_expr(db, *expr, inference_results);
                inference_results.type_of_expr.insert(curr_expr, ty);
                ty
            }
            ExprKind::PrimaryExpr(primary) => {
                if let PrimaryExpr::Literal(elem) = primary
                    && let Err(err) = elem.check(db)
                {
                    let ty: Type = (*elem).into();
                    inference_results.errors.push(
                        TypeError::InferLiteralError {
                            expr: curr_expr,
                            source: None,
                            target: ty,
                            err,
                        }
                        .to_diagnostic(db, inference_results.scope.file(db)),
                    );
                }
                let primary = self.infer_primary(db, primary, inference_results);
                inference_results.type_of_expr.insert(curr_expr, primary);
                inference_results.type_of_expr[&curr_expr]
            }
            ExprKind::FoldExpr {
                param, operator, ..
            } => {
                // Look up the variadic parameter in the current scope
                let scope = curr_expr.scope_id(db);
                let def_map = scope.def_map(db);
                let ty = match def_map.local_variables.get(param) {
                    Some(var) => {
                        if !var.variadic(db) {
                            inference_results.errors.push(
                                TypeError::NonVariadicFoldParameter {
                                    call_site: curr_expr.as_call_site(db),
                                    var: *var,
                                }
                                .to_diagnostic(db, inference_results.scope.file(db)),
                            );
                        }

                        let var_ty = var.spec(db).infer(db).normalize(db);

                        // Check operator-type compatibility
                        let supported = match operator {
                            FoldOperatorKind::Plus | FoldOperatorKind::Minus => {
                                var_ty.supports_add(db)
                            }
                            FoldOperatorKind::Mul => var_ty.supports_mul(db),
                            FoldOperatorKind::Div => var_ty.supports_div(db),
                            FoldOperatorKind::Mod => var_ty.supports_mod(db),
                            FoldOperatorKind::Power => var_ty.supports_power(db),
                            FoldOperatorKind::And
                            | FoldOperatorKind::Or
                            | FoldOperatorKind::Xor => var_ty.supports_bool_op(db),
                            FoldOperatorKind::Eq
                            | FoldOperatorKind::Ne
                            | FoldOperatorKind::Lt
                            | FoldOperatorKind::Gt
                            | FoldOperatorKind::Le
                            | FoldOperatorKind::Ge => var_ty.supports_comparison(db),
                        };

                        if !supported && !var_ty.is_never() {
                            inference_results.errors.push(
                                TypeError::UnsupportedOperator {
                                    call_site: curr_expr.as_call_site(db),
                                    typ: Type::new_var(db, *var),
                                    operator: operator.as_str(),
                                }
                                .to_diagnostic(db, inference_results.scope.file(db)),
                            );
                        }

                        if operator.is_comparison() {
                            Type::new_bool()
                        } else {
                            Type::new_var(db, *var)
                        }
                    }
                    None => Type::Never,
                };
                inference_results.type_of_expr.insert(curr_expr, ty);
                ty
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

                let ty = inference_result.get_type_of_variable_access(db, *v);
                // `THIS` types as the FB/Class itself, which is also what a
                // bare type NAME types as — so the guard below cannot tell a
                // self-reference from `Worker` written where a value belongs.
                // A self-reference IS a value; whether it fits wherever it was
                // written is for the coercion at that site to say, which is
                // what accepts `f(dev := THIS)` for an interface parameter and
                // still refuses it everywhere no arm accepts an FB.
                if !v.is_bare_this(db) {
                    ty.check_not_direct_type(db, CallSite::from_scoped(db, v), inference_result);
                }
                ty
            }
            PrimaryExpr::FuncCall(call) => {
                resolve_func_call(db, self.resolver, *call, inference_result);
                let ty = inference_result.get_type_of_begin_path_expr(db, call.path(db));
                ty.normalize(db)
            }
            PrimaryExpr::EnumValue { name, variant } => {
                self.resolver
                    .resolve_begin_path_expr(db, *name, None, inference_result);

                let find_enm = inference_result.type_of_begin_expr_with_adjustments(db, *name);

                // The path names a TYPE; the variant type keeps THAT (name
                // and all), while normalize digs out the enum spec to check
                // the variant against. A path that reaches an enum any other
                // way than through a DataType has no name to carry and falls
                // back to the enum type itself, which coerces identically.
                match find_enm.normalize(db) {
                    Type::Enum(enm) => enm
                        .enum_variants(db)
                        .get(variant)
                        .map(|v| match find_enm {
                            Type::DataType(dt) => Type::EnumVariant(dt, *v.name),
                            _ => find_enm,
                        })
                        .unwrap_or_else(|| {
                            // variant not found
                            inference_result.errors.push(
                                EnumError::EnumVariantNotFound {
                                    enum_: enm,
                                    variant_name: *variant,
                                }
                                .to_diagnostic(db, inference_result.scope.file(db)),
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
                            .to_diagnostic(db, inference_result.scope.file(db)),
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
            ExprKind::UnaryOperator {
                expr: inner_expr, ..
            } => {
                self.check_expr(db, *inner_expr, inference_results);
            }
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner_expr }) => {
                self.check_expr(db, *inner_expr, inference_results);
                // Update the parenthesized expression's type to match the inner expression
                let inner_ty = inference_results.type_of_expr[inner_expr];
                inference_results.type_of_expr.insert(expr, inner_ty);
            }
            ExprKind::PrimaryExpr(_) | ExprKind::FoldExpr { .. } => {}
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
            ExprKind::PowerOperator { left: _, right } => {
                // The exponent is not coerced to the base — IEC types it
                // ANY_NUM, so it only has to be numeric. `x ** 'a'` is what
                // this rejects.
                let rhs = inference_results
                    .type_of_expr_with_adjustments(db, *right)
                    .normalize(db);
                if !rhs.has_infer() && !rhs.is_never() && !rhs.is_numeric() {
                    inference_results.errors.push(
                        TypeError::UnsupportedOperator {
                            call_site: CallSite::from_scoped(db, right),
                            typ: rhs,
                            operator: "**",
                        }
                        .to_diagnostic(db, inference_results.scope.file(db)),
                    );
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
                        // IEC 61131-3: NOT is defined on ANY_BIT — logical
                        // negation for BOOL, bitwise complement for
                        // BYTE/WORD/DWORD/LWORD. A bit-string operand passes
                        // as-is; everything else must coerce to BOOL.
                        use crate::hir_def::expressions::spec::ElementarySpec;
                        let operand_ty = inference_results
                            .type_of_expr
                            .get(expr)
                            .copied()
                            .map(|t| t.normalize(db));
                        let is_bit_string = matches!(
                            operand_ty,
                            Some(Type::Elementary(
                                ElementarySpec::Bool
                                    | ElementarySpec::Byte
                                    | ElementarySpec::Word
                                    | ElementarySpec::DWord
                                    | ElementarySpec::LWord
                            ))
                        );
                        if !is_bit_string
                            && let Err(err) = self.coerce_type_with_expr(
                                db,
                                Type::new_bool(),
                                *expr,
                                inference_results,
                            )
                        {
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
            ExprKind::PrimaryExpr(_) | ExprKind::FoldExpr { .. } => { /* already checked in infer_expr */
            }
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
        let errors_before = inference_results.errors.len();

        let mut table = InferenceTable::new();
        table.set_target_type(db, Some(lhs.into()), lhs);
        table.add_type(db, rhs, to, self.resolver);
        table.resolve_completly(db, self.resolver, inference_results);

        let result = lhs.coerce_with_type(
            db,
            inference_results.type_of_expr_with_adjustments(db, rhs),
            inference_results.adjustments_of_expr(db, rhs),
            self.resolver,
        );
        // Only when this assignment is otherwise clean. A value that already
        // failed against the BASE type (`-1` for a UINT subrange) is reported
        // there; adding "outside subrange" would be two errors for one mistake.
        if result.is_ok()
            && inference_results.errors.len() == errors_before
            && let Some(err) = lhs.subrange_violation(db, rhs)
        {
            inference_results
                .errors
                .push(err.to_diagnostic(db, inference_results.scope.file(db)));
        }
        result
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
        let errors_before = inference_results.errors.len();

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

        let result = lhs.coerce_with_type(
            db,
            inference_results.type_of_expr_with_adjustments(db, rhs),
            inference_results.adjustments_of_expr(db, rhs),
            self.resolver,
        );
        // See `coerce_var_decl_with_expr`: bounds are only meaningful for an
        // assignment that is otherwise clean.
        if result.is_ok()
            && inference_results.errors.len() == errors_before
            && let Some(err) = lhs.subrange_violation(db, rhs)
        {
            inference_results
                .errors
                .push(err.to_diagnostic(db, inference_results.scope.file(db)));
        }
        result
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
        let lhs_ty = inference_results.type_of_expr_with_adjustments(db, left);
        let rhs_ty = inference_results.type_of_expr_with_adjustments(db, right);

        let mut table = InferenceTable::new();
        table.add_type(db, left, lhs_ty, self.resolver);
        table.add_type(db, right, rhs_ty, self.resolver);

        table.resolve_completly(db, self.resolver, inference_results);

        let l = inference_results.type_of_expr_with_adjustments(db, left);
        let r = inference_results.type_of_expr_with_adjustments(db, right);

        // The two operands of a binary operator are commutative for coercion:
        // they are compatible iff EITHER widens to the other (they share a
        // common type in the widening lattice). Coercing only `right -> left`
        // made `INT * REAL` an error while `REAL * INT` compiled. Try both
        // directions; report the original (left-anchored) error if neither
        // works, so genuine mismatches (e.g. BOOL * REAL) still fail.
        match l.coerce_with_type(
            db,
            r,
            inference_results.adjustments_of_expr(db, right),
            self.resolver,
        ) {
            Ok(()) => Ok(()),
            Err(err) => match r.coerce_with_type(
                db,
                l,
                inference_results.adjustments_of_expr(db, left),
                self.resolver,
            ) {
                Ok(()) => Ok(()),
                Err(_) => Err(err),
            },
        }
    }
}
