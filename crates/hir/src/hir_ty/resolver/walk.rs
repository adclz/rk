use std::cmp::Ordering;

use db::WorkspaceDataBase;

use crate::{
    HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::expressions::{
        expression::{BeginPathExpr, MultibitsPart, PathExpr},
        invocation::InvocationKind,
    },
    hir_ty::{
        body::{Adjustment, AdjustmentInfo, BodyInferenceResult},
        expr_store::{InitExprWalkStep, PathExprWalkStep},
        resolver::{Resolver, invocation::resolve_invocation, visibility::check_visibility},
        signature::inheritance::inherited_methods,
        signature::init_inference::InitExprInferenceResult,
        ty::Type,
    },
};

/// When waking a path expression, we need to keep track of the parent type
///
/// The parent type is either a variable or a data type
///
/// This is only useful to create accurate diagnostics
#[derive(Debug, Copy, Clone)]
pub struct PathPlaceBuilder<'db> {
    pub current_typ: Type<'db>,
    pub current_path: PathExpr<'db>,
}

/// When waking a init expression, we need to keep track of the parent type
///
/// The parent type is either a variable or a data type
///
/// This is only useful to create accurate diagnostics
#[derive(Debug, Copy, Clone)]
pub struct InitPlaceBuilder<'db> {
    pub current_init_typ: Type<'db>,
}

impl<'db> Type<'db> {
    pub fn walk_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: BeginPathExpr<'db>,
        multibits: Option<MultibitsPart>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        // a begin path expr can either have:
        // - an invocation
        // - a path expression
        // - an invocation and a path expression

        // resolve invocation if present
        if let Some(invocation) = expr.invocation(db) {
            let pou = match resolve_invocation(db, ctx.scope, invocation, ctx) {
                Some(pou) => pou,
                None => return,
            };

            // walk invocation w/ path expr and kind
            let inherited = inherited_methods(db, pou);

            let mut current = Type::new_pou(db, pou);
            if let Some(path_expr) = expr.expr(db) {
                /*
                CLASS:

                7Access reference
                9a THIS: Reference to own methods
                9b SUPER: Access reference to method in base class

                FUNCTION BLOCKS:

                Access reference
                10a THIS:  Reference to own methods
                10b SUPER:  Access reference to method in base function block
                10c SUPER():  Access reference to body in base function block
                */
                match invocation.kind(db) {
                    InvocationKind::This => {
                        let steps = path_expr.flatten(db);
                        let mut place = PathPlaceBuilder {
                            current_typ: current,
                            current_path: path_expr,
                        };
                        for step in steps {
                            current.walk_path_expr(db, true, step, multibits, &mut place, ctx);
                            // it is necessary to apply adjustments at each step
                            current = ctx
                                .type_of_path_expr_with_adjustments(*step.get_expr())
                                .unwrap_or_default();
                        }

                        if current.is_never() {
                            return;
                        }

                        if let Type::MethodDecl(m) = current {
                            check_visibility(db, &invocation.as_call_site(db), m, &mut ctx.errors);
                            ctx.type_of_path_expr.insert(path_expr, Type::MethodDecl(m));
                            return;
                        } else {
                            ctx.errors.push(
                                ResolveError::NoSuchFieldPathExpr {
                                    expr: path_expr,
                                    ident: *path_expr.ident(db),
                                    ty: place.current_typ,
                                }
                                .to_diagnostic(db),
                            );
                            return;
                        }
                    }
                    InvocationKind::Super => {
                        let steps = path_expr.flatten(db);
                        let first = steps.first();
                        if let Some(PathExprWalkStep::Field { ident, expr }) = first {
                            // check if it's an inherited method
                            if let Some(method) = inherited.methods.get(&ident.ident) {
                                check_visibility(
                                    db,
                                    &ident.as_call_site(db),
                                    method.method,
                                    &mut ctx.errors,
                                );
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
                                return;
                            }
                        }
                    }
                    InvocationKind::SuperBody => {}
                }
            }
        }

