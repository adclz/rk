use auto_lsp::default::db::BaseDatabase;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, ExprKind, ParamAssign, PathExpr, PrimaryExpr, RefValue,
                VariableAccess, VariableAccessKind,
            },
            invocation::Invocation,
        },
        pous::{
            pou::Pou,
            variable::{DirectVariable, VariableDecl},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        resolver::{
            Resolver,
            body::{InferenceCtx, NestedScope},
        },
        ty::Type,
    },
};

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn infer_body_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
) -> BodyInferenceResult<'db> {
    let mut result = BodyInferenceResult::new(scope);
    let ctx = InferenceCtx::new(scope);

    // Only Scopes with bodies can have statements
    let statements = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => match pou {
            Pou::Function(f) => f.statements(db),
            Pou::FunctionBlock(fb) => fb.statements(db),
            _ => return result,
        },
        ScopeKind::MethodDecl(m) => m.stmts(db),
        ScopeKind::Program(program) => program.statements(db),
        _ => return result,
    };

    ctx.check_statements(
        db,
        Resolver::for_scope(db, scope),
        statements,
        NestedScope::None,
        &mut result,
    );

    result
}

/// Result of body inference
///
/// When the this struct is emitted via the [`infer_body_scope`] query, it is important to note 2 things about the type mappings:
///
/// 1. The types mapped to expressions and invocations are the types *before* any normalization or adjustments are applied.
/// see the note on normalization in the normalize module.
///
/// 2. There should be no [`Type::Infer`] types in the mappings. All types should be fully resolved,
/// those that can't be resolved will be represented as [`Type::Never`].
#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct BodyInferenceResult<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    // Mapping from parameter assignments to variables
    pub variable_of_param: FxHashMap<ParamAssign<'db>, VariableDecl<'db>>,

    // Mapping of direct variables to their types
    pub type_of_direct_variable: FxHashMap<DirectVariable<'db>, Type<'db>>,

    // Mapping from invocations to their resolved types.
    pub type_of_invocation: FxHashMap<Invocation<'db>, Type<'db>>,

    // Mapping from path expressions to their resolved types.
    pub type_of_path_expr: FxHashMap<PathExpr<'db>, Type<'db>>,

    // Mapping from expressions to their resolved types.
    pub type_of_expr: FxHashMap<Expr<'db>, Type<'db>>,

    // Mapping from path expressions to their adjustment sequences.
    pub path_expr_adjustments: FxHashMap<PathExpr<'db>, Vec<Adjustment<'db>>>,

    // Errors encountered during inference
    pub errors: Vec<IdeDiagnostic>,
}

impl<'db> BodyInferenceResult<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        Self {
            scope,
            variable_of_param: FxHashMap::default(),
            type_of_direct_variable: FxHashMap::default(),
            type_of_invocation: FxHashMap::default(),
            type_of_expr: FxHashMap::default(),
            type_of_path_expr: FxHashMap::default(),
            path_expr_adjustments: FxHashMap::default(),
            errors: Vec::new(),
        }
    }

    pub fn get_type_of_expr(&self, expr: Expr<'db>) -> Option<Type<'db>> {
        self.type_of_expr.get(&expr).copied()
    }

    pub fn type_of_expr_with_adjustments(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
    ) -> Option<Type<'db>> {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(var)) => {
                self.type_of_variable_access_with_adjustments(db, *var)
            }
            _ => self.type_of_expr.get(&expr).copied(),
        }
    }

    pub fn type_of_variable_access_with_adjustments(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
    ) -> Option<Type<'db>> {
        match var_access.kind(db) {
            VariableAccessKind::Symbolic(sym) => self.type_of_begin_expr_with_adjustments(db, sym),
            VariableAccessKind::Direct(dv) => self.type_of_direct_variable.get(&dv).copied(),
        }
    }

    pub fn get_type_of_variable_access(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var_access: VariableAccess<'db>,
    ) -> Option<Type<'db>> {
        match var_access.kind(db) {
            VariableAccessKind::Symbolic(sym) => self.get_type_of_begin_path_expr(db, sym),
            VariableAccessKind::Direct(dv) => self.type_of_direct_variable.get(&dv).copied(),
        }
    }

    pub fn type_of_begin_expr_with_adjustments(
        &self,
        db: &'db dyn WorkspaceDataBase,
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

    pub fn get_type_of_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        begin: BeginPathExpr<'db>,
    ) -> Option<Type<'db>> {
        match begin.expr(db) {
            Some(expr) => self.get_type_of_path_expr(db, expr),
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

    pub fn get_type_of_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: PathExpr<'db>,
    ) -> Option<Type<'db>> {
        self.type_of_path_expr.get(&expr).copied()
    }

    pub fn adjustments_of_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        expr: Expr<'db>,
    ) -> Option<&[Adjustment<'db>]> {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(var)) => {
                self.adjustments_of_var_access(db, *var)
            }
            ExprKind::PrimaryExpr(PrimaryExpr::RefValue { value }) => match value {
                RefValue::Address(address) => self.adjustments_of_begin_path_expr(db, *address),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn adjustments_of_var_access(
        &self,
        db: &'db dyn WorkspaceDataBase,
        var: VariableAccess<'db>,
    ) -> Option<&[Adjustment<'db>]> {
        match var.kind(db) {
            VariableAccessKind::Symbolic(sym) => self.adjustments_of_begin_path_expr(db, sym),
            _ => None,
        }
    }

    pub fn adjustments_of_begin_path_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        begin: BeginPathExpr<'db>,
    ) -> Option<&[Adjustment<'db>]> {
        match begin.expr(db) {
            Some(expr) => self.adjustments_of_path_expr(expr),
            None => None,
        }
    }

    pub fn adjustments_of_path_expr(&self, expr: PathExpr<'db>) -> Option<&[Adjustment<'db>]> {
        self.path_expr_adjustments.get(&expr).map(|it| &**it)
    }

    pub fn variable_for_param(&self, param: ParamAssign<'db>) -> Option<VariableDecl<'db>> {
        self.variable_of_param.get(&param).copied()
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
    pub fn new_deref(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Deref,
            target: ty,
        }
    }

    pub fn new_index(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Index,
            target: ty,
        }
    }

    pub fn new_ref(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Ref,
            target: ty,
        }
    }
}
