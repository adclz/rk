use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;

use crate::{
    AstId, HirNodeInfo,
    check::errors::path_error::AccessError,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, PathExpr, PathExprKind, VarAccess, VariableAccess,
                VariableAccessKind,
            },
            invocation::{self, Invocation, InvocationKind},
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
    },
    hir_ty::{
        def_map::FxIndexMap, flatten::{Flatten, PathExprWalkStep}, inheritance_solver::MethodRef, name_res::{pou_names_res, resolve_namespace_access}, ty::Ty, walk::{Adjustement, ResolvedPath, ResolvedPathKind, ResolvedPathResult}
    },
};

pub trait LookUp<'db> {
    fn lookup(&self, db: &'db dyn BaseDatabase) -> ResolvedAccess<'db>;
}

impl<'db> LookUp<'db> for VariableAccess<'db> {
    fn lookup(&self, db: &'db dyn BaseDatabase) -> ResolvedAccess<'db> {
        match &self.kind(db) {
            VariableAccessKind::Direct {
                adress,
                partly,
                offset,
            } => {
                // tododododo asap
                todo!()
            }
            VariableAccessKind::Symbolic(symbolic) => symbolic.lookup(db),
        }
    }
}

impl<'db> LookUp<'db> for PathExpr<'db> {
    fn lookup(&self, db: &'db dyn BaseDatabase) -> ResolvedAccess<'db> {
        GlobalResolverCtx::new(db, *self, SearchMode::Global).resolve()
    }
}

impl<'db> LookUp<'db> for BeginPathExpr<'db> {
    fn lookup(&self, db: &'db dyn BaseDatabase) -> ResolvedAccess<'db> {
        match self.invocation(db) {
            Some(invocation) => {
                let invoc = match find_invocation_target(db, invocation, invocation.scope_id(db)) {
                    Ok(place) => place,
                    Err(e) => return e,
                };
                match self.expr(db) {
                    Some(expr) => {
                        let elements = resolve_path_rest(db, invoc.clone(), expr.flatten(db));

                        ResolvedAccess::new(
                            db,
                            ResolvedPathResult::Ok(invoc),
                            CallSite::new(self.scope_id(db), self.get_id(db)),
                            elements,
                        )
                    }
                    None => ResolvedAccess::new(
                        db,
                        ResolvedPathResult::Ok(invoc),
                        CallSite::new(self.scope_id(db), self.get_id(db)),
                        vec![],
                    ),
                }
            }
            None => match self.expr(db) {
                Some(expr) => expr.lookup(db),
                None => ResolvedAccess::new(
                    db,
                    ResolvedPathResult::Err(AccessError::NoBeginLocalItemInScope { expr: *self }),
                    CallSite::new(self.scope_id(db), self.get_id(db)),
                    vec![],
                ),
            },
        }
    }
}

impl<'db> LookUp<'db> for Invocation<'db> {
    fn lookup(&self, db: &'db dyn BaseDatabase) -> ResolvedAccess<'db> {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedAccess<'db> {
    // Primarily resolved path
    // Private: use initial() or resolved() methods
    kind: ResolvedPathResult<'db>,

    pub call_site: CallSite<'db>,

    pub elements: Vec<ResolvedPathResult<'db>>,
}

/// Represents where a variable or method is accessed from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct CallSite<'db> {
    pub scope: ScopeId<'db>,
    pub id: AstId,
}

impl<'db> CallSite<'db> {
    pub fn new(scope: ScopeId<'db>, id: AstId) -> Self {
        Self { scope, id }
    }
}

