// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{ToIdeDiagnostic, e01_duplicates::DuplicateError, e05_array::ArrayError},
    hir_def::{
        expressions::expression::{Expr, InitExpr, InitExprKind},
        interned::identifier::Ident,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::BodyInferenceResult,
        expr_store::InitExprWalkStep,
        infer::{Infer, expr::InferExprCtx},
        resolver::{Resolver, walk::InitPlaceBuilder},
        ty::Type,
    },
};

#[tracing::instrument(level = "trace", skip(db))]
#[salsa::tracked(returns(ref))]
pub fn infer_initialization<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
) -> InitInference<'db> {
    InitInference::new(scope).check_init(db)
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct InitInference<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    /// Initializer expression inference results
    pub init_expr_result: InitExprInferenceResult<'db>,

    /// What the initializers' expressions resolved to, read through
    /// [`ScopeInference`](crate::hir_ty::body::ScopeInference).
    pub(crate) body_infer_result: BodyInferenceResult<'db>,

    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> InitInference<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            init_expr_result: InitExprInferenceResult::new(scope),
            body_infer_result: BodyInferenceResult::new(scope),
            errors: Vec::new(),
        }
    }

    pub fn check_init(mut self, db: &'db dyn WorkspaceDataBase) -> Self {
        if let ScopeKind::Pou(pou) = get_scope(db, self.scope).kind
            && let Pou::DataType(dt) = pou
        {
            self.check_spec(db, dt.spec(db));

            let typ = dt.spec(db).infer(db);
            if let Some(expr) = dt.init(db) {
                self.init_expr_result
                    .resolve_init_expr(db, expr, &mut self.body_infer_result, typ);
                self.check_string_init(db, dt.spec(db), expr);
            };
        }

        self.check_variables(db);
        self.check_initialization_order(db);
        self.check_usings(db);
        self.check_methods(db);
        self.check_function_specifier(db);
        self.check_return_type(db);
        self.check_config_values(db);
        self.check_connection_constants(db);
        self.check_task_constants(db);

        // Once-per-type initializers must be constant (user-ruled): a TYPE
        // default, an FB/CLASS member default, and anything static — a
        // PROGRAM field, a config global — is part of a declaration, not a
        // computation. FUNCTIONs and METHODs re-initialize per call and are
        // exempt. The acceptance test is `init_leaf_is_constant`, the SAME
        // one MIR folds by, so nothing accepted here fails to lower.
        let enforce = matches!(
            get_scope(db, self.scope).kind,
            ScopeKind::Pou(Pou::DataType(_))
                | ScopeKind::Pou(Pou::FunctionBlock(_))
                | ScopeKind::Pou(Pou::Class(_))
                | ScopeKind::Program(_)
                | ScopeKind::Config(_)
        );
        if enforce {
            for leaves in self.init_expr_result.resolved.values() {
                for leaf in leaves {
                    if !crate::hir_ty::infer::const_eval::init_leaf_is_constant(db, leaf.value) {
                        self.errors.push(
                            crate::check::errors::e04_init::InitError::InitNotConstant {
                                value: leaf.value,
                                input_default: false,
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                }
            }
        }

        self.check_input_defaults(db);
        self.check_constant_references(db);
        // No initializer runs in a call: a member's runs at `__init`, a
        // method local's in a call of its own.
        crate::hir_ty::body::refuse_conformand_outside_body(db, false, &mut self.body_infer_result);

        for error in &self.init_expr_result.errors {
            self.errors.push(error.clone());
        }

        for error in &self.body_infer_result.errors {
            self.errors.push(error.clone());
        }
        self
    }
}

impl<'db> InitInference<'db> {
    /// A FUNCTION's or METHOD's input default is what the CALLER passes for
    /// an omitted argument, before the callee's own variables exist. It is
    /// a constant, by the test member defaults pass, and a `REF()` in it,
    /// or at the end of the CONSTANT chain it names, is a global's address.
    fn check_input_defaults(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::check::errors::e04_init::InitError;
        use crate::hir_def::expressions::expression::InitExprKind;
        use crate::hir_ty::infer::const_eval;
        if !matches!(
            get_scope(db, self.scope).kind,
            ScopeKind::Pou(Pou::Function(_)) | ScopeKind::MethodDecl(_) | ScopeKind::MethodProt(_)
        ) {
            return;
        }
        let Some(vars) = self.scope.variables(db) else {
            return;
        };
        for var in vars.iter().filter(|var| var.is_input(db)) {
            let Some(InitExprKind::ConstantExpr(value)) = var.init(db).map(|init| init.kind(db))
            else {
                continue;
            };
            let error = if !const_eval::init_leaf_is_constant(db, value) {
                InitError::InitNotConstant {
                    value,
                    input_default: true,
                }
            } else if let Some(origin) = self.reference_origin(
                db,
                const_eval::resolve_constant_ref(db, value).unwrap_or(value),
            ) {
                InitError::ReferenceNotConstant {
                    value,
                    origin,
                    input_default: true,
                }
            } else {
                continue;
            };
            self.errors
                .push(error.to_diagnostic(db, self.scope.file(db)));
        }
    }

    /// A CONSTANT is one value for every instance and every call, so a
    /// `REF()` in it is a global's address: one of a member or of a call's
    /// own variable is a different address in each.
    fn check_constant_references(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::check::errors::e04_init::InitError;
        let Some(vars) = self.scope.variables(db) else {
            return;
        };
        for var in vars {
            if !var.qualifier(db).contains(crate::Qualifier::CONSTANT) {
                continue;
            }
            let Some(init) = var.init(db) else {
                continue;
            };
            let Some(leaves) = self.init_expr_result.resolved.get(&init) else {
                continue;
            };
            let refused: Vec<_> = leaves
                .iter()
                .filter_map(|leaf| {
                    let origin = self.reference_origin(db, leaf.value)?;
                    Some(InitError::ReferenceNotConstant {
                        value: leaf.value,
                        origin,
                        input_default: false,
                    })
                })
                .collect();
            for error in refused {
                self.errors
                    .push(error.to_diagnostic(db, self.scope.file(db)));
            }
        }
    }

    /// What keeps `value`, when it is a `REF()`, from being one address
    /// everywhere: a global's, reached through fields and subscripts that
    /// fold, is. `None` for that, and for what is not a `REF()`.
    fn reference_origin(
        &self,
        db: &'db dyn WorkspaceDataBase,
        value: crate::hir_def::expressions::expression::Expr<'db>,
    ) -> Option<crate::check::errors::e04_init::RefOrigin<'db>> {
        use crate::check::errors::e04_init::RefOrigin;
        use crate::hir_def::expressions::expression::{
            ExprKind, PathExprKind, PrimaryExpr, RefValue,
        };
        use crate::hir_def::pous::variable::StorageClass;
        use crate::hir_ty::expr_store::PathExprWalkStep;
        let ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
            value: RefValue::Address(path),
        }) = value.expr(db)
        else {
            return None;
        };
        let Some(path) = path.expr(db) else {
            return Some(RefOrigin::Unknown);
        };
        let steps = path.flatten(db);
        let root = steps.first()?.get_expr(db);
        // A CONSTANT's value was resolved where it is declared.
        let scope = root.scope_id(db);
        let decl = if scope == self.scope {
            self.body_infer_result.variable_for_path_expr(root)
        } else {
            infer_initialization(db, scope)
                .body_infer_result
                .variable_for_path_expr(root)
        };
        match decl {
            None => return Some(RefOrigin::Unknown),
            Some(decl) if decl.storage_class(db) != StorageClass::Global => {
                return Some(RefOrigin::Variable(decl));
            }
            Some(_) => {}
        }
        for step in steps {
            let subscripts = match step {
                PathExprWalkStep::Field { .. } => continue,
                PathExprWalkStep::Deref { .. } => return Some(RefOrigin::Deref),
                PathExprWalkStep::Index { expr } => match expr.expr(db) {
                    PathExprKind::Index(index) => index.index,
                    _ => return Some(RefOrigin::Unknown),
                },
            };
            for sub in subscripts {
                if crate::hir_ty::infer::const_eval::spec_value(db, sub).is_some() {
                    continue;
                }
                let part =
                    crate::hir_ty::infer::const_eval::non_constant_part(db, sub).unwrap_or(sub);
                return Some(match part.expr(db) {
                    ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(_)) => RefOrigin::Call,
                    ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) => {
                        match crate::hir_ty::infer::const_eval::spec_name_binding(db, *va) {
                            Some(var) => RefOrigin::Variable(var),
                            None => RefOrigin::Unknown,
                        }
                    }
                    _ => RefOrigin::Unknown,
                });
            }
        }
        None
    }

    /// A CONFIGURATION's VAR_CONFIG values, resolved like any initializer
    /// against the type of the variable each names, so lowering finds their
    /// leaves where it finds every other's. A member with no value of its own
    /// is refused as it is in an instance's initializer (E0405).
    fn check_config_values(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::check::errors::e04_init::{InitError, UninitializableMember};
        let ScopeKind::Config(config) = get_scope(db, self.scope).kind else {
            return;
        };
        for entry in &crate::hir_ty::config::resolve_config_entries(db, config).entries {
            let (Some(init), Some(var)) = (entry.init, entry.members.last()) else {
                continue;
            };
            let uninitializable = if var.is_in_out(db) {
                Some(UninitializableMember::InOut)
            } else if var.is_temp(db) {
                Some(UninitializableMember::Temp)
            } else if var.is_external(db) {
                Some(UninitializableMember::External)
            } else if var.qualifier(db).contains(crate::Qualifier::CONSTANT) {
                Some(UninitializableMember::Constant)
            } else {
                None
            };
            if let Some(kind) = uninitializable {
                self.errors.push(
                    InitError::UninitializableMember {
                        expr: init,
                        var: *var,
                        kind,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
                continue;
            }
            self.init_expr_result.resolve_init_expr(
                db,
                init,
                &mut self.body_infer_result,
                var.spec(db).infer(db),
            );
        }
    }

    /// A task's INTERVAL and SINGLE written as constants, typed as they are
    /// written.
    fn check_task_constants(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::hir_def::config::DataSource;
        let ScopeKind::Config(config) = get_scope(db, self.scope).kind else {
            return;
        };
        for task in config.resources(db).iter().flat_map(|r| r.tasks(db).iter()) {
            for source in [task.interval(db), task.single(db)].into_iter().flatten() {
                let DataSource::Constant(value) = source else {
                    continue;
                };
                let body = &mut self.body_infer_result;
                let mut infer_ctx = InferExprCtx::new(Resolver::for_scope(db, self.scope));
                infer_ctx.resolve_expr_expecting(db, value, body, None);
                infer_ctx.check_expr(db, value, body);
            }
        }
    }

    /// The constants a program configuration feeds its inputs, typed against
    /// each input as an initializer is against its variable.
    fn check_connection_constants(&mut self, db: &'db dyn WorkspaceDataBase) {
        use crate::hir_ty::config::ConnectionEnd;
        let ScopeKind::Config(config) = get_scope(db, self.scope).kind else {
            return;
        };
        for (_, elements) in &crate::hir_ty::config::prog_elements(db, config).per_program {
            for (var, source) in &elements.inputs {
                let ConnectionEnd::Constant(value) = source else {
                    continue;
                };
                let expected = var.spec(db).infer(db);
                let body = &mut self.body_infer_result;
                let mut infer_ctx = InferExprCtx::new(Resolver::for_scope(db, self.scope));
                infer_ctx.resolve_expr_expecting(db, *value, body, Some(expected));
                infer_ctx.check_expr(db, *value, body);
                if let Err(err) = infer_ctx.coerce_type_with_expr(db, expected, *value, body) {
                    self.errors.push(err.into_non_assignable_init(
                        db,
                        expected,
                        CallSite::from_scoped(db, value),
                    ));
                } else if let Some(err) = expected.subrange_violation(db, *value) {
                    self.errors.push(err.to_diagnostic(db, self.scope.file(db)));
                }
            }
        }
    }
}

/// One step from the initialized variable's root to a leaf value: either into a
/// struct/FB field by name, or into an array at a flat (row-major) element index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitPathStep {
    Field(Ident),
    ArrayElem(u32),
}

