use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::inheritance::MethodError, hir_def::{
        expressions::invocation::{Invocation, InvocationKind},
        pous::pou::Pou,
        scope::{FileScopeId, Scope, ScopeKind},
    }, hir_ty::{
        func_call_resolver::ResolvedParam, inheritance_solver::{declared_methods, inherited_methods, MethodRef}, name_res::resolve_namespace_access, param_resolver::{resolve_invocation_func_call_parameters, resolve_invocation_method_parameters, resolve_method_parameters}, ty_var_access_resolver::{CallSite, ResolvedAccess}, walk::{Adjustement, ResolvedPath, ResolvedPathKind, ResolvedPathResult}
    }, AstId, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedInvocationResult<'db> {
    pub target: ResolvedInvocation<'db>,
    pub params: Vec<ResolvedParam<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ResolvedInvocation<'db> {
    pub invocation: Invocation<'db>,
    pub kind: ResolvedMethodKind<'db>,
}

impl<'db> ResolvedInvocation<'db> {
    pub fn new(invocation: Invocation<'db>, kind: ResolvedMethodKind<'db>) -> Self {
        Self { invocation, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolvedMethodKind<'db> {
    InheritedMethod {
        target: ResolvedAccess<'db>,
        method: MethodRef<'db>,
    },
    DeclaredMethod {
        target: ResolvedAccess<'db>,
        method: MethodRef<'db>,
    },
    FunctionBlockBody {
        target: ResolvedAccess<'db>,
    },
    Unresolved(MethodError<'db>),
}

/*
CLASSES:

Access reference
9a THIS: Reference to own methods
9b SUPER: Access reference to method in base class

FUNCTION BLOCKS:

Access reference
10a THIS:  Reference to own methods
10b SUPER:  Access reference to method in base function block
10c SUPER():  Access reference to body in base function block

The derived function blocks and their base function block may each have a function block
body. The function block body is not automatically inherited from the base function block. It is
empty by default. It can be called using SUPER().
In this case the rules above for EXTENDS of a function block and additionally the following
rules apply:
1. The body (if any) of the derived function block type will be executed when the function
block is called.
2. To execute additionally the body of the base function block (if any) in the derived function
block the call of SUPER() shall be used. The call of SUPER() has no parameters.
The call SUPER() shall occur once in the function block body and shall not be in a loop.
3. The names of the variables in the base and the derived function blocks shall be unique.
4. The call of the function block shall be bound dynamically.
a) A derived function block type can be used in all places where its base function block
type can be used.
b) A derived function block type can be used in all places where its base class type can
be used.
5. SUPER() may only be called in the function block body, not in the method of a function
block
*/

impl<'db> Invocation<'db> {
    pub fn resolve_invocation(
        &self,
        db: &'db dyn BaseDatabase,
        scope: &Scope<'db>,
    ) -> ResolvedInvocationResult<'db> {
        match scope.kind {
            ScopeKind::Pou(pou) => {
                // Ensure we are in a Pou that can have and inherit methods
                match pou.pou(db) {
                    Pou::FunctionBlock { .. } | Pou::Class { .. } => {}
                    _ => {
                        return ResolvedInvocationResult {
                            target: ResolvedInvocation::new(
                                *self,
                                ResolvedMethodKind::Unresolved(match self.kind(db) {
                                    InvocationKind::This { path } => {
                                        MethodError::ThisOnIncompatiblePou {
                                            path,
                                            method: *self,
                                        }
                                    }
                                    InvocationKind::Super { path } => {
                                        MethodError::SuperOnIncompatiblePou {
                                            path,
                                            method: *self,
                                        }
                                    }
                                    InvocationKind::SuperBody => {
                                        MethodError::SuperBodyOnIncompatiblePou {
                                            ctx: pou,
                                            method: *self,
                                        }
                                    }
                                }),
                            ),
                            params: vec![],
                        };
                    }
                }

                let methods = declared_methods(db, pou);
                let len = methods.len();

                match self.kind(db) {
                    InvocationKind::This { path } => methods
                        .get(&path.ident(db))
                        .map(|method| ResolvedInvocationResult {
                            target: ResolvedInvocation::new(
                                *self,
                                ResolvedMethodKind::DeclaredMethod {
                                    target: ResolvedAccess::new(
                                        db,
                                        ResolvedPathResult::Ok(ResolvedPath {
                                            kind: ResolvedPathKind::Pou(pou),
                                            expr: CallSite::InvocationKeyword(
                                                self.keyword_id(db),
                                                *self,
                                            ),
                                            adjustement: Adjustement::None,
                                        }),
                                        vec![],
                                    ),
                                    method: *method,
                                },
                            ),
                            params: resolve_invocation_method_parameters(db, *method, *self),
                        })
                        .unwrap_or_else(|| ResolvedInvocationResult {
                            target: ResolvedInvocation::new(
                                *self,
                                ResolvedMethodKind::Unresolved(MethodError::UnresolvedThisMethod {
                                    ctx: pou,
                                    path,
                                    method: *self,
                                }),
                            ),
                            params: vec![],
                        }),
                    InvocationKind::Super { path } => inherited_methods(db, pou)
                        .methods(db)
                        .get(&path.ident(db).ident)
                        .map(|ty| ResolvedInvocationResult {
                            target: ResolvedInvocation::new(
                                *self,
                                ResolvedMethodKind::InheritedMethod {
                                    target: ResolvedAccess::new(
                                        db,
                                        ResolvedPathResult::Ok(ResolvedPath {
                                            kind: ResolvedPathKind::Pou(ty.source),
                                            expr: CallSite::InvocationKeyword(
                                                self.keyword_id(db),
                                                *self,
                                            ),
                                            adjustement: Adjustement::None,
                                        }),
                                        vec![],
                                    ),
                                    method: ty.method,
                                },
                            ),
                            params: resolve_invocation_method_parameters(db, ty.method, *self),
                        })
                        .unwrap_or_else(|| ResolvedInvocationResult {
                            target: ResolvedInvocation::new(
                                *self,
                                ResolvedMethodKind::Unresolved(
                                    MethodError::UnresolvedSuperMethod {
                                        ctx: match pou.pou(db) {
                                            Pou::Class(class) => class.extends(db).and_then(|e| {
                                                resolve_namespace_access(db, e.path)
                                            }),
                                            Pou::FunctionBlock(fb) => {
                                                fb.extends(db).and_then(|e| {
                                                    resolve_namespace_access(db, e.path)
                                                })
                                            }
                                            _ => None,
                                        },
                                        path,
                                        method: *self,
                                    },
                                ),
                            ),
                            params: vec![],
                        }),
                    InvocationKind::SuperBody => {
                        let params = resolve_invocation_func_call_parameters(db, pou, *self);
                        if let Pou::FunctionBlock { .. } = pou.pou(db) {
                            ResolvedInvocationResult {
                                target: ResolvedInvocation::new(
                                    *self,
                                    ResolvedMethodKind::FunctionBlockBody {
                                        target: ResolvedAccess::new(
                                            db,
                                            ResolvedPathResult::Ok(ResolvedPath {
                                                kind: ResolvedPathKind::Pou(pou),
                                                expr: CallSite::Invocation(*self),
                                                adjustement: Adjustement::None,
                                            }),
                                            vec![],
                                        ),
                                    },
                                ),
                                params,
                            }
                        } else {
                            ResolvedInvocationResult {
                                target: ResolvedInvocation::new(
                                    *self,
                                    ResolvedMethodKind::Unresolved(
                                        MethodError::SuperBodyOnIncompatiblePou {
                                            ctx: pou,
                                            method: *self,
                                        },
                                    ),
                                ),
                                params,
                            }
                        }
                    }
                }
            } 
            _ => unreachable!(""),
        }
    }
}

impl<'db> HirNodeInfo<'db> for ResolvedInvocation<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.invocation.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.invocation.scope_id(db)
    }
}
