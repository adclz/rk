use auto_lsp::default::db::BaseDatabase;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic, body_inference::BodyInferenceError,
        init_inference::InitInferenceError,
    },
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, InitExpr},
            invocation::InvocationKind,
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    hir_ty::{
        body_inference::{Adjustment, BodyInferenceResult},
        expr_store::{InitExprWalkStep, PathExprWalkStep},
        inheritance_solver::inherited_methods,
        init_inference::InitExprInferenceResult,
        resolver::{Resolver, invocation::resolve_invocation, visibility::check_visibility},
        ty::Type,
    },
};

impl<'db> Type<'db> {
    #[must_use]
    pub fn walk_begin_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        expr: BeginPathExpr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        // a begin path expr can either have:
        // - an invocation
        // - a path expression
        // - an invocation and a path expression

        // resolve invocation if present
        if let Some(invocation) = expr.invocation(db) {
            let pou = match resolve_invocation(db, ctx.scope, invocation, ctx) {
                Some(pou) => pou,
                None => return Type::Never,
            };

            // walk invocation w/ path expr and kind
            let inherited = inherited_methods(db, pou);

            let mut current = Type::new_pou(db, pou);
            match expr.expr(db) {
                Some(path_expr) => {
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
                            for step in steps {
                                current = current.walk_path_expr(db, true, step, ctx);
                            }
                            if let Type::MethodDecl(m) = current {
                                check_visibility(
                                    db,
                                    &invocation.as_call_site(db),
                                    m,
                                    &mut ctx.errors,
                                );
                                ctx.type_of_path_expr.insert(path_expr, Type::MethodDecl(m));
                                return current;
                            } else {
                                ctx.errors.push(
                                    BodyInferenceError::NoSuchField {
                                        expr: path_expr,
                                        ident: *path_expr.ident(db),
                                        ty: current,
                                    }
                                    .to_diagnostic(db),
                                );
                                return Type::Never;
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
                                    return Type::MethodDecl(method.method);
                                } else {
                                    ctx.errors.push(
                                        BodyInferenceError::NoSuchField {
                                            expr: path_expr,
                                            ident: **ident,
                                            ty: current,
                                        }
                                        .to_diagnostic(db),
                                    );
                                    return Type::Never;
                                }
                            }
                        }
                        InvocationKind::SuperBody => {}
                    }
                }
                None => return Type::new_pou(db, pou),
            }
        }

        if let Some(path) = expr.expr(db) {
            // resolve path steps
            let _ = Resolver {
                scope: path.scope_id(db),
                walkable_typ: None,
            }
            .resolve_path_steps(*self, db, path, ctx);

            return ctx
                .type_of_path_expr_with_adjustments(path)
                .unwrap_or_default();
        }

        Type::Never
    }

    #[must_use]
    pub fn walk_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        report_errors: bool,
        step: &'db PathExprWalkStep<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        let expr = step.get_expr();
        let mut result_ty = Type::Never;

        match step {
            PathExprWalkStep::Deref {
                expr: _, target: _, ..
            } => match self {
                Type::RefTo(ref_to) => {
                    result_ty = Type::new_spec(db, *ref_to);
                    ctx.path_expr_adjustments
                        .insert(*expr, vec![Adjustment::new_deref(db, result_ty)]);
                }
                _ => {
                    if report_errors {
                        ctx.errors.push(
                            BodyInferenceError::DerefNonRefType {
                                expr: *expr,
                                ty: *self,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
            },

            PathExprWalkStep::Field { ident, expr: _ } => {
                match self {
                    Type::Variable(v) => {
                        return Type::new_spec(db, v.spec(db)).walk_path_expr(
                            db,
                            report_errors,
                            step,
                            ctx,
                        );
                    }
                    Type::DataType(typ) => {
                        return Type::new_spec(db, typ.spec(db)).walk_path_expr(
                            db,
                            report_errors,
                            step,
                            ctx,
                        );
                    }
                    Type::Struct(st) => {
                        if let Some(field) = st.resolve_elements(db).get(&ident.ident) {
                            result_ty = Type::StructElement(*field);
                        } else if report_errors {
                            ctx.errors.push(
                                BodyInferenceError::NoSuchField {
                                    expr: *expr,
                                    ident: **ident,
                                    ty: *self,
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
                        if let Some(var) = def_map.global_variables.get(&ident.ident) {
                            result_ty = Type::new_var(db, *var);
                        }
                        // Methods
                        else if let Some(m) = def_map.declared_methods.get(&ident.ident) {
                            result_ty = Type::MethodDecl(*m);
                            check_visibility(db, &ident.as_call_site(db), *m, &mut ctx.errors);
                        } else if report_errors {
                            ctx.errors.push(
                                BodyInferenceError::NoSuchField {
                                    expr: *expr,
                                    ident: **ident,
                                    ty: *self,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }

                    _ => {
                        if report_errors {
                            ctx.errors.push(
                                BodyInferenceError::NoSuchField {
                                    expr: *expr,
                                    ident: **ident,
                                    ty: *self,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }
                }
            }

            PathExprWalkStep::Index { expr: _ } => match self {
                Type::Array(arr) => {
                    result_ty = Type::new_spec(db, arr.of_type(db));
                    ctx.path_expr_adjustments
                        .insert(*expr, vec![Adjustment::new_index(db, result_ty)]);
                }
                _ => {
                    if report_errors {
                        ctx.errors.push(
                            BodyInferenceError::IndexNonArrayType {
                                expr: *expr,
                                ty: *self,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
            },
        }

        ctx.type_of_path_expr.insert(*expr, result_ty);
        result_ty
    }

    #[must_use]
    pub fn walk_init_expr(
        &self,
        db: &'db dyn BaseDatabase,
        expr: InitExpr<'db>,
        step: InitExprWalkStep<'db>,
        ctx: &mut InitExprInferenceResult<'db>,
    ) -> Type<'db> {
        if let Type::DataType(dt) = self {
            return Type::new_spec(db, dt.spec(db)).walk_init_expr(db, expr, step, ctx);
        }
        if let Type::StructElement(elem) = self {
            return Type::new_spec(db, elem.spec(db)).walk_init_expr(db, expr, step, ctx);
        }
        let mut result_ty = Type::Never;
        match step {
            InitExprWalkStep::Index => {
                match self {
                    Type::Array(array) => {
                        // return array type
                        result_ty = Type::new_spec(db, array.of_type(db));
                    }
                    _ => {
                        ctx.errors.push(InitInferenceError::IndexNonArrayType {
                            expr: expr,
                            ty: *self,
                        });
                    }
                }
            }
            InitExprWalkStep::Access => match self {
                Type::Class(_) | Type::FunctionBlock(_) | Type::Interface(_) | Type::Struct(_) => {
                    result_ty = *self;
                }
                _ => {
                    ctx.errors.push(InitInferenceError::IsElementaryType {
                        expr: expr,
                        ty: *self,
                    });
                }
            },
            InitExprWalkStep::Field(ident) => {
                match self {
                    Type::Struct(st) => {
                        if let Some(field) = st.resolve_elements(db).get(&ident.ident) {
                            result_ty = Type::StructElement(*field);
                        } else {
                            ctx.errors.push(InitInferenceError::NoSuchField {
                                expr: expr,
                                ident: *ident,
                                ty: *self,
                            });
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
                        if let Some(var) = def_map.global_variables.get(&ident.ident) {
                            result_ty = Type::new_var(db, *var);
                        } else {
                            ctx.errors.push(InitInferenceError::NoSuchField {
                                expr: expr,
                                ident: *ident,
                                ty: *self,
                            });
                        }
                    }
                    _ => {
                        ctx.errors.push(InitInferenceError::NoSuchField {
                            expr: expr,
                            ident: *ident,
                            ty: *self,
                        });
                    }
                }
            }
            InitExprWalkStep::NoOp => {
                result_ty = *self;
            }
        }
        ctx.type_of_expr.insert(expr, result_ty);
        result_ty
    }
}