/// A fully-resolved leaf of an initializer: the constant `value` belongs at
/// `path` from the variable root. This is the authoritative, flattened output of
/// init inference — produced once here, with validation, so consumers (MIR
/// lowering) never re-walk or re-interpret the raw `InitExpr` tree.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct ResolvedInit<'db> {
    pub path: Vec<InitPathStep>,
    pub value: Expr<'db>,
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct InitExprInferenceResult<'db> {
    /// Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,
    /// Mapping of init expr to their resolved types
    pub type_of_init_expr: FxHashMap<InitExpr<'db>, Type<'db>>,
    /// Resolved, flattened leaves per root initializer expression — the
    /// authoritative output consumed by MIR lowering.
    pub resolved: FxHashMap<InitExpr<'db>, Vec<ResolvedInit<'db>>>,
    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> InitExprInferenceResult<'db> {
    pub(crate) fn new(scope: ScopeId<'db>) -> Self {
        Self {
            type_of_init_expr: FxHashMap::default(),
            resolved: FxHashMap::default(),
            scope,
            errors: Vec::new(),
        }
    }

    pub(crate) fn resolve_init_expr(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expr: InitExpr<'db>,
        body_ctx: &mut BodyInferenceResult<'db>,
        typ: Type<'db>,
    ) {
        let map = expr.flatten(db);
        let normalized = typ.normalize(db);
        let array_root = matches!(normalized, Type::Array(_)).then_some(typ);
        let mut ctx = InitContext::new(array_root);
        let mut place = InitPlaceBuilder {
            current_init_typ: typ,
        };
        let errors_before = self.errors.len();
        self.resolve_steps(db, typ, &mut place, body_ctx, &mut ctx, map);

        // Produce the authoritative, flattened resolved leaves. This is a pure,
        // type-DIRECTED-by-structure pass (brackets = nesting): it flattens the
        // initializer into row-major (path, value) leaves for MIR to consume,
        // placing each nested bracket as the walk above decided, a row or an
        // element. Validation lives in the walk; this never re-validates.
        //
        // Only for an initializer the walk accepted: the leaves feed the
        // lowering, which a rejected program never reaches, and a repetition
        // the walk refused as too many elements (E0507) is expanded here
        // count by count. `[15532559262904483838(0)]` took the checker down
        // at two gigabytes.
        if self.errors.len() > errors_before {
            return;
        }
        let mut leaves = Vec::new();
        resolve_leaves(
            db,
            expr,
            &self.type_of_init_expr,
            &ctx.roles,
            &mut Vec::new(),
            &mut leaves,
        );
        if !leaves.is_empty() {
            self.resolved.insert(expr, leaves);
        }
    }

    fn resolve_steps(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expected: Type<'db>,
        place: &mut InitPlaceBuilder<'db>,
        body_ctx: &mut BodyInferenceResult<'db>,
        ctx: &mut InitContext<'db>,
        map: &'db [InitExprWalkStep],
    ) {
        for step in map {
            let mut child_place = *place;
            self.resolve_step(db, expected, &mut child_place, body_ctx, ctx, step);
        }
    }

    /// The values of an array's last dimension, of its element type. When
    /// the element is itself an array, a bracket is one element: measured
    /// against the element's own dimensions, in a context of its own, and one
    /// cell of this array, where a row of it would be all its values.
    fn resolve_elements(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        element: Type<'db>,
        place: &mut InitPlaceBuilder<'db>,
        body_ctx: &mut BodyInferenceResult<'db>,
        ctx: &mut InitContext<'db>,
        values: &'db [InitExprWalkStep],
    ) {
        let element_is_array = matches!(element.normalize(db), Type::Array(_));
        for step in values {
            let mut child_place = *place;
            match step {
                InitExprWalkStep::ArrayInit { expr, .. } if element_is_array => {
                    let saved_root = ctx.array_root;
                    let saved_positions = ctx.positions.clone();
                    let saved_overflow = ctx.overflow_reported.clone();
                    ctx.reset_for_new_array(Some(element));
                    child_place.current_init_typ = element;

                    self.resolve_step(db, element, &mut child_place, body_ctx, ctx, step);

                    ctx.array_root = saved_root;
                    ctx.positions = saved_positions;
                    ctx.overflow_reported = saved_overflow;
                    ctx.roles.insert(*expr, BracketRole::Element);
                    ctx.advance(1);
                    self.check_bounds(db, *expr, ctx, ctx.current_pos());
                }
                _ => self.resolve_step(db, element, &mut child_place, body_ctx, ctx, step),
            }
        }
    }

    fn resolve_step(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expected: Type<'db>,
        place: &mut InitPlaceBuilder<'db>,
        body_ctx: &mut BodyInferenceResult<'db>,
        ctx: &mut InitContext<'db>,
        step: &'db InitExprWalkStep,
    ) {
        match step {
            InitExprWalkStep::ArrayInit { expr, values } => {
                expected.walk_init_expr(db, step, place, self);

                let resolved = self
                    .type_of_init_expr
                    .get(expr)
                    .copied()
                    .unwrap_or_default()
                    .normalize(db);

                match resolved {
                    Type::Array(array) => {
                        // Set array root if not already set
                        if ctx.array_root.is_none() {
                            ctx.array_root = Some(resolved);
                        };
                        // Where this bracket's values go, for the leaves: a
                        // row of the array from this dimension down. An
                        // element's own bracket is recorded by the array
                        // holding it instead.
                        if let Some(cells) = Self::cells_from(db, ctx.array_root, ctx.current_dim())
                        {
                            ctx.roles.insert(*expr, BracketRole::Row { cells });
                        }

                        let num_dims = array.subranges(db).len();
                        // Multi-dimensional bracket init: an inner bracket opens the next
                        // dimension. Detect brackets even when wrapped in a repetition
                        // (`n([..])`) — otherwise `[2([1,2,3])]` would type-check its inner
                        // bracket against the scalar element type and spuriously emit E0508
                        // (and the result would depend on whether a bracket sibling exists).
                        let has_inner_brackets = values.iter().any(step_contains_bracket);

                        if has_inner_brackets && num_dims > 1 && ctx.current_dim() < num_dims - 1 {
                            // Multi-dimensional bracket init: each inner bracket
                            // opens the next dimension and covers a whole
                            // sub-array, so it spends every cell below this one.
                            // Push/pop per child so each starts fresh.
                            let sub_array =
                                Self::cells_from(db, ctx.array_root, ctx.current_dim() + 1)
                                    .unwrap_or(1);
                            for child in values.iter() {
                                let mut child_place = *place;
                                // A repetition sits at THIS dimension — it is
                                // `2([1,2,3])`, two rows, not a row. It counts
                                // and descends on its own; opening a dimension
                                // around it would measure its rows against the
                                // width of one.
                                if matches!(child, InitExprWalkStep::SizedIndex { .. }) {
                                    self.resolve_step(
                                        db,
                                        expected,
                                        &mut child_place,
                                        body_ctx,
                                        ctx,
                                        child,
                                    );
                                    continue;
                                }
                                ctx.push_dimension();
                                self.resolve_step(
                                    db,
                                    expected,
                                    &mut child_place,
                                    body_ctx,
                                    ctx,
                                    child,
                                );
                                ctx.pop_dimension();
                                ctx.advance(sub_array);
                                self.check_bounds(db, *expr, ctx, ctx.current_pos());
                            }
                        } else {
                            // Single-dimensional, innermost dimension, or SizedIndex children.
                            let inner = array.of_type(db).infer(db);
                            self.resolve_elements(db, inner, place, body_ctx, ctx, values);
                        }
                    }
                    _ => {
                        self.resolve_steps(db, resolved, place, body_ctx, ctx, values);
                    }
                }
            }
            InitExprWalkStep::SizedIndex { expr, size, values } => {
                let repeat_count = size.with_case.as_u64(db).unwrap_or_else(|_| {
                    self.errors.push(
                        ArrayError::InvalidIndex { size: *size }
                            .to_diagnostic(db, self.scope.file(db)),
                    );
                    1
                }) as usize;

                // A repetition spends what it repeats, not one slot per count:
                // `[5(7(1))]` is 35 values, and `ARRAY[1..2, 3..4] :=
                // [2(10), 2(20)]` is the short form of four.
                let cells = repeat_count
                    * spent_cells(db, values, ctx.array_root, ctx.current_dim()).max(1);
                // Whether a bracket it repeats is a row of this array, or an
                // element that is itself an array.
                let rows = Self::cells_from(db, ctx.array_root, ctx.current_dim() + 1).is_some();

                // Check bounds before advancing
                let end_pos = ctx.current_pos() + cells;
                self.check_bounds(db, *expr, ctx, end_pos);
                ctx.advance(cells);

                // Process nested values at next dimension
                ctx.push_dimension();
                if rows {
                    self.resolve_steps(db, expected, place, body_ctx, ctx, values);
                } else {
                    self.resolve_elements(db, expected, place, body_ctx, ctx, values);
                }
                ctx.pop_dimension();
            }
            InitExprWalkStep::FieldInit { expr, values } => {
                expected.walk_init_expr(db, step, place, self);
                let expected = self
                    .type_of_init_expr
                    .get(expr)
                    .copied()
                    .unwrap_or_default()
                    .normalize(db);

                let expected = match expected {
                    Type::Array(array) => array.of_type(db).infer(db),
                    _ => expected,
                };

                // Clear seen fields for new struct
                ctx.clear_fields();

                // Save and restore context for struct
                let saved_root = ctx.array_root;
                let saved_positions = ctx.positions.clone();
                let saved_overflow = ctx.overflow_reported.clone();

                self.resolve_steps(db, expected, place, body_ctx, ctx, values);

                // Restore context
                ctx.array_root = saved_root;
                ctx.positions = saved_positions;
                ctx.overflow_reported = saved_overflow;

                // A struct is one element of the enclosing array, and that
                // count is checked HERE. It used to be caught by accident: a
                // scalar field leaked into the enclosing count and fired the
                // error with the caret on its own literal.
                ctx.advance(1);
                self.check_bounds(db, *expr, ctx, ctx.current_pos());
            }
            InitExprWalkStep::Field { expr, value, name } => {
                expected.walk_init_expr(db, step, place, self);
                let field_type = self
                    .type_of_init_expr
                    .get(expr)
                    .copied()
                    .unwrap_or_default()
                    .normalize(db);

                if let Some(prev) = ctx.seen_fields.insert(name.ident(db), *expr) {
                    self.errors.push(
                        DuplicateError::InitExprField {
                            name: name.with_case,
                            field1: *expr,
                            field2: prev,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                }

                // Each field walks in its own array context, restored after.
                // Resetting only for an ARRAY field left a scalar written
                // after one counting as that array's next element: `n := 1`
                // behind `v := [4, 5, 6]` was one too many for `v`.
                let saved_root = ctx.array_root;
                let saved_positions = ctx.positions.clone();
                let saved_overflow = ctx.overflow_reported.clone();
                let own_array =
                    matches!(field_type.normalize(db), Type::Array(_)).then_some(field_type);
                ctx.reset_for_new_array(own_array);

                self.resolve_step(db, field_type, place, body_ctx, ctx, value);

                ctx.array_root = saved_root;
                ctx.positions = saved_positions;
                ctx.overflow_reported = saved_overflow;
            }
            InitExprWalkStep::ConstantExpr { expr, value } => {
                // Type check the constant expression
                let mut infer_ctx = InferExprCtx::new(Resolver::for_scope(db, self.scope));
                // The declared type directs the initializer, so a
                // RETURN-overloaded call initializes by the declaration.
                infer_ctx.resolve_expr_expecting(db, *value, body_ctx, Some(expected));
                infer_ctx.check_expr(db, *value, body_ctx);

                if let Err(err) = infer_ctx.coerce_type_with_expr(db, expected, *value, body_ctx) {
                    self.errors.push(err.into_non_assignable_init(
                        db,
                        place.current_init_typ,
                        CallSite::from_scoped(db, expr),
                    ));
                } else if let Some(err) = expected.subrange_violation(db, *value) {
                    // An initializer is an assignment too: `VAR p : INT (0..100)
                    // := 200;` must be rejected like `p := 200`. Bounds are
                    // checked in every phase that assigns a value, not only in
                    // body inference.
                    self.errors.push(err.to_diagnostic(db, self.scope.file(db)));
                }
                if let Some(err) = body_ctx.ref_subrange_mismatch(db, expected, *value) {
                    self.errors.push(err.to_diagnostic(db, self.scope.file(db)));
                }
                if let Some(err) = body_ctx.ref_capacity_mismatch(db, expected, *value) {
                    self.errors.push(err.to_diagnostic(db, self.scope.file(db)));
                }

                // Advance position and check bounds
                ctx.advance(1);
                self.check_bounds(db, *expr, ctx, ctx.current_pos());
            }
        }
    }

    /// Cells reachable from `dimension` downwards — the product of that
    /// dimension and every one below it.
    ///
    /// An initializer's bracket nesting need not match the array's rank:
    /// `[1, 2, 3, 4, 5, 6]` and `[[1, 2, 3], [4, 5, 6]]` both fill
    /// `ARRAY[0..1, 0..2]` row-major, and `[2(10), 2(20)]` is the short form
    /// of four values whatever shape holds them. So everything is measured in
    /// CELLS from the current depth down, never in slots of one dimension —
    /// the flat form has six cells to fill, not the first dimension's two.
    ///
    /// Bounds are folded against the LIVE result (this runs inside init
    /// inference), through the same evaluator as the E0501/E0502 check.
    /// `as_range` bailed on a CONSTANT bound — and on a NEGATIVE literal one,
    /// so `ARRAY[-2..2]` never had its initializer length checked at all.
    fn cells_from(
        db: &'db dyn WorkspaceDataBase,
        array_root: Option<Type<'db>>,
        dimension: usize,
    ) -> Option<usize> {
        let Type::Array(array) = array_root?.normalize(db) else {
            return None;
        };
        let dims = crate::hir_ty::infer::const_eval::array_dimensions(db, array);
        let rest = dims.get(dimension..).filter(|rest| !rest.is_empty())?;
        rest.iter()
            .map(|(lower, upper)| Some(((*upper)? - (*lower)? + 1).max(0) as usize))
            .try_fold(1usize, |acc, len| Some(acc * len?))
    }

    fn check_bounds(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expr: InitExpr<'db>,
        ctx: &mut InitContext<'db>,
        end_position: usize,
    ) {
        if ctx.is_overflow_reported() {
            return;
        }

        let dim = ctx.current_dim();
        if let Some(array_size) = Self::cells_from(db, ctx.array_root, dim)
            && end_position > array_size
        {
            ctx.set_overflow_reported();
            self.errors.push(
                ArrayError::TooManyElements {
                    expr,
                    dimension: dim,
                    max_size: array_size,
                }
                .to_diagnostic(db, self.scope.file(db)),
            );
        }
    }
}

/// How many cells of the array being filled a written-out list of values
/// spends at dimension `dim`, following repetitions down: a value one, a
/// bracket a whole row while dimensions remain below `dim`, and one element
/// of an array of arrays otherwise. `[5(7(1))]` is 35, not 5, and `2([1])`
/// into a 2x3 is two rows, 6: counting its values let `[3([1])]` pass.
fn spent_cells<'db>(
    db: &'db dyn WorkspaceDataBase,
    values: &[InitExprWalkStep],
    root: Option<Type<'db>>,
    dim: usize,
) -> usize {
    values
        .iter()
        .map(|step| match step {
            InitExprWalkStep::SizedIndex { size, values, .. } => {
                size.with_case.as_u64(db).unwrap_or(1) as usize
                    * spent_cells(db, values, root, dim).max(1)
            }
            InitExprWalkStep::ArrayInit { .. } => {
                InitExprInferenceResult::cells_from(db, root, dim + 1).unwrap_or(1)
            }
            _ => 1,
        })
        .sum()
}

