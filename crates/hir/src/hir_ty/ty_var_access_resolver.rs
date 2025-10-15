use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{path_error::PathResolveError, var_error::VarResolveError}, hir_def::{
        expressions::{
            expression::{
                Expr, PathExpr, PathExprKind, VarAccess, VariableAccess, VariableAccessKind,
            },
            invocation::{Invocation, InvocationKind}, spec::StructElement,
        },
        interned::{
            identifier::SpanIdent,
            namespace::{NamespaceAccess, NamespacePath},
        },
        pous::{
            pou::PouDecl,
            variable::{VariableDecl, VariableKind},
        },
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
    }, hir_ty::{
        inheritance_solver::MethodRef,
        name_res::{pou_names_res, resolve_namespace_access},
        ty::{ty_for_pou, SearchMode, Ty},
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
    pub kind: Place<'db>,

    pub elements: Vec<ResolvedPathElement<'db>>,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Place<'db> {
    Variable(VariableDecl<'db>),
    Pou(PouDecl<'db>),
    Method(MethodRef<'db>),
    StructElement(StructElement<'db>), // A field in a struct
    Invalid,
}

impl<'db> ResolvedAccess<'db> {
    fn as_var(&self, db: &'db dyn BaseDatabase) -> Option<VariableDecl<'db>> {
        match self.kind(db) {
            Place::Variable(v) => Some(v),
            _ => None,
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
}

impl<'db> ResolvedAccess<'db> {
    pub fn ty(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, VarResolveError<'db>> {
        match self.kind(db) {
            Place::Variable(v) => Ok(v.spec(db).spec_to_ty(db)),
            Place::Method(m) => Ok(m.to_ty(db)),
            Place::Pou(p) => Ok(ty_for_pou(db, p)),
            Place::StructElement(e) => Ok(e.spec(db).spec_to_ty(db)),
            Place::Invalid => Err(VarResolveError::Unknown {
                call_site: self.call_site(db),
            }),
        }
    }

    pub fn decl_name(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            Place::Variable(v) => v.name(db).text(db).to_string(),
            Place::Pou(p) => p.name(db).text(db).to_string(),
            Place::Method(m) => m.name(db).text(db).to_string(),
            Place::StructElement(e) => e.name(db).text(db).to_string(),
            Place::Invalid => "{unknown}".to_string(),
        }
    }

    pub fn decl_kind(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            Place::Variable(v) => match v.kind(db) {
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
                    Some((place, rest)) => {
                        let ty = match &place {
                            Place::Variable(v) => v.spec(self.db).spec_to_ty(self.db),
                            Place::Pou(p) => ty_for_pou(self.db, *p),
                            Place::Method(m) => m.to_ty(self.db),
                            Place::StructElement(e) => e.spec(self.db).spec_to_ty(self.db),
                            Place::Invalid => {
                                return ResolvedAccess::new(
                                    self.db,
                                    CallSite::Access(self.access),
                                    Place::Invalid,
                                    vec![],
                                );
                            }
                        };
                        ResolvedAccess::new(
                            self.db,
                            CallSite::Access(self.access),
                            place,
                            resolve_path_rest(self.db, ty, rest),
                        )
                    }
                    None => ResolvedAccess::new(
                        self.db,
                        CallSite::Access(self.access),
                        Place::Invalid,
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
            Some((place, rest)) => {
                let ty = match &place {
                    Place::Variable(v) => v.spec(self.db).spec_to_ty(self.db),
                    Place::Pou(p) => ty_for_pou(self.db, *p),
                    Place::Method(m) => m.to_ty(self.db),
                    Place::StructElement(e) => e.spec(self.db).spec_to_ty(self.db),
                    Place::Invalid => {
                        return ResolvedAccess::new(
                            self.db,
                            CallSite::PathExpr(self.path_expr),
                            Place::Invalid,
                            vec![],
                        );
                    }
                };
                ResolvedAccess::new(
                    self.db,
                    CallSite::PathExpr(self.path_expr),
                    place,
                    resolve_path_rest(self.db, ty, rest),
                )
            }
            None => ResolvedAccess::new(
                self.db,
                CallSite::PathExpr(self.path_expr),
                Place::Invalid,
                vec![],
            ),
        }
    }
}

fn find_primary_target<'db>(
    db: &'db dyn BaseDatabase,
    path_expr: PathExpr<'db>,
) -> Option<(Place<'db>, &'db [PathExprWalkStep<'db>])> {
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
                let ty = ty_for_pou(db, pou);

                if let Ok(ty) = ty.walk(db, first, SearchMode::Global) {
                    todo!()
                    //return Some((flatten[1..].as_ref()));
                }

                // Special case: if the first element is the same as the current POU, it's a self-reference
                if pou.name(db) == &ident.ident {
                    return Some((Place::Pou(pou), flatten[1..].as_ref()));
                }
            }

            // Try local POU names
            if let Some(pou) = pou_names_res(db, ident, expr.scope_id(db)) {
                return Some((Place::Pou(pou), flatten[1..].as_ref()));
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
                return Some((Place::Pou(pou), flatten[fragments.len() - 1..].as_ref()));
            }
            None
        }
        None => None,
    }
}

fn resolve_path_rest<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    rest: &'db [PathExprWalkStep],
) -> Vec<ResolvedPathElement<'db>> {
    let mut result = vec![];
    if rest.is_empty() {
        return result;
    }

    for step in rest {
        match ty.walk(db, step, SearchMode::Global) {
            Ok(resolved) => result.push(ResolvedPathElement {
                expr: *step.get_expr(),
                kind: ResolvedPathElementKind::Ty(resolved),
            }),
            Err(err) => {
                result.push(ResolvedPathElement {
                    expr: *step.get_expr(),
                    kind: ResolvedPathElementKind::Error(err),
                });
                break;
            } // stop on first error
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

/// Represents a resolved element in a path expression.
/// Contains the expression and its type.
#[derive(Debug, Clone, PartialEq, Hash, Eq, salsa::Update)]
pub struct ResolvedPathElement<'db> {
    // The expression that was resolved
    pub expr: PathExpr<'db>,
    // The type of the resolved element
    pub kind: ResolvedPathElementKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Hash, Eq, salsa::Update)]
pub enum ResolvedPathElementKind<'db> {
    Ty(Ty<'db>),
    Error(PathResolveError<'db>),
}

impl<'db> ResolvedPathElement<'db> {
    pub fn new(expr: PathExpr<'db>, kind: ResolvedPathElementKind<'db>) -> Self {
        Self { expr, kind }
    }

    pub fn get_expr(&self) -> &PathExpr<'db> {
        &self.expr
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
    pub fn flatten_steps(self, db: &'db dyn BaseDatabase) -> Vec<PathExprWalkStep<'db>> {
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

impl<'db> HirNodeInfo<'db> for ResolvedPathElement<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        // We do not use the id field of the expression because of the way PathExpr are built from the AST.
        // Instead, get_id method will return the id of *this* specific path element.
        self.expr.get_id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.expr.scope_id(db)
    }
}
