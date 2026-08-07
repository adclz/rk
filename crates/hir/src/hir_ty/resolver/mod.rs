use db::WorkspaceDataBase;

pub mod func_call;
pub mod invocation;
pub mod name;
pub mod visibility;
pub mod walk;

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e2_resolve::ResolveError, e6_array::ArrayError},
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, MultibitsPart, PathExpr, PathExprKind, VariableAccess,
                VariableAccessKind,
            },
            spec::ElementarySpec,
        },
        interned::namespace::NamespaceAccess,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::BodyInferenceResult,
        expr_store::PathExprWalkStep,
        index_graphs::namespace_index,
        infer::{expr::InferExprCtx, table::InferenceTable},
        resolver::{visibility::check_test_visibility, walk::PathPlaceBuilder},
        ty::Type,
    },
};

#[derive(Debug, Copy, Clone)]
pub struct Resolver<'db> {
    pub root: PathResolutionRoot<'db>,
}

#[derive(Debug, Copy, Clone)]
pub enum PathResolutionRoot<'db> {
    /// Path starts from a known value/type (THIS, SUPER, implicit self)
    Value { base: Type<'db> },

    /// Path starts from a namespace / scope
    Namespace { scope: ScopeId<'db> },
}

impl<'db> Resolver<'db> {
    pub fn for_scope(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> Self {
        let root = match get_scope(db, scope).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::DataType(_) => PathResolutionRoot::Namespace { scope },
                _ => PathResolutionRoot::Value {
                    base: Type::new_pou(db, pou),
                },
            },
            ScopeKind::MethodDecl(m) => PathResolutionRoot::Value {
                base: Type::MethodDecl(m.into()),
            },
            ScopeKind::Program(program) => PathResolutionRoot::Value {
                base: Type::Program(program),
            },
            _ => PathResolutionRoot::Namespace { scope },
        };

        Self { root }
    }

    /// Try to resolve `path_expr` as a fully-qualified namespace access.
    ///
    /// Returns `true` if the path was successfully resolved, `false` otherwise
    /// (an error diagnostic is always pushed on failure).
    fn try_resolve_as_fq(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> bool {
        let Some((access, _)) = path_expr.to_namespace_access(db) else {
            ctx.errors.push(
                ResolveError::NoItemInScope {
                    expr: path_expr,
                    scope: path_expr.scope_id(db),
                }
                .to_diagnostic(db, ctx.scope.file(db)),
            );
            return false;
        };

        match name::resolve_name(db, access, path_expr.get_scope_id(db)) {
            name::NameResolution::MethodSelf(method) => {
                ctx.type_of_path_expr
                    .insert(path_expr, Type::MethodDecl(method.into()));
                true
            }
            name::NameResolution::Pou(pou, using) => {
                if let Some(using) = using {
                    ctx.usings_used.insert(using);
                }
                let call_site = CallSite::new(path_expr.scope_id(db), path_expr.get_id(db));
                check_test_visibility(db, &call_site, pou.get_scope_id(db), &mut ctx.errors);
                ctx.type_of_path_expr
                    .insert(path_expr, Type::new_pou(db, pou));
                true
            }
            name::NameResolution::Program(prog) => {
                let call_site = CallSite::new(path_expr.scope_id(db), path_expr.get_id(db));
                check_test_visibility(db, &call_site, prog.scope_id(db), &mut ctx.errors);
                ctx.type_of_path_expr.insert(path_expr, Type::Program(prog));
                true
            }
            name::NameResolution::Ambiguous(candidates) => {
                ctx.errors.push(
                    ResolveError::MultipleItemsInScope {
                        name: path_expr.ident(db).ident,
                        span: path_expr.get_span(db),
                        candidates,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
                false
            }
            name::NameResolution::NotFound => {
                // When a namespaced path like `MY_TYPE.field` fails FQ resolution,
                // check whether the namespace prefix itself is invalid. If the prefix
                // is not a real namespace (e.g. it's a TYPE name), point the error at
                // the first step rather than the last — the root cause is the prefix.
                let error_expr = if let Some(ns_path) = &access.namespace
                    && namespace_index(db, **ns_path).is_empty()
                {
                    path_expr
                        .flatten(db)
                        .first()
                        .map(|step| step.get_expr(db))
                        .unwrap_or(path_expr)
                } else {
                    path_expr
                };
                ctx.errors.push(
                    ResolveError::NoItemInScope {
                        expr: error_expr,
                        scope: path_expr.scope_id(db),
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
                false
            }
        }
    }

    pub fn resolve_variable_access(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match var_access.kind(db) {
            VariableAccessKind::Direct(dv) => {
                ctx.type_of_direct_variable
                    .insert(dv, Type::DirectVariable((dv, var_access.multibits(db))));
            }
            VariableAccessKind::Symbolic(s) => {
                self.resolve_begin_path_expr(db, s, var_access.multibits(db), ctx);
            }
        }
    }

    pub fn resolve_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: BeginPathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match self.root {
            PathResolutionRoot::Value { base } => {
                base.walk_begin_path_expr(db, path_expr, multibits, ctx);
                if let Some(path) = path_expr.expr(db) {
                    self.resolve_index_subscripts(db, path, ctx);
                }
            }
            PathResolutionRoot::Namespace { .. } => {
                // a begin path expr will always refer to a local variable in this context
            }
        };
    }

    pub fn resolve_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match self.root {
            PathResolutionRoot::Value { base } => {
                self.resolve_path_steps(base, db, path_expr, multibits, ctx);
                self.resolve_index_subscripts(db, path_expr, ctx);
            }
            PathResolutionRoot::Namespace { .. } => {
                self.try_resolve_as_fq(db, path_expr, ctx);
            }
        }
    }

    /// Resolve the subscript expressions of every Index step in a path —
    /// AFTER the step walk, following the walk-then-resolve split: the walk
    /// (`walk.rs`) types the path's STEPS, and never descends into the
    /// expressions inside them.
    ///
    /// Subscripts are ordinary expressions in their own right (`a[n + 1]`,
    /// `a[idx()]`), and this is the only place body inference ever sees
    /// them. Left untyped, MIR has no width for the index computation and
    /// refuses to lower anything but a bare variable or literal. They
    /// resolve from the SCOPE, not from the walked path: the `n` of `a[n]`
    /// is a local lookup, not a field of `a`.
    fn resolve_index_subscripts(
        &self,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        for step in path_expr.flatten(db) {
            let PathExprWalkStep::Index { expr } = step else {
                continue;
            };
            let PathExprKind::Index(index_expr) = expr.expr(db) else {
                continue;
            };
            for sub in index_expr.index.iter() {
                // A path expression can be resolved through more than one
                // entry; the first pass already did the work.
                if ctx.type_of_expr.contains_key(sub) {
                    continue;
                }
                let mut infer = InferExprCtx::new(*self);
                infer.resolve_expr(db, *sub, ctx);
                infer.check_expr(db, *sub, ctx);

                // An untyped literal is pinned to DINT, the width the index
                // computation runs at.
                let raw = ctx.type_of_expr_with_adjustments(db, *sub);
                if raw.has_infer() {
                    let mut table = InferenceTable::new();
                    table.set_target_type(db, None, Type::Elementary(ElementarySpec::DInt));
                    table.add_type(db, *sub, raw, *self);
                    table.resolve_completly(db, *self, ctx);
                }

                // IEC subscripts are ANY_INT: signed or unsigned integers,
                // subranges included (they index like their base). `Never`
                // already carries its own diagnostic.
                let ty = ctx.type_of_expr_with_adjustments(db, *sub).normalize(db);
                if !ty.is_never() && !ty.is_signed_integer() && !ty.is_unsigned_integer() {
                    ctx.errors.push(
                        ArrayError::NonIntegerIndex { expr: *sub, ty }
                            .to_diagnostic(db, ctx.scope.file(db)),
                    );
                }
            }
        }
    }

    /// Walk each step of a path expression against the current type.
    ///
    /// On the **first** step, errors are suppressed because a failed local
    /// lookup may still succeed as a fully-qualified namespace access.
    /// If step 0 fails and the root is a `Value`, we fall back to FQ resolution.
    pub(crate) fn resolve_path_steps(
        &self,
        mut current: Type<'db>,
        db: &'db dyn WorkspaceDataBase,
        path_expr: PathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let steps = path_expr.flatten(db);
        let Some(first_step) = steps.first() else {
            return;
        };

        let mut place = PathPlaceBuilder {
            current_typ: current,
            current_path: first_step.get_expr(db),
        };

        // For single-step paths (e.g. By.7), pass multibit directly so it's
        // applied to the variable. For multi-step paths (e.g. SX[SN].0), defer
        // multibit to after the walk — otherwise it applies to the array variable
        // before indexing, changing its type prematurely.
        let single_step = steps.len() == 1;

        for (index, step) in steps.iter().enumerate() {
            let is_first_step = index == 0;

            // Inside a Function/Method, the callable's OWN name refers to the
            // return value. Detected BEFORE the walk, because the walk does
            // not fail uniformly: a function's name is not a member and falls
            // through to the fallback below, but a METHOD's name resolves via
            // the implicit THIS to the method itself — and then `.x` dies on
            // a MethodDecl ("'GetPt' has no field named 'x'").
            if is_first_step
                && let PathExprWalkStep::Field { ident, .. } = step
                && let PathResolutionRoot::Value { base } = self.root
                && let Some(ret_ty) = base.with_return_type(db)
                && self_reference_name(db, base) == Some(ident.ident)
                // A DECLARED variable of the same name shadows the implicit
                // return-value name (pinned by the missing-return lint's
                // shadowing test), so the name is only taken as the return
                // value when the ordinary walk would not find a variable —
                // or would find the enclosing method ITSELF, which is the
                // method-body case this branch exists for.
                && match current.resolve_field(db, &ident.ident) {
                    walk::FieldLookup::NotFound => true,
                    walk::FieldLookup::Method(m) => {
                        matches!(base, Type::MethodDecl(m2) if m == m2)
                    }
                    _ => false,
                }
            {
                if single_step {
                    // The whole path IS the return value, typed as the
                    // callable so assignment to it hits the return-slot
                    // handling.
                    ctx.type_of_path_expr.insert(step.get_expr(db), base);
                    ctx.type_of_path_expr.insert(path_expr, base);
                    return;
                }
                // Multi-step: the root is the return VALUE and the next steps
                // walk its fields, so the root is recorded as the return type.
                let normalized = ret_ty.normalize(db);
                ctx.type_of_path_expr.insert(step.get_expr(db), normalized);
                current = normalized;
                place.current_typ = normalized;
                place.current_path = step.get_expr(db);
                continue;
            }

            let step_multibits = if single_step { multibits } else { None };

            // Suppress errors on the first step: if it fails we may fall back to FQ resolution.
            current.walk_path_expr(db, !is_first_step, step, step_multibits, &mut place, ctx);

            // Check whether walk_path_expr actually resolved this step.
            let resolved = ctx.type_of_path_expr.contains_key(&step.get_expr(db));

            if !resolved {
                if is_first_step && matches!(self.root, PathResolutionRoot::Value { .. }) {
                    // Try full FQ resolution (for namespace-qualified paths).
                    if self.try_resolve_as_fq(db, path_expr, ctx) {
                        return;
                    }

                    // Direct access to a configuration VAR_GLOBAL.
                    // A declared VAR_EXTERNAL resolves above (it's a POU variable),
                    // so reaching here means the global was NOT imported via
                    // VAR_EXTERNAL. We resolve it anyway and record it so the linter
                    // can warn — strict IEC wants an explicit VAR_EXTERNAL.
                    if let PathExprWalkStep::Field { ident, .. } = step
                        && let Some(global) =
                            crate::hir_ty::index_graphs::external_var_lookup(db, ident.ident)
                    {
                        // try_resolve_as_fq pushed a "not found" error; this access is
                        // actually valid, so drop it.
                        ctx.errors.pop();
                        let ty = Type::new_var_with_multibits(db, global, step_multibits);
                        ctx.type_of_path_expr.insert(step.get_expr(db), ty);
                        ctx.variables_used.insert(global);
                        ctx.globals_without_external
                            .push((step.get_expr(db), global));
                        if single_step {
                            ctx.type_of_path_expr.insert(path_expr, ty);
                            return;
                        } else {
                            current = ty;
                            place.current_typ = ty;
                            place.current_path = step.get_expr(db);
                            continue;
                        }
                    }

                    // FQ failed. For multi-step paths like TYPE_NAME.field, the first
                    // step may be a DataType used as a constant. Try resolving just the
                    // first step as a POU name and continue walking the remaining steps.
                    // Only DataTypes are allowed here — Functions/FBs are not valid
                    // constant-access targets.
                    if steps.len() > 1
                        && let PathExprWalkStep::Field { ident, .. } = step
                    {
                        let access = NamespaceAccess::new(db, None, *ident);
                        if let name::NameResolution::Pou(pou @ Pou::DataType(_), using) =
                            name::resolve_name(db, &access, path_expr.get_scope_id(db))
                        {
                            if let Some(using) = using {
                                ctx.usings_used.insert(using);
                            }
                            // Remove the FQ error — this path is valid so far.
                            ctx.errors.pop();
                            let ty = Type::new_pou(db, pou);
                            ctx.type_of_path_expr.insert(step.get_expr(db), ty);
                            current = ty;
                            place.current_typ = ty;
                            place.current_path = step.get_expr(db);
                            continue;
                        }
                    }
                }
                return;
            }

            // Shadowing detection: on the first step, if a variable was resolved,
            // check if a POU with the same name is also visible in this scope.
            if is_first_step
                && let Some(Type::Variable((var, _))) =
                    ctx.type_of_path_expr.get(&step.get_expr(db))
            {
                let var_name = var.get_name_ident(db);
                if let name::PouResolution::Found(pou, _) =
                    name::pou_names_res(db, var_name, path_expr.scope_id(db))
                {
                    ctx.variables_shadowing.insert(*var, pou);
                }
            }

            current = ctx.type_of_path_expr_with_adjustments(step.get_expr(db));
        }

        // For multi-step paths with multibit access (e.g. SX[SN].0), apply
        // multibit to the final resolved type now that indexing/deref is done.
        if !single_step
            && let Some(mb) = multibits
            && let Some(last_step) = steps.last()
        {
            let last_expr = last_step.get_expr(db);
            let mb_type = crate::hir_ty::infer::normalize::multibits_to_type(db, mb);
            // Update type_of_path_expr and also replace the last adjustment
            // target (e.g. array index target) with the multibit type.
            ctx.type_of_path_expr.insert(last_expr, mb_type);
            if let Some(adjustments) = ctx.path_expr_adjustments.get_mut(&last_expr)
                && let Some(last_adj) = adjustments.last_mut()
            {
                last_adj.target = mb_type;
            }
        }
    }
}

/// The name a callable's return value is addressed by from inside its own
/// body — the callable's own name. `None` for anything that is not a callable
/// with a return value.
fn self_reference_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    base: Type<'db>,
) -> Option<crate::hir_def::interned::identifier::Ident> {
    match base {
        Type::Function(f) => Some(f.get_name_ident(db)),
        Type::MethodDecl(m) => Some(m.get_name_ident(db)),
        _ => None,
    }
}
