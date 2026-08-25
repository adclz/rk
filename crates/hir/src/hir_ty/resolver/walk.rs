use std::cmp::Ordering;

use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic, e2_resolve::ResolveError, e10_control_flow::ControlFlowError,
    },
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, InitExpr, MultibitsPart, PathExpr, PathExprKind},
            invocation::{Invocation, InvocationKind},
            spec::StructElement,
        },
        interned::identifier::{Ident, SpanIdent},
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::{Adjustment, AdjustmentInfo, BodyInferenceResult, NullState},
        expr_store::{InitExprWalkStep, PathExprWalkStep},
        head::{
            inheritance::{MethodRef, inherited_methods, instance_members},
            init_inference::InitExprInferenceResult,
        },
        infer::Infer,
        resolver::{Resolver, invocation::resolve_invocation, visibility::check_visibility},
        ty::{Size, Type},
    },
};

/// When walking a path expression, we need to keep track of the parent type.
///
/// The parent type is either a variable or a data type.
/// This is only useful to create accurate diagnostics.
#[derive(Debug, Copy, Clone)]
pub struct PathPlaceBuilder<'db> {
    pub current_typ: Type<'db>,
    pub current_path: PathExpr<'db>,
}

/// When walking an init expression, we need to keep track of the parent type.
///
/// The parent type is either a variable or a data type.
/// This is only useful to create accurate diagnostics.
#[derive(Debug, Copy, Clone)]
pub struct InitPlaceBuilder<'db> {
    pub current_init_typ: Type<'db>,
}

