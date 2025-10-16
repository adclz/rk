use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;

use crate::{
    check::errors::path_error::PathResolveError, hir_def::{
        expressions::{
            expression::{
                Expr, PathExpr, PathExprKind, VarAccess, VariableAccess, VariableAccessKind,
            },
            invocation::{Invocation, InvocationKind},
            spec::{Spec, StructElement},
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespaceAccess, NamespacePath},
        },
        pous::{
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index, visibility::Visibility,
    }, hir_ty::{
        inheritance_solver::MethodRef, name_res::{pou_names_res, resolve_namespace_access}, signatures::LocalVariables, ty::{SearchMode, Ty}, walk::{ResolvedPath, ResolvedPathResult}
    }, AstId, HirNodeInfo, TypeInfo
};

#[salsa::tracked]
pub fn resolve_var_access<'db>(
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
) -> ResolvedAccess<'db> {
    VarAccessResolverCtx::new(db, access).resolve()
}

#[salsa::tracked]
pub fn resolve_path_expr<'db>(
    db: &'db dyn BaseDatabase,
    path: PathExpr<'db>,
) -> ResolvedAccess<'db> {
    GlobalResolverCtx::new(db, path).resolve()
}

#[salsa::tracked(debug)]
pub struct ResolvedAccess<'db> {
    // Where the access was made
    pub call_site: CallSite<'db>,

    // Where the variable is stored / how it is accessed
    pub kind: ResolvedPathResult<'db>,

    pub elements: Vec<ResolvedPathResult<'db>>,
}

/// Represents where a variable or method is accessed from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum CallSite<'db> {
    /// THIS or SUPER keyword
    InvocationKeyword(AstId, Invocation<'db>),
    /// Invocation expression
    Invocation(Invocation<'db>),
    /// Variable Access (direct or symbolic)
    Access(VariableAccess<'db>),
    /// Non-formal parameter
    NonFormal(Expr<'db>),
    /// Formal parameter
    Formal(SpanIdent<'db>),
    /// Path expression - global access
    PathExpr(PathExpr<'db>),
}

impl<'db> CallSite<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            CallSite::InvocationKeyword(id, i) => match i.kind(db) {
                InvocationKind::This { .. } => "THIS".to_string(),
                InvocationKind::Super { .. } => "SUPER".to_string(),
                InvocationKind::SuperBody => format!("SUPER()"),
            },
            CallSite::Access(access) => format!(
                "{}",
                match access.kind(db) {
                    VariableAccessKind::Direct { adress, .. } => adress.text(db).to_string(),
                    VariableAccessKind::Symbolic(symbolic) =>
                        symbolic.kind.ident(db).text(db).to_string(),
                }
            ),
            CallSite::PathExpr(path) => path.ident(db).text(db).to_string(),
            _ => "{unknown}".to_string(),
        }
    }
}

impl<'db> ResolvedAccess<'db> {
    pub fn resolved(&self, db: &'db dyn BaseDatabase) -> Result<ResolvedPath<'db>, PathResolveError<'db>> {
        match self.elements(db).last() {
            Some(ResolvedPathResult::Ok(ok)) => Ok(ok.clone()),
            Some(ResolvedPathResult::Err(err)) => Err(err.clone()),
            _ => match &self.kind(db) {
                ResolvedPathResult::Ok(ok) => Ok(ok.clone()),
                ResolvedPathResult::Err(err) => Err(err.clone()),
            },
        }
    }

    pub fn to_ty(&self, db: &'db dyn BaseDatabase) -> Result<Option<Ty<'db>>, PathResolveError<'db>> {
        match self.resolved(db) {
            Ok(r) => Ok(r.to_ty(db)),
            Err(err) => Err(err),
        }
    }

    fn as_var(&self, db: &'db dyn BaseDatabase) -> Option<VariableDecl<'db>> {
        match self.kind(db) {
            ResolvedPathResult::Ok(ResolvedPath::Variable(v)) => Some(v),
            _ => None,
        }
    }

    pub fn is_method(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(
            self.kind(db),
            ResolvedPathResult::Ok(ResolvedPath::Method(_))
        )
    }

    pub fn is_struct_field(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(
            self.kind(db),
            ResolvedPathResult::Ok(ResolvedPath::StructElement(_))
        )
    }

    pub fn is_function(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), ResolvedPathResult::Ok(ResolvedPath::Pou(p)) if matches!(p.pou(db), Pou::Function(_)))
    }

    pub fn is_function_block(&self, db: &'db dyn BaseDatabase) -> bool {
        matches!(self.kind(db), ResolvedPathResult::Ok(ResolvedPath::Pou(p)) if matches!(p.pou(db), Pou::FunctionBlock(_)))
    }

    pub fn is_callable(&self, db: &'db dyn BaseDatabase) -> bool {
        self.is_function(db) || self.is_function_block(db) || self.is_method(db)
    }

    pub fn callable(&self, db: &'db dyn BaseDatabase) -> Option<&'db IndexMap<Ident, VariableDecl<'db>>> {
        Some(match self.kind(db) {
            ResolvedPathResult::Ok(ResolvedPath::Pou(p)) => p.local_variables(db),
            ResolvedPathResult::Ok(ResolvedPath::Method(m)) => m.local_variables(db),
            _ => None?
        })
    }

    pub fn with_return_type(&self, db: &'db dyn BaseDatabase) -> Option<Spec<'db>> {
        match self.kind(db) {
            ResolvedPathResult::Ok(ResolvedPath::Pou(p)) => match p.pou(db) {
                Pou::Function(f) => f.return_type(db).copied(),
                _ => None,
            },
            ResolvedPathResult::Ok(ResolvedPath::Method(m)) => m.return_type(db).copied(),
            _ => None,
        }
    }

    pub fn visibility(&self, db: &'db dyn BaseDatabase) -> Visibility {
        match self.kind(db) {
            ResolvedPathResult::Ok(ResolvedPath::Method(m)) => m.visibility(db),
            _ => Visibility::PUBLIC
        }
    }

    pub fn is_variable(&self, db: &'db dyn BaseDatabase) -> bool {
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
        match &self.kind(db) {
            ResolvedPathResult::Ok(ok) => ok.decl_name(db),
            ResolvedPathResult::Err(err) => "{unknown}".to_string(),
        }
    }

    pub fn decl_kind(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            ResolvedPathResult::Ok(ResolvedPath::Variable(v)) => match v.kind(db) {
                VariableKind::Var => "VAR".to_string(),
                VariableKind::Input => "VAR_INPUT".to_string(),
                VariableKind::Output => "VAR_OUPUT".to_string(),
                VariableKind::InOut => "VAR_INOUT".to_string(),
                VariableKind::Temp => "VAR_TEMP".to_string(),
                VariableKind::Access => "VAR_ACCESS".to_string(),
                VariableKind::Global => "VAR_GLOBAL".to_string(),
                VariableKind::External => "VAR_EXTERNAL".to_string(),
                VariableKind::Config => "VAR_CONFIG".to_string(),
            },
            _ => "".to_string(),
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
                let target = find_primary_target(self.db, symbolic.kind);
                match target {
                    Some((place, rest)) => ResolvedAccess::new(
                        self.db,
                        CallSite::Access(self.access),
                        ResolvedPathResult::Ok(place.clone()),
                        resolve_path_rest(self.db, place, rest),
                    ),
                    None => ResolvedAccess::new(
                        self.db,
                        CallSite::Access(self.access),
                        ResolvedPathResult::Err(PathResolveError::NoItemInScope {
                            expr: symbolic.kind,
                            scope: symbolic.kind.scope_id(self.db),
                        }),
                        vec![],
                    ),
                }
            }
        }
    }
}