/// How the walk placed a nested bracket, which the leaves follow.
#[derive(Clone, Copy, Debug)]
enum BracketRole {
    /// A row of a multi-dimensional array, `cells` long: its values from
    /// where it starts, and what follows it a whole row later.
    Row { cells: usize },
    /// An element that is itself an array: one element of the array holding
    /// it, its values from its own first cell.
    Element,
}

/// Mutable context for tracking position during array init traversal
struct InitContext<'db> {
    /// Current position per dimension (index = dimension)
    positions: Vec<usize>,
    /// Root array type for bounds checking
    array_root: Option<Type<'db>>,
    /// Whether overflow has been reported per dimension
    overflow_reported: Vec<bool>,
    /// Seen fields in current struct (for duplicate detection)
    seen_fields: FxHashMap<Ident, InitExpr<'db>>,
    /// How each nested bracket is placed ([`BracketRole`]).
    roles: FxHashMap<InitExpr<'db>, BracketRole>,
}

impl<'db> InitContext<'db> {
    fn new(array_root: Option<Type<'db>>) -> Self {
        Self {
            positions: vec![0],
            array_root,
            overflow_reported: vec![false],
            seen_fields: FxHashMap::default(),
            roles: FxHashMap::default(),
        }
    }

    fn current_dim(&self) -> usize {
        self.positions.len() - 1
    }