/// Result of looking up a field by name on a type.
pub(crate) enum FieldLookup<'db> {
    StructElement(StructElement<'db>),
    Variable(VariableDecl<'db>),
    Method(MethodRef<'db>),
    NotFound,
}

/// Check that a multibit access offset is within the bounds of the base it
/// slices.
///
/// `base_type` is the type the slice actually applies to, which is NOT always
/// a declaration's: `arr[k].7` slices the ELEMENT and `s.fld.15` the FIELD.
/// Measuring those against the array or the struct let every out-of-range
/// offset through, and lowering — which does read the element's width — then
/// failed with an internal compiler error on a body `check` had passed.
pub(crate) fn check_multibits_bounds<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: PathExpr<'db>,
    base_type: Type<'db>,
    var: Option<VariableDecl<'db>>,
    multibits: MultibitsPart,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let base_type = base_type.normalize(db);
    let Size::Size(base_bits) = base_type.get_size() else {
        return;
    };

    let Some(slice) = crate::hir_ty::infer::normalize::multibits_slice(db, multibits) else {
        // A size character naming no slice reaches lowering as a BOOL that
        // lowering cannot emit, so it is refused here instead.
        if let MultibitsPart::AccessOffset { access, .. } = multibits
            && let Some(c) = access.text(db).chars().next()
            && crate::hir_ty::infer::normalize::access_size(c).is_none()
        {
            ctx.errors.push(
                ResolveError::UnknownMultibitsAccess {
                    expr,
                    access: access.text(db).clone(),
                }
                .to_diagnostic(db, ctx.scope.file(db)),
            );
        }
        return;
    };
    let (access_bits, offset_val) = (slice.width, slice.index);

    // The access occupies `access_bits` starting at position `offset_val * access_bits`.
    // Valid when: (offset_val + 1) * access_bits <= base_bits
    if (offset_val + 1) * access_bits > base_bits {
        let max_offset = (access_bits <= base_bits).then(|| base_bits / access_bits - 1);
        ctx.errors.push(
            ResolveError::MultibitsOutOfRange {
                expr,
                var,
                offset: offset_val,
                access_bits,
                max_offset,
                base_type,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
    }
}

/// The FB/Class that declares a method — its enclosing POU, reached via the
/// method scope's parent. Used to resolve bare member access inside a method as
/// an implicit `THIS`.
fn method_owner_pou<'db>(db: &'db dyn WorkspaceDataBase, m: MethodRef<'db>) -> Option<Pou<'db>> {
    let parent = get_scope(db, m.get_scope_id(db)).parent?;
    match get_scope(db, parent).kind {
        ScopeKind::Pou(pou) => Some(pou),
        _ => None,
    }
}

impl<'db> Type<'db> {
    /// Peel through `Variable`, `DataType` and `StructElement` wrappers,
    /// returning the inner spec type and any multibits qualifier.
    fn peel_to_spec(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<(Type<'db>, Option<MultibitsPart>)> {
        match self {
            Type::Variable((v, multibits)) => Some((v.spec(db).infer(db), *multibits)),
            Type::DataType(dt) => Some((dt.spec(db).infer(db), None)),
            Type::StructElement(st) => Some((st.spec(db).infer(db), None)),
            _ => None,
        }
    }

    /// If this type can have variables/methods (Function / FunctionBlock / Class / Program / Method),
    /// return its scope id.
    fn as_walkable_scope(&self, db: &'db dyn WorkspaceDataBase) -> Option<ScopeId<'db>> {
        match self {
            Type::Function(f) => Some(f.scope_id(db)),
            Type::FunctionBlock(fb) => Some(fb.scope_id(db)),
            Type::Class(c) => Some(c.scope_id(db)),
            Type::Interface(i) => Some(i.scope_id(db)),
            Type::Program(p) => Some(p.scope_id(db)),
            Type::MethodDecl(m) => Some(m.get_scope_id(db)),
            _ => None,
        }
    }

    /// Resolve a named field on this (concrete) type.
    pub(crate) fn resolve_field(
        &self,
        db: &'db dyn WorkspaceDataBase,
        name: &Ident,
    ) -> FieldLookup<'db> {
        match self {
            Type::Struct(st) => match st.struct_elements(db).get(&name.fold(db)) {
                Some(field) => FieldLookup::StructElement(*field),
                None => FieldLookup::NotFound,
            },
            _ => {
                if let Some(scope) = self.as_walkable_scope(db) {
                    let def_map = scope.def_map(db);
                    if let Some(var) = def_map.global_variables.get(&name.fold(db)) {
                        FieldLookup::Variable(*var)
                    } else if let Some(m) = def_map.declared_methods.get(&name.fold(db)) {
                        FieldLookup::Method(*m)
                    } else if let Some(pou) = self.as_pou(db) {
                        // Inherited state and behaviour both come from the
                        // EXTENDS chain. `instance_members` is HIR's own
                        // resolved member list — the same one the MIR layout is
                        // built from — so an inherited field is reachable
                        // through an instance (`derived.base_field`) exactly
                        // where the layout says it lives.
                        if let Some(member) = instance_members(db, pou)
                            .iter()
                            .find(|m| m.var.name(db).fold(db) == name.fold(db))
                        {
                            FieldLookup::Variable(member.var)
                        } else {
                            match inherited_methods(db, pou).methods.get(&name.fold(db)) {
                                Some(inherited) => FieldLookup::Method(inherited.method),
                                None => FieldLookup::NotFound,
                            }
                        }
                    } else if let Type::MethodDecl(m) = self {
                        // Inside a method body, a bare name that isn't one of the
                        // method's own locals/params/return resolves to a MEMBER of
                        // the enclosing FB/Class — an implicit THIS. Method-locals
                        // (checked above) shadow members, as in IEC.
                        // Recurse on the owner so its own members AND inherited ones
                        // are covered.
                        match method_owner_pou(db, *m) {
                            Some(owner) => Type::new_pou(db, owner).resolve_field(db, name),
                            None => FieldLookup::NotFound,
                        }
                    } else {
                        FieldLookup::NotFound
                    }
                } else {
                    FieldLookup::NotFound
                }
            }
        }
    }
}

impl<'db> Type<'db> {
    pub fn walk_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: BeginPathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        // A begin path expr can either have:
        // - an invocation
        // - a path expression
        // - an invocation and a path expression

        if let Some(invocation) = expr.invocation(db) {
            let Some(pou) = resolve_invocation(db, ctx.scope, invocation, ctx) else {
                return;
            };

            if let Some(path_expr) = expr.expr(db) {
                self.walk_invocation_path(db, invocation, pou, path_expr, multibits, ctx);
            }
            return;
        }

        if let Some(path) = expr.expr(db) {
            Resolver::for_scope(db, path.scope_id(db))
                .resolve_path_steps(*self, db, path, multibits, ctx)
        }
    }

    /// Walk a path that follows an invocation keyword (`THIS`, `SUPER`, `SUPER()`).
    ///
    /// ```text
    /// CLASS:
    ///   9a THIS:    Reference to own methods
    ///   9b SUPER:   Access reference to method in base class
    /// FUNCTION BLOCKS:
    ///  10a THIS:    Reference to own methods
    ///  10b SUPER:   Access reference to method in base function block
    ///  10c SUPER(): Access reference to body in base function block
    /// ```
    fn walk_invocation_path(
        &self,
        db: &'db dyn WorkspaceDataBase,
        invocation: Invocation<'db>,
        pou: Pou<'db>,
        path_expr: PathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match invocation.kind(db) {
            InvocationKind::This => {
                self.walk_this_path(db, invocation, pou, path_expr, multibits, ctx);
            }
            InvocationKind::Super => {
                self.walk_super_path(db, pou, path_expr, ctx);
            }
            InvocationKind::SuperBody => {}
        }
    }

    fn walk_this_path(
        &self,
        db: &'db dyn WorkspaceDataBase,
        invocation: Invocation<'db>,
        pou: Pou<'db>,
        path_expr: PathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let mut current = Type::new_pou(db, pou);
        let steps = path_expr.flatten(db);
        let mut place = PathPlaceBuilder {
            current_typ: current,
            current_path: path_expr,
        };

        for step in steps {
            current.walk_path_expr(db, true, step, multibits, &mut place, ctx);
            // it is necessary to apply adjustments at each step
            current = ctx.type_of_path_expr_with_adjustments(step.get_expr(db));
        }

        if current.is_never() {
            return;
        }

        match current {
            Type::MethodDecl(m) => {
                check_visibility(db, &invocation.as_call_site(db), m, &mut ctx.errors);
                ctx.type_of_path_expr.insert(path_expr, Type::MethodDecl(m));
            }
            Type::Variable((var, multibits)) => {
                ctx.type_of_path_expr
                    .insert(path_expr, Type::new_var_with_multibits(db, var, multibits));
            }
            // Anything else that survived the walk is a RESOLVED end that is
            // not a bare FB variable: an indexed element (`THIS.a[n]`), a
            // struct member (`THIS.p.x`). A failed step records nothing, so
            // `current` defaults to `Never` and returns above — this arm is
            // only reachable when every step resolved. Each step already
            // recorded its type and adjustments, and the outermost step IS
            // this path expr, so there is nothing left to record. (This arm
            // used to be an error, which rejected every valid THIS path that
            // did not end directly at an FB variable or method.)
            _ => {}
        }
    }

    fn walk_super_path(
        &self,
        db: &'db dyn WorkspaceDataBase,
        pou: Pou<'db>,
        path_expr: PathExpr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let inherited = inherited_methods(db, pou);
        let current = Type::new_pou(db, pou);
        let steps = path_expr.flatten(db);

        if let Some(PathExprWalkStep::Field { ident, expr }) = steps.first() {
            if let Some(method) = inherited.methods.get(&ident.ident.fold(db)) {
                check_visibility(db, &ident.as_call_site(db), method.method, &mut ctx.errors);
                ctx.type_of_path_expr
                    .insert(*expr, Type::MethodDecl(method.method));
            }
            //fixme: Should SUPER allow access to variables in the base POU?
            else {
                ctx.errors.push(
                    ResolveError::NoSuchFieldPathExpr {
                        expr: path_expr,
                        ident: **ident,
                        ty: current,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }
        }
    }
}

impl<'db> Type<'db> {
    pub fn walk_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        report_errors: bool,
        step: &'db PathExprWalkStep<'db>,
        multibits: Option<MultibitsPart>,
        place: &mut PathPlaceBuilder<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        // Peel through Variable / DataType / StructElement wrappers first.
        if let Some((inner, mb)) = self.peel_to_spec(db) {
            return inner.walk_path_expr(db, report_errors, step, mb.or(multibits), place, ctx);
        }

        match step {
            PathExprWalkStep::Field { ident, .. } => {
                self.walk_field(
                    db,
                    report_errors,
                    step.get_expr(db),
                    ident,
                    multibits,
                    place,
                    ctx,
                );
            }
            PathExprWalkStep::Deref { count, .. } => {
                self.walk_deref(db, report_errors, step.get_expr(db), *count, place, ctx);
            }
            PathExprWalkStep::Index { .. } => {
                self.walk_index(db, report_errors, step.get_expr(db), place, ctx);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_field(
        &self,
        db: &'db dyn WorkspaceDataBase,
        report_errors: bool,
        expr: PathExpr<'db>,
        ident: &SpanIdent<'db>,
        multibits: Option<MultibitsPart>,
        place: &mut PathPlaceBuilder<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match self.resolve_field(db, &ident.ident) {
            FieldLookup::StructElement(field) => {
                let ty = Type::StructElement(field);
                ctx.type_of_path_expr.insert(expr, ty);
                place.current_typ = ty;
                place.current_path = expr;
            }
            FieldLookup::Variable(var) => {
                if let Some(mb) = multibits {
                    let base = var.spec(db).infer(db);
                    check_multibits_bounds(db, expr, base, Some(var), mb, ctx);
                }
                let ty = Type::new_var_with_multibits(db, var, multibits);
                ctx.type_of_path_expr.insert(expr, ty);
                ctx.variables_used.insert(var);
                place.current_typ = ty;
                place.current_path = expr;
            }
            FieldLookup::Method(m) => {
                ctx.type_of_path_expr.insert(expr, Type::MethodDecl(m));
                place.current_typ = Type::MethodDecl(m);
                place.current_path = expr;
                check_visibility(db, &ident.as_call_site(db), m, &mut ctx.errors);
            }
            FieldLookup::NotFound => {
                if report_errors && !self.is_never() {
                    ctx.errors.push(
                        ResolveError::NoSuchFieldPathExpr {
                            expr,
                            ident: **ident,
                            ty: place.current_typ,
                        }
                        .to_diagnostic(db, ctx.scope.file(db)),
                    );
                }
            }
        }
    }

    fn walk_deref(
        &self,
        db: &'db dyn WorkspaceDataBase,
        report_errors: bool,
        expr: PathExpr<'db>,
        count: u16,
        place: &mut PathPlaceBuilder<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        // Check nullability: extract the variable from the previous path step
        if report_errors {
            let maybe_var = ctx
                .type_of_path_expr
                .get(&place.current_path)
                .and_then(|t| {
                    if let Type::Variable((var, _)) = t {
                        Some(*var)
                    } else {
                        None
                    }
                });
            if let Some(var) = maybe_var {
                let state = ctx.ref_null_state.get(&var).copied();
                if let Some(state @ (NullState::Null(_) | NullState::Uninitialized(_))) = state {
                    ctx.errors.push(
                        ControlFlowError::DerefPossiblyNull { var, expr, state }
                            .to_diagnostic(db, ctx.scope.file(db)),
                    );
                }
            }
        }

        for result in iter_deref_types(db, *self).take(count as usize) {
            match result {
                Ok(ty) => {
                    ctx.path_expr_adjustments
                        .entry(place.current_path)
                        .or_default()
                        .push(Adjustment::new_deref(db, ty));

                    ctx.type_of_path_expr.insert(expr, place.current_typ);
                    ctx.path_expr_adjustments
                        .entry(expr)
                        .or_default()
                        .push(Adjustment::new_deref(db, ty));
                }
                Err(non_ref) => {
                    if report_errors {
                        ctx.errors.push(
                            ResolveError::DerefNonRefType { expr, ty: non_ref }
                                .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }
                    break;
                }
            }
        }
    }

    fn walk_index(
        &self,
        db: &'db dyn WorkspaceDataBase,
        report_errors: bool,
        expr: PathExpr<'db>,
        place: &mut PathPlaceBuilder<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let Type::Array(arr) = self else {
            if report_errors {
                ctx.errors.push(
                    ResolveError::IndexNonArrayTypePathExpr {
                        expr,
                        ty: place.current_typ,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }
            return;
        };

        // Multi-dimensional arrays use comma-separated indices (e.g., arr[i, j]).
        // Each index corresponds to one dimension of the array.
        let index_count = match expr.expr(db) {
            PathExprKind::Index(index_expr) => index_expr.index.len(),
            _ => 1,
        };

        for i in 0..index_count {
            let curr_dimension = ctx
                .adjustments_of_path_expr(place.current_path)
                .map(|adjs| adjs.array_dimensions(self))
                .unwrap_or(0);

            let dimensions = arr.subranges(db).len() - 1;
            let array_type = match curr_dimension.cmp(&dimensions) {
                Ordering::Less if arr.subranges(db).len() > 1 => *self,
                Ordering::Less | Ordering::Equal => arr.of_type(db).infer(db),
                Ordering::Greater => {
                    if report_errors {
                        ctx.errors.push(
                            ResolveError::IndexNonArrayTypePathExpr {
                                expr,
                                ty: place.current_typ,
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }
                    return;
                }
            };

            ctx.path_expr_adjustments
                .entry(place.current_path)
                .or_default()
                .push(Adjustment::new_index(db, array_type));
        }

        // Use the final adjustment target for the index expression.
        let final_type = ctx
            .adjustments_of_path_expr(place.current_path)
            .and_then(|adjs| adjs.last().map(|a| a.target))
            .unwrap_or(*self);

        ctx.type_of_path_expr.insert(expr, place.current_typ);
        ctx.path_expr_adjustments
            .entry(expr)
            .or_default()
            .push(Adjustment::new_index(db, final_type));
    }
}

impl<'db> Type<'db> {
    pub fn walk_init_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        step: &'db InitExprWalkStep<'db>,
        place: &mut InitPlaceBuilder<'db>,
        ctx: &mut InitExprInferenceResult<'db>,
    ) {
        // Peel through Variable / DataType / StructElement wrappers first.
        if let Some((inner, _)) = self.peel_to_spec(db) {
            return inner.walk_init_expr(db, step, place, ctx);
        }

        let expr = step.get_expr();
        match step {
            InitExprWalkStep::ArrayInit { .. } => {
                if matches!(self, Type::Array(_)) {
                    ctx.type_of_init_expr.insert(*expr, place.current_init_typ);
                } else {
                    ctx.errors.push(
                        ResolveError::IndexNonArrayTypeInitExpr {
                            expr: *expr,
                            ty: place.current_init_typ,
                        }
                        .to_diagnostic(db, ctx.scope.file(db)),
                    );
                }
            }

            InitExprWalkStep::FieldInit { .. } => match self {
                Type::Class(_) | Type::FunctionBlock(_) | Type::Interface(_) => {
                    place.current_init_typ = *self;
                    ctx.type_of_init_expr.insert(*expr, place.current_init_typ);
                }
                Type::Struct(_) => {
                    ctx.type_of_init_expr.insert(*expr, place.current_init_typ);
                }
                _ => {
                    ctx.errors.push(
                        ResolveError::NoFieldOnElementaryType {
                            expr: *expr,
                            ty: *self,
                        }
                        .to_diagnostic(db, ctx.scope.file(db)),
                    );
                }
            },

            InitExprWalkStep::Field { name, .. } => {
                self.walk_init_field(db, *expr, name, place, ctx);
            }

            InitExprWalkStep::ConstantExpr { .. } | InitExprWalkStep::SizedIndex { .. } => {
                /* handled by the inference layer */
            }
        }
    }

    fn walk_init_field(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: InitExpr<'db>,
        name: &SpanIdent<'db>,
        place: &mut InitPlaceBuilder<'db>,
        ctx: &mut InitExprInferenceResult<'db>,
    ) {
        match self.resolve_field(db, &name.ident) {
            FieldLookup::StructElement(field) => {
                place.current_init_typ = Type::StructElement(field);
                ctx.type_of_init_expr.insert(expr, place.current_init_typ);
            }
            FieldLookup::Variable(var) => {
                place.current_init_typ = Type::new_var(db, var);
                ctx.type_of_init_expr.insert(expr, place.current_init_typ);
            }
            FieldLookup::Method(_) | FieldLookup::NotFound => {
                ctx.errors.push(
                    ResolveError::NoSuchFieldInitExpr {
                        expr,
                        ident: **name,
                        ty: place.current_init_typ,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }
        }
    }
}

fn iter_deref_types<'db>(
    db: &'db dyn WorkspaceDataBase,
    mut ty: Type<'db>,
) -> impl Iterator<Item = Result<Type<'db>, Type<'db>>> {
    std::iter::from_fn(move || match ty {
        Type::RefTo(inner) => {
            let next = inner.infer(db);
            ty = next;
            Some(Ok(next))
        }
        non_ref => {
            ty = non_ref;
            Some(Err(non_ref))
        }
    })
}
