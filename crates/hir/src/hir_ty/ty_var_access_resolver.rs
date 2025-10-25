use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;

use crate::{
    check::errors::path_error::AccessError, hir_def::{
        expressions::{
            expression::{
                Expr, PathExpr, PathExprKind, VarAccess, VariableAccess, VariableAccessKind,
            },
            invocation::{Invocation, InvocationKind},
            spec::Spec,
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespaceAccess, NamespacePath},
        },
        pous::{
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
        visibility::Visibility,
    }, hir_ty::{
        inheritance_solver::MethodRef, name_res::{pou_names_res, resolve_namespace_access}, signatures::LocalVariables, ty::Ty, walk::{Adjustement, ResolvedPath, ResolvedPathKind, ResolvedPathResult}
    }, AstId, HirNodeInfo
};

#[salsa::tracked]
pub fn resolve_var_access<'db>(
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
) -> ResolvedAccess<'db> {
    VarAccessResolverCtx::new(db, access).resolve()
}

#[salsa::tracked]
pub fn resolve_local_path_expr<'db>(
    db: &'db dyn BaseDatabase,
    path: PathExpr<'db>,
) -> ResolvedAccess<'db> {
    GlobalResolverCtx::new(db, path, SearchMode::Local).resolve()
}

#[salsa::tracked]
pub fn resolve_global_path_expr<'db>(
    db: &'db dyn BaseDatabase,
    path: PathExpr<'db>,
) -> ResolvedAccess<'db> {
    GlobalResolverCtx::new(db, path, SearchMode::Global).resolve()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedAccess<'db> {
    // Primarily resolved path
    // Private: use initial() or resolved() methods
    kind: ResolvedPathResult<'db>,

    pub elements: Vec<ResolvedPathResult<'db>>,
}

/// Represents where a variable or method is accessed from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct CallSite<'db> { pub scope: ScopeId<'db>, pub id: AstId }

impl<'db> CallSite<'db> {
    pub fn new(scope: ScopeId<'db>, id: AstId) -> Self {
        Self { scope, id }
    }
}

impl<'db> ResolvedAccess<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        kind: ResolvedPathResult<'db>,
        elements: Vec<ResolvedPathResult<'db>>,
    ) -> Self {
        ResolvedAccess { kind, elements }
    }

    /// Get the initially resolved path (first element)
    pub fn resolved(
        &self,
        db: &'db dyn BaseDatabase,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        match &self.kind {
            ResolvedPathResult::Ok(ok) => Ok(ok.clone()),
            ResolvedPathResult::Err(err) => Err(err.clone()),
        }
    }

    /// Get the last resolved path (after all elements)
    /// If there are no elements, returns the initial path
    pub fn fully_resolved(
        &self,
        db: &'db dyn BaseDatabase,
    ) -> Result<ResolvedPath<'db>, AccessError<'db>> {
        match self.elements.last() {
            Some(ResolvedPathResult::Ok(ok)) => Ok(ok.clone()),
            Some(ResolvedPathResult::Err(err)) => Err(err.clone()),
            _ => self.resolved(db),
        }
    }

    /// Tries to resolve this acess to a concrete type
    pub fn try_to_ty(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, AccessError<'db>> {
        match self.fully_resolved(db) {
            Ok(r) => r.try_to_ty(db),
            Err(err) => Err(err),
        }
    }

    pub fn is_method(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(
            self.kind,
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Method(_),
                ..
            })
        )
    }

    pub fn as_pou(&self, db: &'db dyn BaseDatabase) -> Option<PouDecl<'db>> {
        match self.kind {
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Pou(p),
                ..
            }) => Some(p),
            _ => None,
        }
    }

    pub fn as_method(&self, db: &'db dyn BaseDatabase) -> Option<MethodRef<'db>> {
        match self.kind {
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Method(m),
                ..
            }) => Some(m),
            _ => None,
        }
    }

    pub fn is_struct_field(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(
            self.kind,
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::StructElement(_),
                ..
            })
        )
    }

    pub fn callable(
        &self,
        db: &'db dyn BaseDatabase,
    ) -> Option<&'db IndexMap<Ident, VariableDecl<'db>>> {
        Some(match self.kind {
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Pou(p),
                ..
            }) => p.local_variables(db),
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Method(m),
                ..
            }) => m.local_variables(db),
            _ => None?,
        })
    }

    pub fn with_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Spec<'db>> {
        match self.kind {
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Pou(p),
                ..
            }) => match p.pou(db) {
                Pou::Function(f) => f.return_type(db).copied(),
                _ => None,
            },
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Method(m),
                ..
            }) => m.return_type(db).copied(),
            _ => None,
        }
    }

    pub fn visibility(&self, db: &'db dyn BaseDatabase) -> Visibility {
        match self.kind {
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Method(m),
                ..
            }) => m.visibility(db),
            _ => Visibility::PUBLIC,
        }
    }

    pub fn as_var(&self, db: &'db dyn BaseDatabase) -> Option<VariableDecl<'db>> {
        match &self.kind {
            ResolvedPathResult::Ok(r) => r.as_var(db),
            _ => None,
        }
    }

    pub fn is_var(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_var(db))
    }

    pub fn is_var_input(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_input(db))
    }

    pub fn is_var_output(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_output(db))
    }

    pub fn is_var_in_out(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_in_out(db))
    }

    pub fn is_var_external(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_external(db))
    }

    pub fn is_var_global(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_global(db))
    }

    pub fn is_var_access(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_access(db))
    }

    pub fn is_var_temp(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_temp(db))
    }

    pub fn is_var_config(&self, db: &'db dyn BaseDatabase) -> bool {
        self.as_var(db).is_some_and(|var| var.is_config(db))
    }

    pub fn decl_name(&self, db: &'db dyn BaseDatabase) -> String {
        match &self.kind {
            ResolvedPathResult::Ok(ok) => ok.decl_name(db),
            ResolvedPathResult::Err(err) => "{unknown}".to_string(),
        }
    }
}

