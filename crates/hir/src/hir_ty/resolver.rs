use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    CallSite, HasVisibility, HirNodeInfo, Visibility,
    check::errors::{
        analysis_error::ToIdeDiagnostic, body_inference::BodyInferenceError,
        init_inference::InitInferenceError, visibility::VisibilityError,
    },
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, InitExpr, PathExpr, VariableAccess, VariableAccessKind},
            invocation::InvocationKind,
        },
        namespace::NamespaceDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        body_inference::{Adjustment, BodyInferenceResult},
        expr_store::{InitExprWalkStep, PathExprWalkStep},
        infer::ctx::InferCtx,
        inheritance_solver::inherited_methods,
        init_inference::InitExprInferenceResult,
        name_res::resolve_namespace_access,
        ty::Type,
    },
};

#[derive(Debug, Copy, Clone)]
pub struct Resolver<'db> {
    pub scope: ScopeId<'db>,
    pub walkable_typ: Option<Type<'db>>,
}

impl<'db> Resolver<'db> {
    pub fn new(scope: ScopeId<'db>, walkable_typ: Option<Type<'db>>) -> Self {
        Self { scope, walkable_typ }
    }

    fn resolve_as_fq(
        &self,
        db: &'db dyn BaseDatabase,
        path_expr: PathExpr<'db>,
        infer_results: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        let Some((access, _)) = path_expr.to_namespace_access(db) else {
            infer_results.errors.push(
                BodyInferenceError::NoItemInScope {
                    expr: path_expr,
                    scope: path_expr.scope_id(db),
                }
                .to_diagnostic(db),
            );
            return Type::Never;
        };

        match resolve_namespace_access(db, &access) {
            Some(pou) => {
                let typ = Type::new_pou(db, pou);
                infer_results.type_of_path_expr.insert(path_expr, typ);
                typ
            }
            None => {
                infer_results.errors.push(
                    BodyInferenceError::NoItemInScope {
                        expr: path_expr,
                        scope: path_expr.scope_id(db),
                    }
                    .to_diagnostic(db),
                );
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
        // a begin path expr can either have:
        // - an invocation
        // - a path expression
        // - an invocation and a path expression

        // resolve invocation if present
        if let Some(invocation) = expr.invocation(db) {
            let pou = match InferCtx::resolve_invocation(db, ctx.scope, invocation, ctx) {
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
            let _ = Resolver { scope: path.scope_id(db), walkable_typ: None }.resolve_path_steps(*self, db, path, ctx);

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

/*
Methods and specifiers

5 METHOD...END_METHOD Method definition
5a PUBLIC specifier Method may be called from anywhere
5b PRIVATE specifier Method may only be called from inside the defining POU
5c INTERNAL specifier Method may only be called from inside the same namespace
5d PROTECTED specifier Method may only be called from inside the defining POU
and its derivations (default)
5e FINAL specifier Method shall not be overridden


Variable access specifiers

11a PUBLIC specifier The variable may be accessed from anywhere.
11b PRIVATE specifier The variable may only be accessed from inside the defining POU.
11c INTERNAL specifier The variable may only be accessed from inside the same
namespace.
11d PROTECTED specifier The variable may only be accessed from inside the defining POU
and its derivations (default).
*/

pub fn check_visibility<'db>(
    db: &'db dyn BaseDatabase,
    call_site: &CallSite<'db>,
    target: impl HasVisibility<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    // Methods use their declaring POU as scope for visibility checks
    let calling_scope_id = call_site.get_scope_id(db);
    let calling_scope = match get_scope(db, calling_scope_id).kind {
        ScopeKind::MethodDecl(m) => get_scope(db, calling_scope_id)
            .parent
            .expect("Method should always have a parent scope"),
        _ => calling_scope_id,
    };
    let target_scope = target.get_scope_id(db);
    let target_visibility = target.get_visibility(db);

    // PUBLIC methods can be called from anywhere
    if target_visibility.contains(Visibility::PUBLIC) {
        return;
    }

    // Check PRIVATE visibility - only callable from the same POU (same scope)
    if target_visibility.contains(Visibility::PRIVATE) {
        if calling_scope != target_scope {
            errors.push(
                VisibilityError::Private {
                    call_site: call_site.clone(),
                    target: target.as_call_site(db),
                }
                .to_diagnostic(db),
            );
        }
        return;
    }

    // Check INTERNAL visibility - only callable from the same namespace
    if target_visibility.contains(Visibility::INTERNAL) {
        let result = is_same_namespace(db, calling_scope, target_scope);
        match result {
            SameNamespaceResult::Same => {} // Ok
            _ => {
                errors.push(
                    VisibilityError::Internal {
                        call_site: call_site.clone(),
                        target: target.as_call_site(db),
                        result,
                    }
                    .to_diagnostic(db),
                );
            }
        }
        return;
    }

    // Check PROTECTED visibility (default) - callable from same POU or derived POUs
    if (target_visibility.contains(Visibility::PROTECTED) || target_visibility.is_empty())
        && !is_derived_pou(db, calling_scope, target_scope)
    {
        errors.push(
            VisibilityError::Protected {
                call_site: call_site.clone(),
                target: target.as_call_site(db),
            }
            .to_diagnostic(db),
        );
    }
}

/// Check if the calling scope is in a POU that derives from the method's POU
fn is_derived_pou<'db>(
    db: &'db dyn BaseDatabase,
    calling_scope: ScopeId<'db>,
    method_scope: ScopeId<'db>,
) -> bool {
    let sema_calling_scope = semantic_index(db, calling_scope.file(db));
    let sema_method_scope = semantic_index(db, method_scope.file(db));

    let sema_calling_scope = get_scope(db, calling_scope);
    let sema_method_scope = get_scope(db, method_scope);

    match (sema_calling_scope.kind, sema_method_scope.kind) {
        (ScopeKind::Pou(child), ScopeKind::Pou(parent)) => {
            // In case of THIS
            if child == parent {
                return true;
            }
            // In case of SUPER
            child.get_scope_id(db).inheritors(db).contains(&parent)
        }
        _ => false, // One or both are not POUs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SameNamespaceResult<'db> {
    Same,
    DifferentNamespaces((NamespaceDecl<'db>, NamespaceDecl<'db>)),
    GlobalAndNamespace(NamespaceDecl<'db>),
    NamespaceAndGlobal(NamespaceDecl<'db>),
}

/// Check if two scopes belong to the same namespace
fn is_same_namespace<'db>(
    db: &'db dyn BaseDatabase,
    scope1: ScopeId<'db>,
    scope2: ScopeId<'db>,
) -> SameNamespaceResult<'db> {
    let ns1 = find_containing_namespace(db, scope1);
    let ns2 = find_containing_namespace(db, scope2);

    match (ns1, ns2) {
        // Both scopes are in namespaces
        (Some(n1), Some(n2)) => match n1 == n2 {
            true => SameNamespaceResult::Same,
            false => SameNamespaceResult::DifferentNamespaces((n1, n2)),
        },
        // Both are in global scope
        (None, None) => SameNamespaceResult::Same,
        // Namespace <-> Global
        (Some(n1), None) => SameNamespaceResult::NamespaceAndGlobal(n1),
        // Global <-> Namespace
        (None, Some(n2)) => SameNamespaceResult::GlobalAndNamespace(n2),
    }
}

/// Find the namespace that contains the given scope using the scope iterator
fn find_containing_namespace<'db>(
    db: &'db dyn BaseDatabase,
    scope: ScopeId<'db>,
) -> Option<crate::hir_def::namespace::NamespaceDecl<'db>> {
    let sema = semantic_index(db, scope.file(db));

    for scope_info in sema.scope_iterator(db, scope) {
        if let ScopeKind::Namespace(ns) = scope_info.kind {
            return Some(ns);
        }
    }

    None
}