    fn current_pos(&self) -> usize {
        self.positions[self.current_dim()]
    }

    fn advance(&mut self, count: usize) {
        let dim = self.current_dim();
        self.positions[dim] += count;
    }

    fn push_dimension(&mut self) {
        self.positions.push(0);
        self.overflow_reported.push(false);
    }

    fn pop_dimension(&mut self) {
        self.positions.pop();
        self.overflow_reported.pop();
    }

    fn set_overflow_reported(&mut self) {
        let dim = self.current_dim();
        self.overflow_reported[dim] = true;
    }

    /// Any depth, not just the current one. An over-long innermost repetition
    /// overflows every level that contains it, and the outermost check runs
    /// first — so `[2(3(5(1)))]` into a 2x3x4 reports its total once instead of
    /// once per dimension. Cleared per array by [`Self::reset_for_new_array`],
    /// so two over-filled fields of one struct still report separately.
    fn is_overflow_reported(&self) -> bool {
        self.overflow_reported.iter().any(|reported| *reported)
    }

    /// Reset position for entering a new array (e.g., struct field with array type)
    fn reset_for_new_array(&mut self, new_root: Option<Type<'db>>) {
        self.positions.clear();
        self.positions.push(0);
        self.overflow_reported.clear();
        self.overflow_reported.push(false);
        self.array_root = new_root;
    }