pub struct VarAccessResolverCtx<'db> {
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
}

impl<'db> VarAccessResolverCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, access: VariableAccess<'db>) -> Self {
        Self { db, access }
    }

    pub fn resolve(&self) -> ResolvedAccess<'db> {
        match &self.access.kind(self.db) {
            VariableAccessKind::Direct {
                adress,
                partly,
                offset,
            } => {
                // tododododo asap
                todo!()
            }
            VariableAccessKind::Symbolic(symbolic) => {
                match find_primary_target(self.db, symbolic.kind, SearchMode::Local) {
                    Ok((place, rest)) => {
                        let elements = resolve_path_rest(self.db, place.clone(), rest);

                        ResolvedAccess::new(self.db, ResolvedPathResult::Ok(place), elements)
                    }
                    Err(e) => return e,
                }
            }
        }
    }
}

pub struct GlobalResolverCtx<'db> {
    db: &'db dyn BaseDatabase,
    path_expr: PathExpr<'db>,
    search_mode: SearchMode,
}

impl<'db> GlobalResolverCtx<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        path_expr: PathExpr<'db>,
        search_mode: SearchMode,
    ) -> Self {
        Self {
            db,
            path_expr,
            search_mode,
        }
    }

    pub fn resolve(&self) -> ResolvedAccess<'db> {
        match find_primary_target(self.db, self.path_expr, self.search_mode) {
            Ok((place, rest)) => {
                let elements = resolve_path_rest(self.db, place.clone(), rest);

                ResolvedAccess::new(self.db, ResolvedPathResult::Ok(place), elements)
            }
            Err(e) => return e,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SearchMode {
    Local,
    Global,
}

fn find_primary_target<'db>(
    db: &'db dyn BaseDatabase,
    path_expr: PathExpr<'db>,
    mode: SearchMode,
) -> Result<(ResolvedPath<'db>, &'db [PathExprWalkStep<'db>]), ResolvedAccess<'db>> {
    let flatten = path_expr.flatten_steps(db);
    match flatten.first() {
        Some(first) => {
            let (ident, expr) = match first {
                PathExprWalkStep::Field { ident, expr } => (ident, expr),
                PathExprWalkStep::Deref { target, expr } => (target, expr),
                PathExprWalkStep::Index { expr } => {
                    return Err(ResolvedAccess::new(
                        db,
                        ResolvedPathResult::Err(AccessError::NoLocalItemInScope {
                            expr: path_expr,
                        }),
                        vec![],
                    ));
                } // cannot start with index
            };

            let scope = get_scope(db, expr.scope_id(db));

            // Search for variables in scope
            if let ScopeKind::Pou(pou) = scope.kind {
                match pou.walk(db, first) {
                    Ok(resolved) => {
                        return Ok((resolved, flatten[1..].as_ref()));
                    }
                    Err(err) => {
                        if let SearchMode::Local = mode {
                            if matches!(err, AccessError::TypeHasNoField { .. } | AccessError::UnknownField { .. }) {
                                return Err(ResolvedAccess::new(
                                    db,
                                    ResolvedPathResult::Err(AccessError::NoLocalItemInScope {
                                        expr: path_expr,
                                    }),
                                    vec![],
                                ));
                            }
                            return Err(ResolvedAccess::new(
                                db,
                                ResolvedPathResult::Err(err),
                                vec![],
                            ));
                        }
                    }
                }
            }

            if let SearchMode::Global = mode {
                // Try local POU names
                if let Some(pou) = pou_names_res(db, ident, expr.scope_id(db)) {
                    return Ok((
                        ResolvedPathKind::Pou(pou)
                            .with_call_site(db, *first.get_expr(), Adjustement::None),
                        flatten[1..].as_ref(),
                    ));
                }

                // Try Namespaces
                // Since the IEC standard states that both namespaces and path expressions should be dotted,
                // we need to loop through all steps until we find a matching namespace
                // this is not very efficient, but should work for now

                // first get all fragments that could look like a namespace
                // we just iterate through all steps until we find a non-Field step
                let mut fragments = vec![];
                for step in flatten {
                    match step {
                        PathExprWalkStep::Field { ident, .. } => {
                            fragments.push(*ident);
                        }
                        _ => break,
                    }
                }

                let path = NamespacePath::from((db, &fragments));
                let access = NamespaceAccess::new(db, Some(path), ident);
                if let Some(pou) = resolve_namespace_access(db, access) {
                    return Ok((
                        ResolvedPathKind::Pou(pou)
                            .with_call_site(db, *first.get_expr(), Adjustement::None),
                        flatten[fragments.len() - 1..].as_ref(),
                    ));
                }
            }
            Err(ResolvedAccess::new(
                db,
                ResolvedPathResult::Err(AccessError::NoLocalItemInScope {
                    expr: *first.get_expr(),
                }),
                vec![],
            ))
        }
        None => Err(ResolvedAccess::new(
            db,
            ResolvedPathResult::Err(AccessError::NoLocalItemInScope { expr: path_expr }),
            vec![],
        )),
    }
}

fn resolve_path_rest<'db>(
    db: &'db dyn BaseDatabase,
    fragment: ResolvedPath<'db>,
    rest: &'db [PathExprWalkStep],
) -> Vec<ResolvedPathResult<'db>> {
    let mut result = vec![];
    if rest.is_empty() {
        return result;
    }

    let mut fragment = fragment;

    for step in rest {
        match fragment.walk(db, step) {
            Err(err) => {
                result.push(ResolvedPathResult::Err(err));
                break;
            }
            Ok(r) => {
                result.push(ResolvedPathResult::Ok(r.clone()));
                fragment = r;
            }
        }
    }
    result
}