pub struct GlobalResolverCtx<'db> {
    db: &'db dyn BaseDatabase,
    path_expr: PathExpr<'db>,
}

impl<'db> GlobalResolverCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, path_expr: PathExpr<'db>) -> Self {
        Self { db, path_expr }
    }

    pub fn resolve(&self) -> ResolvedAccess<'db> {
        let target = find_primary_target(self.db, self.path_expr);
        match target {
            Some((place, rest)) => ResolvedAccess::new(
                self.db,
                CallSite::PathExpr(self.path_expr),
                ResolvedPathResult::Ok(place.clone()),
                resolve_path_rest(self.db, place, rest),
            ),
            None => ResolvedAccess::new(
                self.db,
                CallSite::PathExpr(self.path_expr),
                ResolvedPathResult::Err(PathResolveError::NoItemInScope {
                    expr: self.path_expr,
                    scope: self.path_expr.scope_id(self.db),
                }),
                vec![],
            ),
        }
    }
}

fn find_primary_target<'db>(
    db: &'db dyn BaseDatabase,
    path_expr: PathExpr<'db>,
) -> Option<(ResolvedPath<'db>, &'db [PathExprWalkStep<'db>])> {
    let flatten = path_expr.flatten_steps(db);
    match flatten.first() {
        Some(first) => {
            let (ident, expr) = match first {
                PathExprWalkStep::Field { ident, expr } => (ident, expr),
                PathExprWalkStep::Deref { target, expr } => (target, expr),
                PathExprWalkStep::Index { expr } => return None, // cannot start with index
            };

            let sema = semantic_index(db, expr.scope_id(db).file(db));
            let scope = sema.get_scope(db, expr.scope_id(db));

            // Search for variables in scope
            if let ScopeKind::Pou(pou) = scope.kind {
                if let Ok(resolved) = pou.walk(db, first) {
                    return Some((resolved, flatten[1..].as_ref()));
                }
            }

            // Try local POU names
            if let Some(pou) = pou_names_res(db, ident, expr.scope_id(db)) {
                return Some((ResolvedPath::Pou(pou), flatten[1..].as_ref()));
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
                        fragments.push(ident.clone());
                    }
                    _ => break,
                }
            }

            let path = NamespacePath::from((db, &fragments));
            let access = NamespaceAccess::new(db, Some(path), ident);
            if let Some(pou) = resolve_namespace_access(db, expr.scope_id(db), access) {
                return Some((
                    ResolvedPath::Pou(pou),
                    flatten[fragments.len() - 1..].as_ref(),
                ));
            }
            None
        }
        None => None,
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
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self.call_site(db) {
            CallSite::Access(access) => access.get_id(db),
            CallSite::NonFormal(expr) => expr.get_id(db),
            CallSite::Formal(param) => param.get_id(db),
            CallSite::Invocation(ty) => match ty.kind(db) {
                InvocationKind::This { path } => path.get_id(db),
                InvocationKind::Super { path } => path.get_id(db),
                InvocationKind::SuperBody => ty.get_id(db),
            },
            CallSite::InvocationKeyword(id, _) => id,
            CallSite::PathExpr(path) => path.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self.call_site(db) {
            CallSite::Access(access) => access.get_scope_id(db),
            CallSite::NonFormal(expr) => expr.get_scope_id(db),
            CallSite::Formal(param) => param.get_scope_id(db),
            CallSite::Invocation(ty) => ty.get_scope_id(db),
            CallSite::InvocationKeyword(id, scope) => scope.get_scope_id(db),
            CallSite::PathExpr(path) => path.get_scope_id(db),
        }
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
                VarAccess::Deref(target) => result.push(PathExprWalkStep::Deref {
                    expr: self,
                    target: target,
                }),
            },
        }
        result
    }
}