        if let Some(path) = expr.expr(db) {
            // resolve path steps
            Resolver::for_scope(db, path.scope_id(db))
                .resolve_path_steps(*self, db, path, multibits, ctx)
        }
    }

    pub fn walk_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        report_errors: bool,
        step: &'db PathExprWalkStep<'db>,
        multibits: Option<MultibitsPart>,
        place: &mut PathPlaceBuilder<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let expr = step.get_expr();

        match self {
            // both variables and data types can have fields
            // but we need to inspect their spec type
            Type::Variable((v, multibits)) => {
                return Type::new_spec(db, v.spec(db)).walk_path_expr(
                    db,
                    report_errors,
                    step,
                    *multibits,
                    place,
                    ctx,
                );
            }
            Type::DataType(typ) => {
                return Type::new_spec(db, typ.spec(db)).walk_path_expr(
                    db,
                    report_errors,
                    step,
                    multibits,
                    place,
                    ctx,
                );
            }
            Type::StructElement(st) => {
                return Type::new_spec(db, st.spec(db)).walk_path_expr(
                    db,
                    report_errors,
                    step,
                    multibits,
                    place,
                    ctx,
                );
            }
            _ => (),
        }
        match step {
            PathExprWalkStep::Field { ident, expr: _ } => {
                match self {
                    Type::Struct(st) => {
                        if let Some(field) = st.struct_elements(db).get(&ident.ident) {
                            ctx.type_of_path_expr
                                .insert(*expr, Type::StructElement(*field));
                            place.current_typ = Type::StructElement(*field);
                            place.current_path = *step.get_expr();
                        } else if report_errors {
                            ctx.errors.push(
                                ResolveError::NoSuchFieldPathExpr {
                                    expr: *expr,
                                    ident: **ident,
                                    ty: place.current_typ,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }

                    Type::Function(_)
                    | Type::FunctionBlock(_)
                    | Type::Class(_)
                    | Type::Program(_) => {
                        let def_map = match self {
                            Type::Function(f) => f.scope_id(db),
                            Type::FunctionBlock(fb) => fb.scope_id(db),
                            Type::Class(c) => c.scope_id(db),
                            Type::Program(p) => p.scope_id(db),
                            // unreachable due to the match above
                            _ => unreachable!(),
                        }
                        .def_map(db);

                        // Variables
                        if let Some(var) = def_map.global_variables.get(&ident.ident) {
                            ctx.type_of_path_expr
                                .insert(*expr, Type::new_var_with_multibits(db, *var, multibits));
                            place.current_typ = Type::new_var_with_multibits(db, *var, multibits);
                            place.current_path = *step.get_expr();
                        }
                        // Methods
                        else if let Some(m) = def_map.declared_methods.get(&ident.ident) {
                            ctx.type_of_path_expr.insert(*expr, Type::MethodDecl(*m));
                            place.current_typ = Type::MethodDecl(*m);
                            place.current_path = *step.get_expr();
                            check_visibility(db, &ident.as_call_site(db), *m, &mut ctx.errors);
                        } else if report_errors {
                            ctx.errors.push(
                                ResolveError::NoSuchFieldPathExpr {
                                    expr: *expr,
                                    ident: **ident,
                                    ty: place.current_typ,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }

                    _ => {
                        if report_errors {
                            ctx.errors.push(
                                ResolveError::NoSuchFieldPathExpr {
                                    expr: *expr,
                                    ident: **ident,
                                    ty: place.current_typ,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }
                }
            }
            PathExprWalkStep::Deref { expr: _, count } => {
                for result in iter_deref_types(db, *self).take((*count) as usize) {
                    match result {
                        Ok(ty) => {
                            ctx.path_expr_adjustments
                                .entry(place.current_path)
                                .or_default()
                                .push(Adjustment::new_deref(db, ty));

                            ctx.type_of_path_expr.insert(*expr, place.current_typ);
                            ctx.path_expr_adjustments
                                .entry(*expr)
                                .or_default()
                                .push(Adjustment::new_deref(db, ty));
                        }
                        Err(non_ref) => {
                            if report_errors {
                                ctx.errors.push(
                                    ResolveError::DerefNonRefType {
                                        expr: *expr,
                                        ty: non_ref,
                                    }
                                    .to_diagnostic(db),
                                );
                            }
                            break;
                        }
                    }
                }
            }
            PathExprWalkStep::Index { expr: _ } => match self {
                Type::Array(arr) => {
                    let curr_dimension = ctx
                        .adjustments_of_path_expr(place.current_path)
                        .map(|adjs| adjs.array_dimensions(self))
                        .unwrap_or(0);

                    let dimensions = arr.subranges(db).len() - 1;
                    let array_type = match curr_dimension.cmp(&dimensions) {
                        Ordering::Less if arr.subranges(db).len() > 1 => *self,
                        Ordering::Less | Ordering::Equal => Type::new_spec(db, arr.of_type(db)),
                        Ordering::Greater => {
                            if report_errors {
                                ctx.errors.push(
                                    ResolveError::IndexNonArrayTypePathExpr {
                                        expr: *expr,
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

                    ctx.type_of_path_expr.insert(*expr, place.current_typ);
                    ctx.path_expr_adjustments
                        .entry(*expr)
                        .or_default()
                        .push(Adjustment::new_index(db, array_type));
                }
                _ => {
                    if report_errors {
                        ctx.errors.push(
                            ResolveError::IndexNonArrayTypePathExpr {
                                expr: *expr,
                                // an array will always be declared by a DataType or a Variable
                                ty: place.current_typ,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
            },
        }
    }

    pub fn walk_init_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        step: &'db InitExprWalkStep<'db>,
        place: &mut InitPlaceBuilder<'db>,
        ctx: &mut InitExprInferenceResult<'db>,
    ) {
        match self {
            Type::DataType(dt) => {
                return Type::new_spec(db, dt.spec(db)).walk_init_expr(db, step, place, ctx);
            }
            Type::Variable((dt, mul)) => {
                return Type::new_spec(db, dt.spec(db)).walk_init_expr(db, step, place, ctx);
            }
            Type::StructElement(elem) => {
                return Type::new_spec(db, elem.spec(db)).walk_init_expr(db, step, place, ctx);
            }
            _ => (),
        }
        let expr = step.get_expr();
        match step {
            InitExprWalkStep::ArrayInit { .. } => match self {
                Type::Array(array) => {
                    ctx.type_of_init_expr
                        .insert(*expr, Type::new_spec(db, array.of_type(db)));
                }
                _ => {
                    ctx.errors.push(
                        ResolveError::IndexNonArrayTypeInitExpr {
                            expr: *expr,
                            ty: place.current_init_typ,
                        }
                        .to_diagnostic(db),
                    );
                }
            },

            InitExprWalkStep::FieldInit { .. } => match self {
                Type::Class(_) | Type::FunctionBlock(_) | Type::Interface(_) => {
                    place.current_init_typ = *self;
                    ctx.type_of_init_expr.insert(*expr, place.current_init_typ);
                }
                Type::Struct(_) => {
                    ctx.type_of_init_expr.insert(*expr, *self);
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
                match self {
                    Type::Struct(st) => {
                        if let Some(field) = st.struct_elements(db).get(&name.ident) {
                            place.current_init_typ = Type::StructElement(*field);
                            ctx.type_of_init_expr.insert(*expr, place.current_init_typ);
                        } else {
                            ctx.errors.push(
                                ResolveError::NoSuchFieldInitExpr {
                                    expr: *expr,
                                    ident: **name,
                                    ty: place.current_init_typ,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }

                    Type::Function(_) | Type::FunctionBlock(_) | Type::Class(_) => {
                        let def_map = match self {
                            Type::Function(f) => f.scope_id(db),
                            Type::FunctionBlock(fb) => fb.scope_id(db),
                            Type::Class(c) => c.scope_id(db),
                            _ => unreachable!(),
                        }
                        .def_map(db);

                        // Variables
                        if let Some(var) = def_map.global_variables.get(&name.ident) {
                            place.current_init_typ = Type::new_var(db, *var);
                            ctx.type_of_init_expr.insert(*expr, place.current_init_typ);
                        } else {
                            ctx.errors.push(
                                ResolveError::NoSuchFieldInitExpr {
                                    expr: *expr,
                                    ident: **name,
                                    ty: place.current_init_typ,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }
                    _ => {
                        ctx.errors.push(
                            ResolveError::NoSuchFieldInitExpr {
                                expr: *expr,
                                ident: **name,
                                ty: place.current_init_typ,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
            }
            InitExprWalkStep::ConstantExpr { .. } | InitExprWalkStep::SizedIndex { .. } => { /*  handled by the inference layer */ }
        }
    }
}

fn iter_deref_types<'db>(
    db: &'db dyn WorkspaceDataBase,
    mut ty: Type<'db>,
) -> impl Iterator<Item = Result<Type<'db>, Type<'db>>> {
    std::iter::from_fn(move || match ty {
        Type::RefTo(inner) => {
            let next = Type::new_spec(db, inner);
            ty = next;
            Some(Ok(next))
        }
        non_ref => {
            ty = non_ref;
            Some(Err(non_ref))
        }
    })
}
