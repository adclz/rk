use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{ToIdeDiagnostic, e01_duplicates::DuplicateError, e05_array::ArrayError},
    hir_def::{
        expressions::expression::{Expr, InitExpr, InitExprKind},
        interned::identifier::{CaselessIdent, Ident},
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

    /// BodyInference results (Inference of constant expressions)
    pub body_infer_result: BodyInferenceResult<'db>,

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
            };
        }

        self.check_variables(db);
        self.check_usings(db);
        self.check_methods(db);
        self.check_function_specifier(db);

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
                            }
                            .to_diagnostic(db, self.scope.file(db)),
                        );
                    }
                }
            }
        }

        for error in &self.init_expr_result.errors {
            self.errors.push(error.clone());
        }

        for error in &self.body_infer_result.errors {
            self.errors.push(error.clone());
        }
        self
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
        self.resolve_steps(db, typ, &mut place, body_ctx, &mut ctx, map);

        // Produce the authoritative, flattened resolved leaves. This is a pure,
        // type-DIRECTED-by-structure pass (brackets = nesting): it flattens the
        // initializer into row-major (path, value) leaves for MIR to consume.
        // Validation lives in the walk above; this never re-validates.
        let mut leaves = Vec::new();
        resolve_leaves(
            db,
            expr,
            &self.type_of_init_expr,
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
                            self.resolve_steps(db, inner, place, body_ctx, ctx, values);
                        }
                    }
                    _ => {
                        self.resolve_steps(db, resolved, place, body_ctx, ctx, values);
                    }
                }
            }
            InitExprWalkStep::SizedIndex { expr, size, values } => {
                let repeat_count = size.as_u64(db).unwrap_or_else(|err| {
                    self.errors.push(
                        ArrayError::InvalidIndex {
                            size: *size,
                            err: err.to_string(),
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                    1
                }) as usize;

                // A repetition spends what it repeats, not one slot per count:
                // `[5(7(1))]` is 35 values, and `ARRAY[1..2, 3..4] :=
                // [2(10), 2(20)]` is the short form of four.
                let cells = repeat_count * init_cells(db, values).max(1);

                // Check bounds before advancing
                let end_pos = ctx.current_pos() + cells;
                self.check_bounds(db, *expr, ctx, end_pos);
                ctx.advance(cells);

                // Process nested values at next dimension
                ctx.push_dimension();
                self.resolve_steps(db, expected, place, body_ctx, ctx, values);
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

                if let Some(prev) = ctx.seen_fields.insert(name.ident.caseless(db), *expr) {
                    self.errors.push(
                        DuplicateError::InitExprField {
                            name: name.ident,
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

/// How many cells a written-out list of initializer elements fills, following
/// repetitions and brackets down. `[5(7(1))]` is 35, not 5.
fn init_cells(db: &dyn WorkspaceDataBase, values: &[InitExprWalkStep]) -> usize {
    values
        .iter()
        .map(|step| match step {
            InitExprWalkStep::SizedIndex { size, values, .. } => {
                size.as_u64(db).unwrap_or(1) as usize * init_cells(db, values).max(1)
            }
            InitExprWalkStep::ArrayInit { values, .. } => init_cells(db, values).max(1),
            _ => 1,
        })
        .sum()
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
    seen_fields: FxHashMap<CaselessIdent, InitExpr<'db>>,
}

impl<'db> InitContext<'db> {
    fn new(array_root: Option<Type<'db>>) -> Self {
        Self {
            positions: vec![0],
            array_root,
            overflow_reported: vec![false],
            seen_fields: FxHashMap::default(),
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

// The authoritative, flattened output of an initializer, derived PURELY from
// the init's bracket structure (brackets = nesting levels). It does no
// validation — that is the diagnostic walk's job — so it never grows arms for
// type-mismatch handling. MIR consumes `ResolvedInit` leaves directly instead
// of re-walking the InitExpr tree.

/// Flatten an initializer into row-major (path, value) leaves at `path`.
fn resolve_leaves<'db>(
    db: &'db dyn WorkspaceDataBase,
    init: InitExpr<'db>,
    types: &FxHashMap<InitExpr<'db>, Type<'db>>,
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
            // field may be initialized in any case (`(fld := 7)` for `Fld`),
            // and MIR matches these against the declared field names — so
            // resolving here is what keeps the two from diverging, rather
            // than teaching MIR to fold a name HIR has already resolved.
            let struct_ty = types.get(&init).copied().map(|t| t.normalize(db));
            for v in &values {
                if let InitExprKind::StructElement { name, value } = v.kind(db) {
                    let declared = match struct_ty {
                        Some(Type::Struct(st)) => st
                            .struct_elements(db)
                            .get(&name.ident.caseless(db))
                            .map(|field| field.name(db)),
                        _ => None,
                    };
                    path.push(InitPathStep::Field(declared.unwrap_or(name.ident)));
                    resolve_leaves(db, *value, types, path, out);
                    path.pop();
                }
            }
        }
        InitExprKind::ArrayInit { .. } => {
            // A fresh row-major flat index for each array (nested/sub-arrays
            // restart at 0 — MIR offsets each by its own element_size).
            let mut flat = 0u32;
            resolve_array_into(db, init, types, path, &mut flat, out);
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
    path: &mut Vec<InitPathStep>,
    flat: &mut u32,
    out: &mut Vec<ResolvedInit<'db>>,
) {
    if let InitExprKind::ArrayInit { values } = bracket.kind(db) {
        for child in &values {
            array_element(db, *child, types, path, flat, out);
        }
    }
}

/// One slot of an array at the current flat index: a nested bracket continues
/// the same flat counter (next dimension), a repetition `x(y)` expands in place,
/// and a scalar/struct element claims one flat slot.
fn array_element<'db>(
    db: &'db dyn WorkspaceDataBase,
    elem: InitExpr<'db>,
    types: &FxHashMap<InitExpr<'db>, Type<'db>>,
    path: &mut Vec<InitPathStep>,
    flat: &mut u32,
    out: &mut Vec<ResolvedInit<'db>>,
) {
    match elem.kind(db) {
        InitExprKind::ArrayInit { .. } => resolve_array_into(db, elem, types, path, flat, out),
        InitExprKind::ArrayIndexedElement { size, values } => {
            let n = size.as_u64(db).unwrap_or(0);
            for _ in 0..n {
                for v in &values {
                    array_element(db, *v, types, path, flat, out);
                }
            }
        }
        _ => {
            path.push(InitPathStep::ArrayElem(*flat));
            *flat += 1;
            resolve_leaves(db, elem, types, path, out);
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