    fn clear_fields(&mut self) {
        self.seen_fields.clear();
    }
}

// The authoritative, flattened output of an initializer, derived from the
// init's bracket structure (brackets = nesting levels) and the place the
// diagnostic walk gave each nested bracket ([`BracketRole`]). It does no
// validation — that is the walk's job — so it never grows arms for
// type-mismatch handling. MIR consumes `ResolvedInit` leaves directly instead
// of re-walking the InitExpr tree.

/// Flatten an initializer into row-major (path, value) leaves at `path`.
fn resolve_leaves<'db>(
    db: &'db dyn WorkspaceDataBase,
    init: InitExpr<'db>,
    types: &FxHashMap<InitExpr<'db>, Type<'db>>,
    roles: &FxHashMap<InitExpr<'db>, BracketRole>,
    path: &mut Vec<InitPathStep>,
    out: &mut Vec<ResolvedInit<'db>>,
) {
    match init.kind(db) {
        InitExprKind::ConstantExpr(value) => {
            out.push(ResolvedInit {
                path: path.clone(),
                value,
            });
        }
        InitExprKind::StructInit { values } => {
            // The step carries the name as DECLARED, not as written. A
            // member may be initialized in any case (`(fld := 7)` for `Fld`,
            // `(LIMIT := 9)` for an instance's `limit`), and MIR matches these
            // against the declared names — so resolving here is what keeps the
            // two from diverging, rather than teaching MIR to fold a name HIR
            // has already resolved. The walk recorded, for each element, the
            // field or member it named, whatever holds it: a STRUCT, an FB or
            // CLASS instance, an array's element.
            for v in &values {
                if let InitExprKind::StructElement { name, value } = v.kind(db) {
                    let declared = match types.get(v) {
                        Some(Type::StructElement(field)) => Some(field.name(db)),
                        Some(Type::Variable((var, _))) => Some(var.name(db)),
                        _ => None,
                    };
                    path.push(InitPathStep::Field(declared.unwrap_or(name.ident(db))));
                    resolve_leaves(db, *value, types, roles, path, out);
                    path.pop();
                }
            }
        }
        InitExprKind::ArrayInit { .. } => {
            // A fresh row-major flat index for each array (nested/sub-arrays
            // restart at 0 — MIR offsets each by its own element_size).
            let mut flat = 0u32;
            resolve_array_into(db, init, types, roles, path, &mut flat, out);
        }
        // StructElement / ArrayIndexedElement only ever appear nested above.
        _ => {}
    }
}

