use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    CallSite,
    check::errors::{analysis_error::ToIdeDiagnostic, init_inference::InitInferenceError},
    hir_def::{
        expressions::expression::{InitExpr, InitExprKind},
        pous::{data_type::DataType, variable::VariableDecl},
        scope::ScopeId,
    },
    hir_ty::{
        body_inference::BodyInferenceResult, def_map::FxIndexMap, expr_store::InitExprWalkStep,
        infer::expr::InferExprCtx, resolver::Resolver, ty::Type,
    },
};

#[salsa::tracked(returns(ref), no_eq)]
pub fn infer_variable<'db>(
    db: &'db dyn WorkspaceDataBase,
    variable: VariableDecl<'db>,
) -> InitExprInferenceResult<'db> {
    let typ = Type::new_spec(db, variable.spec(db));
    let mut infer = InitExprInferenceResult::new(db, variable.scope_id(db));
    if let Some(expr) = variable.init(db) {
        infer.resolve_init_expr(db, expr, typ);
    };
    infer
}

#[salsa::tracked(returns(ref), no_eq)]
pub fn infer_data_type<'db>(
    db: &'db dyn WorkspaceDataBase,
    data_type: DataType<'db>,
) -> InitExprInferenceResult<'db> {
    let typ = Type::new_spec(db, data_type.spec(db));
    let mut infer = InitExprInferenceResult::new(db, data_type.scope_id(db));
    if let Some(expr) = data_type.init(db) {
        infer.resolve_init_expr(db, expr, typ);
    };
    infer
}

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
    pub type_of_expr: FxHashMap<InitExpr<'db>, Type<'db>>,
    /// Mapping of init expr to their position in array (if applicable)
    pub array_positions: FxHashMap<InitExpr<'db>, ArrayElementPosition>,
    /// BodyInference results (Inference of constant expressions)
    pub body_infer_result: BodyInferenceResult<'db>,
    /// Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> InitExprInferenceResult<'db> {
    pub fn new(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Self {
        Self {
            type_of_expr: FxHashMap::default(),
            array_positions: FxHashMap::default(),
            scope,
            body_infer_result: BodyInferenceResult::new(scope),
            errors: Vec::new(),
        }
    }

    pub fn resolve_init_expr(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expr: InitExpr<'db>,
        typ: Type<'db>,
    ) {
        let map = expr.flatten(db);
        let normalized = typ.normalize(db);
        let array_root = matches!(normalized, Type::Array(_)).then_some(typ);
        let mut ctx = ArrayInitContext::new(array_root);
        self.resolve_expr(db, expr, typ, &mut ctx, map);
    }

    fn resolve_expr(
        &mut self,
        db: &'db dyn WorkspaceDataBase,
        expr: InitExpr<'db>,
        expected: Type<'db>,
        ctx: &mut ArrayInitContext<'db>,
        map: &FxIndexMap<InitExpr<'db>, InitExprWalkStep<'db>>,
    ) {
        let step = map.get(&expr).copied();
        let narrowed = match step {
            Some(s) => expected.walk_init_expr(db, expr, s, self),
            None => expected,
        };
        self.type_of_expr.insert(expr, narrowed);

        match expr.kind(db) {
            InitExprKind::StructInit { values } => {
                // Save current context for struct fields that may have their own arrays
                let saved_root = ctx.array_root;
                let saved_positions = ctx.positions.clone();
                let saved_overflow = ctx.overflow_reported.clone();

                for v in values {
                    // Each struct field gets a fresh array context if it's an array type
                    let field_is_array = matches!(v.kind(db), InitExprKind::StructElement { .. })
                        && {
                            // Check if this field's type is an array
                            let field_step = map.get(&v).copied();
                            let field_narrowed = match field_step {
                                Some(s) => narrowed.walk_init_expr(db, v, s, self),
                                None => narrowed,
                            };
                            matches!(field_narrowed.normalize(db), Type::Array(_))
                        };

                    if field_is_array {
                        // Will be set properly in StructElement
                        ctx.reset_for_new_array(None);
                    }
                    self.resolve_expr(db, v, narrowed, ctx, map);
                }

                // Restore context after processing struct
                ctx.array_root = saved_root;
                ctx.positions = saved_positions;
                ctx.overflow_reported = saved_overflow;

                // A struct init counts as 1 element in the parent array
                ctx.advance(1);
            }

            InitExprKind::StructElement { value, .. } => {
                // Get the actual field type (narrowed might be StructElement, we need its inner type)
                let field_type = narrowed.normalize(db);

                // If the field value is an array, set up array context for it
                let value_is_array = matches!(field_type.normalize(db), Type::Array(_));
                if value_is_array {
                    ctx.reset_for_new_array(Some(field_type));
                }
                self.resolve_expr(db, *value, field_type, ctx, map);
            }

            InitExprKind::ArrayInit { values } => {
                // Set array root if not already set
                if ctx.array_root.is_none() && matches!(expected.normalize(db), Type::Array(_)) {
                    ctx.array_root = Some(expected);
                }

                for v in values {
                    self.resolve_expr(db, v, narrowed, ctx, map);

                    // Check bounds for non-indexed elements (they advance position themselves)
                    // StructInit also advances, so check after it too
                    if !matches!(v.kind(db), InitExprKind::ArrayIndexedElement { .. }) {
                        self.check_bounds(db, v, ctx, ctx.current_pos());
                    }
                }
            }

            InitExprKind::ArrayIndexedElement { values, size } => {
                let repeat_count = size.as_u64(db).unwrap_or_else(|err| {
                    self.errors.push(
                        InitInferenceError::InvalidIndex {
                            size,
                            err: err.to_string(),
                        }
                        .to_diagnostic(db),
                    );
                    1
                }) as usize;

                // Record position information for this indexed element
                if ctx.array_root.is_some() {
                    self.array_positions.insert(
                        expr,
                        ArrayElementPosition {
                            dimension: ctx.current_dim(),
                            count: repeat_count,
                        },
                    );
                }

                // Check bounds before advancing
                let end_pos = ctx.current_pos() + repeat_count;
                self.check_bounds(db, expr, ctx, end_pos);
                ctx.advance(repeat_count);

                // Process nested values at next dimension
                ctx.push_dimension();
                for v in values {
                    self.resolve_expr(db, v, narrowed, ctx, map);
                }
                ctx.pop_dimension();
            }

            InitExprKind::ConstantExpr(rhs) => {
                // Record position information for this constant (single element)
                if ctx.array_root.is_some() {
                    self.array_positions.insert(
                        expr,
                        ArrayElementPosition {
                            dimension: ctx.current_dim(),
                            count: 1,
                        },
                    );
                }

                let mut infer_ctx = InferExprCtx::new(Resolver::for_scope(db, self.scope));
                infer_ctx.resolve_expr(db, rhs, &mut self.body_infer_result);
                infer_ctx.check_expr(db, rhs, &mut self.body_infer_result);

                if let Err(err) =
                    infer_ctx.coerce_type_with_expr(db, expected, rhs, &mut self.body_infer_result)
                {
                    self.errors.push(err.into_non_assignable(
                        db,
                        expected,
                        CallSite::from_init_expr(db, expr),
                    ));
                }

                ctx.advance(1);
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
        ctx: &mut ArrayInitContext<'db>,
        end_position: usize,
    ) {
        if ctx.is_overflow_reported() {
            return;
        }

        let dim = ctx.current_dim();
        if let Some((_lower, _upper, array_size)) = Self::get_array_bounds(db, ctx.array_root, dim)
        {
            if end_position > array_size {
                ctx.set_overflow_reported();
                self.errors.push(
                    InitInferenceError::TooManyElements {
                        expr,
                        dimension: dim,
                        max_size: array_size,
                    }
                    .to_diagnostic(db),
                );
            }
        }
    }
}

/// Mutable context for tracking position during array init traversal
struct ArrayInitContext<'db> {
    /// Current position per dimension (index = dimension)
    positions: Vec<usize>,
    /// Root array type for bounds checking
    array_root: Option<Type<'db>>,
    /// Whether overflow has been reported per dimension
    overflow_reported: Vec<bool>,
}

impl<'db> ArrayInitContext<'db> {
    fn new(array_root: Option<Type<'db>>) -> Self {
        Self {
            positions: vec![0],
            array_root,
            overflow_reported: vec![false],
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
}
