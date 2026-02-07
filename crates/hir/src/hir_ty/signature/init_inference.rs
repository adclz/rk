use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError, e6_array::ArrayError,
    },
    hir_def::{expressions::expression::InitExpr, interned::identifier::Ident, scope::ScopeId},
    hir_ty::{
        body::BodyInferenceResult,
        expr_store::InitExprWalkStep,
        infer::expr::InferExprCtx,
        resolver::{Resolver, walk::InitPlaceBuilder},
        ty::Type,
    },
};

/// Information about an element's position in an array initializer
#[derive(Debug, Clone, Copy, PartialEq, Eq, salsa::Update)]
pub struct ArrayElementPosition {
    /// The dimension this element is in (0 for first dimension, etc.)
    pub dimension: usize,
    /// Number of elements this initializer fills (1 for single values, N for N(value))
    pub count: usize,
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct InitExprInferenceResult<'db> {
    /// Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,
    /// Mapping of init expr to their resolved types
    pub type_of_init_expr: FxHashMap<InitExpr<'db>, Type<'db>>,
    /// Mapping of init expr to their position in array (if applicable)
    pub array_positions: FxHashMap<InitExpr<'db>, ArrayElementPosition>,
    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> InitExprInferenceResult<'db> {
    pub(crate) fn new(scope: ScopeId<'db>) -> Self {
        Self {
            type_of_init_expr: FxHashMap::default(),
            array_positions: FxHashMap::default(),
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

                let expected = self
                    .type_of_init_expr
                    .get(expr)
                    .copied()
                    .unwrap_or_default()
                    .normalize(db);

                let expected = match expected {
                    Type::Array(array) => {
                        // Set array root if not already set
                        if ctx.array_root.is_none() {
                            ctx.array_root = Some(expected);
                        };
                        Type::new_spec(db, array.of_type(db))
                    }
                    _ => expected,
                };

                self.resolve_steps(db, expected, place, body_ctx, ctx, values);
            }
            InitExprWalkStep::SizedIndex { expr, size, values } => {
                let repeat_count = size.as_u64(db).unwrap_or_else(|err| {
                    self.errors.push(
                        ArrayError::InvalidIndex {
                            size: *size,
                            err: err.to_string(),
                        }
                        .to_diagnostic(db),
                    );
                    1
                }) as usize;

                // Record position information
                if ctx.array_root.is_some() {
                    self.array_positions.insert(
                        *expr,
                        ArrayElementPosition {
                            dimension: ctx.current_dim(),
                            count: repeat_count,
                        },
                    );
                }

                // Check bounds before advancing
                let end_pos = ctx.current_pos() + repeat_count;
                self.check_bounds(db, *expr, ctx, end_pos);
                ctx.advance(repeat_count);

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
                    Type::Array(array) => Type::new_spec(db, array.of_type(db)),
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

                // Struct counts as 1 element in parent array
                ctx.advance(1);
            }
            InitExprWalkStep::Field { expr, value, name } => {
                expected.walk_init_expr(db, step, place, self);
                let field_type = self
                    .type_of_init_expr
                    .get(expr)
                    .copied()
                    .unwrap_or_default()
                    .normalize(db);

                if let Some(prev) = ctx.seen_fields.insert(name.ident, *expr) {
                    self.errors.push(
                        DuplicateError::InitExprField {
                            name: name.ident,
                            field1: prev,
                            field2: *expr,
                        }
                        .to_diagnostic(db),
                    );
                }

                // If field is an array, reset context for it
                let value_is_array = matches!(field_type.normalize(db), Type::Array(_));
                if value_is_array {
                    ctx.reset_for_new_array(Some(field_type));
                }

                self.resolve_step(db, field_type, place, body_ctx, ctx, value);
            }
            InitExprWalkStep::ConstantExpr { expr, value } => {
                // Record position information
                if ctx.array_root.is_some() {
                    self.array_positions.insert(
                        *expr,
                        ArrayElementPosition {
                            dimension: ctx.current_dim(),
                            count: 1,
                        },
                    );
                }

                // Type check the constant expression
                let mut infer_ctx = InferExprCtx::new(Resolver::for_scope(db, self.scope));
                infer_ctx.resolve_expr(db, *value, body_ctx);
                infer_ctx.check_expr(db, *value, body_ctx);

                if let Err(err) = infer_ctx.coerce_type_with_expr(db, expected, *value, body_ctx) {
                    self.errors.push(err.into_non_assignable(
                        db,
                        place.current_init_typ,
                        CallSite::from_scoped(db, expr),
                    ));
                }

                // Advance position and check bounds
                ctx.advance(1);
                self.check_bounds(db, *expr, ctx, ctx.current_pos());
            }
        }
    }

    fn get_array_bounds(
        db: &'db dyn WorkspaceDataBase,
        array_root: Option<Type<'db>>,
        dimension: usize,
    ) -> Option<(u64, u64, usize)> {
        let arr_ty = array_root?;
        if let Type::Array(array) = arr_ty.normalize(db) {
            let subranges = array.subranges(db);
            if dimension < subranges.len() {
                let current_range = &subranges[dimension];
                let lower = current_range.0.as_range(db)?;
                let upper = current_range.1.as_range(db)?;
                return Some((lower, upper, (upper - lower + 1) as usize));
            }
        }
        None
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
        if let Some((_lower, _upper, array_size)) = Self::get_array_bounds(db, ctx.array_root, dim)
            && end_position > array_size
        {
            ctx.set_overflow_reported();
            self.errors.push(
                ArrayError::TooManyElements {
                    expr,
                    dimension: dim,
                    max_size: array_size,
                }
                .to_diagnostic(db),
            );
        }
    }
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

    fn is_overflow_reported(&self) -> bool {
        self.overflow_reported[self.current_dim()]
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