impl<'db> HirNodeInfo<'db> for ResolvedAccess<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        match &self.kind {
            ResolvedPathResult::Ok(ok) => ok.get_id(db),
            ResolvedPathResult::Err(err) => match err {
                AccessError::NoLocalItemInScope { expr } => expr.get_id(db),
                AccessError::InvalidTypeAccess { access } => access.get_id(db),
                AccessError::UnknownField { expr, .. } => expr.get_id(db),
                AccessError::TypeHasNoField { expr, .. } => expr.get_id(db),
                AccessError::NotAnArray { expr, .. } => expr.get_id(db),
                AccessError::NotAReference { expr, .. } => expr.get_id(db),
                AccessError::NoItemInScope { access } => access.get_id(db),
            },
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        match &self.kind {
            ResolvedPathResult::Ok(ok) => ok.get_scope_id(db),
            ResolvedPathResult::Err(err) => match err {
                AccessError::NoLocalItemInScope { expr } => expr.get_scope_id(db),
                AccessError::InvalidTypeAccess { access } => access.get_scope_id(db),
                AccessError::UnknownField { expr, .. } => expr.get_scope_id(db),
                AccessError::TypeHasNoField { expr, .. } => expr.get_scope_id(db),
                AccessError::NotAnArray { expr, .. } => expr.get_scope_id(db),
                AccessError::NotAReference { expr, .. } => expr.get_scope_id(db),
                AccessError::NoItemInScope { access } => access.get_scope_id(db),
            },
        }
    }
}

impl<'db> HirNodeInfo<'db> for CallSite<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum PathExprWalkStep<'db> {
    Field {
        ident: SpanIdent<'db>,
        expr: PathExpr<'db>,
    }, // By name
    Index {
        expr: PathExpr<'db>,
    }, // By index
    Deref {
        target: SpanIdent<'db>,
        expr: PathExpr<'db>,
    }, // For pointers
}

impl PathExprWalkStep<'_> {
    pub fn get_expr(&self) -> &PathExpr<'_> {
        match self {
            PathExprWalkStep::Field { expr, .. } => expr,
            PathExprWalkStep::Index { expr } => expr,
            PathExprWalkStep::Deref { expr, .. } => expr,
        }
    }
}

#[salsa::tracked]
impl<'db> PathExpr<'db> {
    #[salsa::tracked(returns(ref))]
    fn flatten_steps(self, db: &'db dyn BaseDatabase) -> Vec<PathExprWalkStep<'db>> {
        let mut result = Vec::new();

        match self.expr(db) {
            PathExprKind::Field(field_expr) => {
                result.extend(field_expr.path.flatten_steps(db).iter().cloned());
                match &field_expr.var {
                    VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                        expr: self,
                        ident: *simple,
                    }),
                    VarAccess::Deref(target) => result.push(PathExprWalkStep::Deref {
                        expr: self,
                        target: *target,
                    }),
                }
            }
            PathExprKind::Index(index_expr) => {
                result.extend(index_expr.path.flatten_steps(db).iter().cloned());
                result.push(PathExprWalkStep::Index { expr: self });
            }
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => result.push(PathExprWalkStep::Field {
                    expr: self,
                    ident: simple,
                }),
                VarAccess::Deref(target) => {
                    result.push(PathExprWalkStep::Deref { expr: self, target })
                }
            },
        }
        result
    }
}
