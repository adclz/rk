use ast::generated::{PrimaryExpression, Subrange};
use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::{IdeDiagnostic, diag};
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    builder::interface,
    check::errors::{
        analysis_error::ToIdeDiagnostic, body_inference::BodyInferenceError,
        path_error::AccessError,
    },
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, ExprKind, FuncCall, InitExpr, ParamAssign, ParamAssignKind,
                PathExpr, PathExprKind, PrimaryExpr, VarAccess, VariableAccess, VariableAccessKind,
            },
            invocation::{self, Invocation, InvocationKind},
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespaceAccess, SpanNamespacePath},
        },
        pous::{
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::{HirNode, get_scope},
    },
    hir_ty::{
        infer::ctx::{InferCtx, NestedScope},
        resolver::Resolver,
        ty::Type,
    },
};

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn infer_body_scope<'db>(
    db: &'db dyn BaseDatabase,
    scope: ScopeId<'db>,
) -> BodyInferenceResult<'db> {
    let mut result = BodyInferenceResult::new(scope);
    let ctx = InferCtx::new(scope);

    // Only Scopes with bodies can have statements
    let (scope_typ, statements) = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => match pou.pou(db) {
            Pou::Function(f) => (Type::Function(*f), f.statements(db)),
            Pou::FunctionBlock(fb) => (Type::FunctionBlock(*fb), fb.statements(db)),
            _ => return result,
        },
        ScopeKind::MethodDecl(m) => (Type::MethodDecl(m.into()), m.stmts(db)),
        _ => return result,
    };

    ctx.resolve_statements(
        db,
        Resolver::new(Some(scope_typ)),
        statements,
        NestedScope::None,
        &mut result,
    );

    result
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct BodyInferenceResult<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    // Mapping from parameter assignments to variables
    pub variable_of_param: FxHashMap<ParamAssign<'db>, VariableDecl<'db>>,

    // Mapping from invocations to their resolved types.
    pub type_of_invocation: FxHashMap<Invocation<'db>, Type<'db>>,

    // Mapping from path expressions to their resolved types.
    pub type_of_path_expr: FxHashMap<PathExpr<'db>, Type<'db>>,

    // Mapping from expressions to their resolved types.
    pub type_of_expr: FxHashMap<Expr<'db>, Type<'db>>,

    // Mapping from path expressions to their adjustment sequences.
    pub path_expr_adjustments: FxHashMap<PathExpr<'db>, Vec<Adjustment<'db>>>,

    // Errors encountered during inference
    pub errors: Vec<BodyInferenceError<'db>>,
}

impl<'db> BodyInferenceResult<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            variable_of_param: FxHashMap::default(),
            type_of_invocation: FxHashMap::default(),
            type_of_expr: FxHashMap::default(),
            type_of_path_expr: FxHashMap::default(),
            path_expr_adjustments: FxHashMap::default(),
            errors: Vec::new(),
        }
    }

    pub fn type_of_expr_with_adjustments(
        &self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
    ) -> Option<Type<'db>> {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(var)) => match var.kind(db) {
                VariableAccessKind::Symbolic(sym) => {
                    self.type_of_begin_expr_with_adjustments(db, sym)
                }
                _ => self.type_of_expr.get(&expr).copied(),
            },
            _ => self.type_of_expr.get(&expr).copied(),
        }
    }

    pub fn type_of_variable_access_with_adjustments(
        &self,
        db: &'db dyn BaseDatabase,
        var_access: VariableAccess<'db>,
    ) -> Option<Type<'db>> {
        match var_access.kind(db) {
            VariableAccessKind::Symbolic(sym) => self.type_of_begin_expr_with_adjustments(db, sym),
            _ => None,
        }
    }

    pub fn type_of_begin_expr_with_adjustments(
        &self,
        db: &'db dyn BaseDatabase,
        begin: BeginPathExpr<'db>,
    ) -> Option<Type<'db>> {
        match begin.expr(db) {
            Some(expr) => self.type_of_path_expr_with_adjustments(expr),
            None => match begin.invocation(db) {
                Some(invocation) => self.type_of_invocation.get(&invocation).copied(),
                None => None,
            },
        }
    }

    pub fn type_of_path_expr_with_adjustments(&self, expr: PathExpr<'db>) -> Option<Type<'db>> {
        match self
            .path_expr_adjustments
            .get(&expr)
            .and_then(|adjustements| adjustements.last())
        {
            Some(adjustment) => Some(adjustment.target),
            None => self.type_of_path_expr.get(&expr).copied(),
        }
    }

    pub fn variable_for_param(&self, param: ParamAssign<'db>) -> Option<VariableDecl<'db>> {
        self.variable_of_param.get(&param).copied()
    }

    pub fn path_expr_adjustments(&self, expr: PathExpr<'db>) -> Option<&[Adjustment<'db>]> {
        self.path_expr_adjustments.get(&expr).map(|it| &**it)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Adjust {
    Deref,
    Ref,
    Index,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Adjustment<'db> {
    pub kind: Adjust,
    pub target: Type<'db>,
}

impl<'db> Adjustment<'db> {
    pub fn new_deref(db: &'db dyn BaseDatabase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Deref,
            target: ty,
        }
    }

    pub fn new_ref(db: &'db dyn BaseDatabase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Ref,
            target: ty,
        }
    }

    pub fn new_index(db: &'db dyn BaseDatabase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Index,
            target: ty,
        }
    }
}

/// Flattened representation of a path expression.
/// Each step in the path is represented as a [`PathExprWalkStep`].
///
/// This is later used by type resolution to walk trough a path expression step by step.
#[derive(Debug, Copy, Clone, PartialEq, Eq, salsa::Update)]
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
    pub fn flatten(self, db: &'db dyn BaseDatabase) -> Vec<PathExprWalkStep<'db>> {
        let mut result = Vec::new();

        match self.expr(db) {
            PathExprKind::Field(field_expr) => {
                result.extend(field_expr.path.flatten(db).iter().cloned());
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
                result.extend(index_expr.path.flatten(db).iter().cloned());
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

    #[salsa::tracked(returns(ref))]
    pub fn to_namespace_access(
        self,
        db: &'db dyn BaseDatabase,
    ) -> Option<(NamespaceAccess<'db>, Ident)> {
        let flatten = self.flatten(db);

        // Collect FIELD.FIELD.FIELD prefix
        let mut frags = Vec::new();

        for step in flatten.iter().rev() {
            match step {
                PathExprWalkStep::Field { ident, .. } => frags.push(*ident),
                _ => break,
            }
        }

        if frags.is_empty() {
            return None;
        }

        let first = frags.remove(0);

        let scope = self.scope_id(db);
        let access = NamespaceAccess::new(
            db,
            match frags.len() {
                0 => None,
                _ => Some(SpanNamespacePath::from((db, &frags, scope))),
            },
            first,
        );

        Some((access, *first)) // returning `first` is optional
    }
}
