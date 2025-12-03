use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{body_inference::BodyInferenceError, init_inference::InitInferenceError},
    hir_def::expressions::expression::{
        BeginPathExpr, InitExpr, PathExpr, VariableAccess, VariableAccessKind,
    },
    hir_ty::{
        body_inference::{Adjustment, BodyInferenceResult}, expr_store::{InitExprWalkStep, PathExprWalkStep}, infer::ctx::InferCtx, init_inference::InitExprInferenceResult, name_res::resolve_namespace_access, ty::Type
    },
};

#[derive(Debug, Copy, Clone)]
pub struct Resolver<'db> {
    pub walkable_typ: Option<Type<'db>>,
}

impl<'db> Resolver<'db> {
    pub fn new(walkable_typ: Option<Type<'db>>) -> Self {
        Self { walkable_typ }
    }

    fn resolve_as_fq(
        &self,
        db: &'db dyn BaseDatabase,
        path_expr: PathExpr<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        let Some((access, _)) = path_expr.to_namespace_access(db) else {
            infer_results
                .errors
                .push(BodyInferenceError::NoItemInScope {
                    expr: path_expr,
                    scope: path_expr.scope_id(db),
                });
            return Type::Never;
        };

        match resolve_namespace_access(db, &access) {
            Some(pou) => {
                let typ = Type::new_pou(db, pou);
                infer_results.type_of_path_expr.insert(path_expr, typ);
                typ
            },
            None => {
                infer_results
                    .errors
                    .push(BodyInferenceError::NoItemInScope {
                        expr: path_expr,
                        scope: path_expr.scope_id(db),
                    });
                Type::Never
            }
        }
    }

    #[must_use]
    pub fn resolve_variable_access(
        &self,
        db: &'db dyn BaseDatabase,
        var_access: VariableAccess<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match var_access.kind(db) {
            VariableAccessKind::Direct { .. } => todo!(),
            VariableAccessKind::Symbolic(s) => match self.walkable_typ {
                Some(typ) => typ.walk_begin_path_expr(db, s, infer_results),
                None => Type::Never,
            },
        }
    }

    #[must_use]
    pub fn resolve_begin_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        path_expr: BeginPathExpr<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match self.walkable_typ {
            Some(typ) => typ.walk_begin_path_expr(db, path_expr, infer_results),
            None => Type::Never,
        }
    }

    #[must_use]
    pub fn resolve_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        path_expr: PathExpr<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        match self.walkable_typ {
            Some(start) => self.resolve_path_steps(start, db, path_expr, infer_results),
            None => self.resolve_as_fq(db, path_expr, infer_results),
        }
    }

    #[must_use]
    fn resolve_path_steps(
        &self,
        mut current: Type<'db>,
        db: &'db dyn BaseDatabase,
        path_expr: PathExpr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        let steps = path_expr.flatten(db);
        for (index, step) in steps.into_iter().enumerate() {
            match current.walk_path_expr(db, index != 0, step, ctx) {
                Type::Never => {
                    // no path was resolved yet
                    if index == 0 {
                        return self.resolve_as_fq(db, path_expr, ctx);
                    }
                    return Type::Never;
                }
                next => current = next,
            }
        }
        current
    }
}

impl<'db> Type<'db> {
    #[must_use]
    fn walk_begin_path_expr(
        &self,
        db: &'db dyn BaseDatabase,
        expr: BeginPathExpr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        // resolve invocation if present
        let current = if let Some(invocation) = expr.invocation(db) {
            InferCtx::resolve_invocation(db, ctx.scope, invocation, ctx);
            ctx.type_of_invocation
                .get(&invocation)
                .copied()
                .unwrap_or_default()
        } else {
            *self
        };

        if let Some(path) = expr.expr(db) {
            // resolve path steps
            let _ = Resolver { walkable_typ: None }.resolve_path_steps(current, db, path, ctx);

            return ctx
                .type_of_path_expr_with_adjustments(path)
                .unwrap_or_default();
        }

        Type::Never
    }

    #[must_use]
    fn walk_path_expr(
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
                        ctx.errors.push(BodyInferenceError::DerefNonRefType {
                            expr: *expr,
                            ty: *self,
                        });
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
                            ctx.errors.push(BodyInferenceError::NoSuchField {
                                expr: *expr,
                                ident: **ident,
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
                        }
                        // Methods
                        else if let Some(m) = def_map.declared_methods.get(&ident.ident) {
                            result_ty = Type::MethodDecl(*m);
                        } else if report_errors {
                            ctx.errors.push(BodyInferenceError::NoSuchField {
                                expr: *expr,
                                ident: **ident,
                                ty: *self,
                            });
                        }
                    }

                    _ => {
                        if report_errors {
                            ctx.errors.push(BodyInferenceError::NoSuchField {
                                expr: *expr,
                                ident: **ident,
                                ty: *self,
                            });
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
                        ctx.errors.push(BodyInferenceError::IndexNonArrayType {
                            expr: *expr,
                            ty: *self,
                        });
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
