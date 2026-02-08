use std::cmp::Ordering;

use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, InitExpr, MultibitsPart, PathExpr},
            invocation::{Invocation, InvocationKind}, spec::StructElement,
        }, interned::identifier::{Ident, SpanIdent}, pous::{pou::Pou, variable::VariableDecl}, scope::ScopeId
    },
    hir_ty::{
        body::{Adjustment, AdjustmentInfo, BodyInferenceResult},
        expr_store::{InitExprWalkStep, PathExprWalkStep},
        head::{inheritance::{MethodRef, inherited_methods}, init_inference::InitExprInferenceResult},
        infer::Infer,
        resolver::{Resolver, invocation::resolve_invocation, visibility::check_visibility},
        ty::Type,
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
enum FieldLookup<'db> {
    StructElement(StructElement<'db>),
    Variable(VariableDecl<'db>),
    Method(MethodRef<'db>),
    NotFound,
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

    /// If this type is a POU (Function / FunctionBlock / Class / Program),
    /// return its scope id.
    fn as_pou_scope(&self, db: &'db dyn WorkspaceDataBase) -> Option<ScopeId<'db>> {
        match self {
            Type::Function(f) => Some(f.scope_id(db)),
            Type::FunctionBlock(fb) => Some(fb.scope_id(db)),
            Type::Class(c) => Some(c.scope_id(db)),
            Type::Program(p) => Some(p.scope_id(db)),
            _ => None,
        }
    }

    /// Resolve a named field on this (concrete) type.
    fn resolve_field(
        &self,
        db: &'db dyn WorkspaceDataBase,
        name: &Ident,
    ) -> FieldLookup<'db> {
        match self {
            Type::Struct(st) => match st.struct_elements(db).get(name) {
                Some(field) => FieldLookup::StructElement(*field),
                None => FieldLookup::NotFound,
            },
            _ => {
                if let Some(scope) = self.as_pou_scope(db) {
                    let def_map = scope.def_map(db);
                    if let Some(var) = def_map.global_variables.get(name) {
                        FieldLookup::Variable(*var)
                    } else if let Some(m) = def_map.declared_methods.get(name) {
                        FieldLookup::Method(*m)
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

        if let Type::MethodDecl(m) = current {
            check_visibility(db, &invocation.as_call_site(db), m, &mut ctx.errors);
            ctx.type_of_path_expr.insert(path_expr, Type::MethodDecl(m));
        } else {
            ctx.errors.push(
                ResolveError::NoSuchFieldPathExpr {
                    expr: path_expr,
                    ident: *path_expr.ident(db),
                    ty: place.current_typ,
                }
                .to_diagnostic(db),
            );
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
            if let Some(method) = inherited.methods.get(&ident.ident) {
                check_visibility(db, &ident.as_call_site(db), method.method, &mut ctx.errors);
                ctx.type_of_path_expr
                    .insert(*expr, Type::MethodDecl(method.method));
            } else {
                ctx.errors.push(
                    ResolveError::NoSuchFieldPathExpr {
                        expr: path_expr,
                        ident: **ident,
                        ty: current,
                    }
                    .to_diagnostic(db),
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
                self.walk_field(db, report_errors, step.get_expr(db), ident, multibits, place, ctx);
            }
            PathExprWalkStep::Deref { count, .. } => {
                self.walk_deref(db, report_errors, step.get_expr(db), *count, place, ctx);
            }
            PathExprWalkStep::Index { .. } => {
                self.walk_index(db, report_errors, step.get_expr(db), place, ctx);
            }
        }
    }

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
                let ty = Type::new_var_with_multibits(db, var, multibits);
                ctx.type_of_path_expr.insert(expr, ty);
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
                if report_errors {
                    ctx.errors.push(
                        ResolveError::NoSuchFieldPathExpr {
                            expr,
                            ident: **ident,
                            ty: place.current_typ,
                        }
                        .to_diagnostic(db),
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
                                .to_diagnostic(db),
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
                    .to_diagnostic(db),
                );
            }
            return;
        };

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
                        .to_diagnostic(db),
                    );
                }
                return;
            }
        };

        ctx.path_expr_adjustments
            .entry(place.current_path)
            .or_default()
            .push(Adjustment::new_index(db, array_type));

        ctx.type_of_path_expr.insert(expr, place.current_typ);
        ctx.path_expr_adjustments
            .entry(expr)
            .or_default()
            .push(Adjustment::new_index(db, array_type));
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
                        .to_diagnostic(db),
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
                        .to_diagnostic(db),
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
                    .to_diagnostic(db),
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