/// Place each element of an array bracket at the next flat (row-major) index.
fn resolve_array_into<'db>(
    db: &'db dyn WorkspaceDataBase,
    bracket: InitExpr<'db>,
    types: &FxHashMap<InitExpr<'db>, Type<'db>>,
    roles: &FxHashMap<InitExpr<'db>, BracketRole>,
    path: &mut Vec<InitPathStep>,
    flat: &mut u32,
    out: &mut Vec<ResolvedInit<'db>>,
) {
    if let InitExprKind::ArrayInit { values } = bracket.kind(db) {
        for child in &values {
            array_element(db, *child, types, roles, path, flat, out);
        }
    }
}

/// One slot of an array at the current flat index, as the walk placed it: a
/// nested bracket is a row, from here and a whole row long however short it
/// is, or an element that is itself an array, one slot with its own index; a
/// repetition `x(y)` expands in place, and a scalar/struct element claims one
/// flat slot.
fn array_element<'db>(
    db: &'db dyn WorkspaceDataBase,
    elem: InitExpr<'db>,
    types: &FxHashMap<InitExpr<'db>, Type<'db>>,
    roles: &FxHashMap<InitExpr<'db>, BracketRole>,
    path: &mut Vec<InitPathStep>,
    flat: &mut u32,
    out: &mut Vec<ResolvedInit<'db>>,
) {
    match (elem.kind(db), roles.get(&elem)) {
        (InitExprKind::ArrayInit { .. }, Some(BracketRole::Element)) => {
            path.push(InitPathStep::ArrayElem(*flat));
            *flat += 1;
            resolve_leaves(db, elem, types, roles, path, out);
            path.pop();
        }
        (InitExprKind::ArrayInit { .. }, Some(BracketRole::Row { cells })) => {
            let start = *flat;
            resolve_array_into(db, elem, types, roles, path, flat, out);
            *flat = start + *cells as u32;
        }
        (InitExprKind::ArrayInit { .. }, None) => {
            resolve_array_into(db, elem, types, roles, path, flat, out)
        }
        (InitExprKind::ArrayIndexedElement { size, values }, _) => {
            let n = size.with_case.as_u64(db).unwrap_or(0);
            // `n()` repeats no value: n elements keep their default, and
            // what follows starts after them.
            if values.is_empty() {
                *flat += n as u32;
            }
            for _ in 0..n {
                for v in &values {
                    array_element(db, *v, types, roles, path, flat, out);
                }
            }
        }
        _ => {
            path.push(InitPathStep::ArrayElem(*flat));
            *flat += 1;
            resolve_leaves(db, elem, types, roles, path, out);
            path.pop();
        }
    }
}

/// Whether an init step contains a bracket (`ArrayInit`), even when wrapped in a
/// repetition group (`n([..])`). Routes multi-dim bracket init to the
/// dimension-descending branch regardless of repetition wrapping.
fn step_contains_bracket(step: &InitExprWalkStep) -> bool {
    match step {
        InitExprWalkStep::ArrayInit { .. } => true,
        InitExprWalkStep::SizedIndex { values, .. } => values.iter().any(step_contains_bracket),
        _ => false,
    }
}