impl<'db> ResolvedAccess<'db> {
    pub fn new(
        db: &'db dyn BaseDatabase,
        kind: ResolvedPathResult<'db>,
        call_site: CallSite<'db>,
        elements: Vec<ResolvedPathResult<'db>>,
    ) -> Self {
        ResolvedAccess {
            kind,
            call_site,
            elements,
        }
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
    ) -> Option<&'db FxIndexMap<Ident, VariableDecl<'db>>> {
        Some(match self.kind {
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Pou(p),
                ..
            }) => &p.scope_id(db).def_map(db).local_variables,
            ResolvedPathResult::Ok(ResolvedPath {
                kind: ResolvedPathKind::Method(m),
                ..
            }) => &m.get_scope_id(db).def_map(db).local_variables,
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

fn find_invocation_target<'db>(
    db: &'db dyn BaseDatabase,
    path: Invocation<'db>,
    scope_id: ScopeId<'db>,
) -> Result<ResolvedPath<'db>, ResolvedAccess<'db>> {
    match get_scope(db, scope_id).kind {
        ScopeKind::MethodDecl(m) => {
            let scope = get_scope(db, m.scope_id(db));
            let parent = scope.parent.expect("A method scope always has a parent scope");
            find_invocation_target(db, path, parent)
        }
        ScopeKind::Pou(pou_decl) => {
            /*
            7Access reference
            9a THIS: Reference to own methods
            9b SUPER: Access reference to method in base class

            FUNCTION BLOCKS:

            Access reference
            10a THIS:  Reference to own methods
            10b SUPER:  Access reference to method in base function block
            10c SUPER():  Access reference to body in base function block
            */
            match path.kind(db) {
                InvocationKind::SuperBody => match pou_decl.pou(db) {
                    Pou::FunctionBlock(f) => Ok(ResolvedPath {
                        kind: ResolvedPathKind::SuperBody(pou_decl),
                        expr: CallSite::new(scope_id, path.keyword_id(db)),
                        adjustement: Adjustement::None,
                    }),
                    _ => Err(ResolvedAccess::new(
                        db,
                        ResolvedPathResult::Err(AccessError::SuperBodyOnIncompatiblePou {
                            call_site: CallSite::new(scope_id, path.keyword_id(db)),
                        }),
                        CallSite::new(scope_id, path.keyword_id(db)),
                        vec![],
                    )),
                },
                InvocationKind::Super => match pou_decl.pou(db) {
                    Pou::FunctionBlock(_) | Pou::Class(_) => Ok(ResolvedPath {
                        kind: ResolvedPathKind::Super(pou_decl),
                        expr: CallSite::new(scope_id, path.keyword_id(db)),
                        adjustement: Adjustement::None,
                    }),
                    _ => Err(ResolvedAccess::new(
                        db,
                        ResolvedPathResult::Err(AccessError::SuperOnIncompatiblePou {
                            call_site: CallSite::new(scope_id, path.keyword_id(db)),
                        }),
                        CallSite::new(scope_id, path.keyword_id(db)),
                        vec![],
                    )),
                },
                InvocationKind::This => match pou_decl.pou(db) {
                    Pou::FunctionBlock(_) | Pou::Class(_) => Ok(ResolvedPath {
                        kind: ResolvedPathKind::This(pou_decl),
                        expr: CallSite::new(scope_id, path.keyword_id(db)),
                        adjustement: Adjustement::None,
                    }),
                    _ => {
                        return Err(ResolvedAccess::new(
                            db,
                            ResolvedPathResult::Err(AccessError::ThisOnIncompatiblePou {
                                call_site: CallSite::new(scope_id, path.keyword_id(db)),
                            }),
                            CallSite::new(scope_id, path.keyword_id(db)),
                            vec![],
                        ));
                    }
                },
            }
        }
        _ => unreachable!("An invocation will always be in a POU scope"),
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

                ResolvedAccess::new(
                    self.db,
                    ResolvedPathResult::Ok(place),
                    CallSite::new(
                        self.path_expr.scope_id(self.db),
                        self.path_expr.get_id(self.db),
                    ),
                    elements,
                )
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
    let flatten = path_expr.flatten(db);
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
                        CallSite::new(expr.scope_id(db), expr.get_id(db)),
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
                            if matches!(
                                err,
                                AccessError::TypeHasNoField { .. }
                                    | AccessError::UnknownField { .. }
                            ) {
                                return Err(ResolvedAccess::new(
                                    db,
                                    ResolvedPathResult::Err(AccessError::NoLocalItemInScope {
                                        expr: path_expr,
                                    }),
                                    CallSite::new(expr.scope_id(db), expr.get_id(db)),
                                    vec![],
                                ));
                            }
                            return Err(ResolvedAccess::new(
                                db,
                                ResolvedPathResult::Err(err),
                                CallSite::new(expr.scope_id(db), expr.get_id(db)),
                                vec![],
                            ));
                        }
                    }
                }
            } else if let ScopeKind::MethodDecl(m) = scope.kind {
                match MethodRef::from(m).walk(db, first) {
                    Ok(resolved) => {
                        return Ok((resolved, flatten[1..].as_ref()));
                    }
                    Err(err) => {
                        if let SearchMode::Local = mode {
                            if matches!(
                                err,
                                AccessError::TypeHasNoField { .. }
                                    | AccessError::UnknownField { .. }
                            ) {
                                return Err(ResolvedAccess::new(
                                    db,
                                    ResolvedPathResult::Err(AccessError::NoLocalItemInScope {
                                        expr: path_expr,
                                    }),
                                    CallSite::new(expr.scope_id(db), expr.get_id(db)),
                                    vec![],
                                ));
                            }
                            return Err(ResolvedAccess::new(
                                db,
                                ResolvedPathResult::Err(err),
                                CallSite::new(expr.scope_id(db), expr.get_id(db)),
                                vec![],
                            ));
                        }
                    }
                }
            }

            if let SearchMode::Global = mode {
                // Try local POU names
                if let Some(pou) = pou_names_res(db, ident) {
                    return Ok((
                        ResolvedPathKind::Pou(pou).with_call_site(
                            db,
                            *first.get_expr(),
                            Adjustement::None,
                        ),
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
                let access = NamespaceAccess::new(db, Some(path), *ident);
                if let Some(pou) = resolve_namespace_access(db, &access) {
                    return Ok((
                        ResolvedPathKind::Pou(pou).with_call_site(
                            db,
                            *first.get_expr(),
                            Adjustement::None,
                        ),
                        flatten[fragments.len() - 1..].as_ref(),
                    ));
                }
            }
            Err(ResolvedAccess::new(
                db,
                ResolvedPathResult::Err(AccessError::NoLocalItemInScope {
                    expr: *first.get_expr(),
                }),
                CallSite::new(first.get_expr().scope_id(db), first.get_expr().get_id(db)),
                vec![],
            ))
        }
        None => Err(ResolvedAccess::new(
            db,
            ResolvedPathResult::Err(AccessError::NoLocalItemInScope { expr: path_expr }),
            CallSite::new(path_expr.scope_id(db), path_expr.get_id(db)),
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

impl<'db> HirNodeInfo<'db> for CallSite<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope
    }
}

impl<'db> HirNodeInfo<'db> for ResolvedAccess<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.call_site.get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.call_site.get_scope_id(db)
    }
}
