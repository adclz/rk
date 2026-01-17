use db::WorkspaceDataBase;

use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::expression::{BeginPathExpr, Expr, FuncCall, VariableAccess},
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Stmt<'db> {
    #[returns(ref)]
    pub stmt: StmtKind<'db>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for Stmt<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum StmtKind<'db> {
    EmptyPathExpression(BeginPathExpr<'db>),
    Assignment {
        var: VariableAccess<'db>,
        target: Expr<'db>,
    },
    AssignmentAttempt {
        var: VariableAccess<'db>,
        target: Expr<'db>, // todo: replace with ref or identifier
    },
    FuncCall(FuncCall<'db>),
    Return,
    If {
        condition: Expr<'db>,
        then: Option<Vec<Stmt<'db>>>,
        else_if: Vec<(Expr<'db>, Vec<Stmt<'db>>)>,
        else_: Option<Vec<Stmt<'db>>>,
    },
    Case {
        condition: Expr<'db>,
        cases: Vec<(Vec<CaseKind<'db>>, Vec<Stmt<'db>>)>,
        else_: Option<Vec<Stmt<'db>>>,
    },
    For {
        control_variable: VariableAccess<'db>,
        start: Expr<'db>,
        end: Expr<'db>,
        step: Option<Expr<'db>>,
        body: Vec<Stmt<'db>>,
    },
    While {
        condition: Expr<'db>,
        body: Vec<Stmt<'db>>,
    },
    Repeat {
        condition: Expr<'db>,
        body: Vec<Stmt<'db>>,
    },
    Exit,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum CaseKind<'db> {
    Expression(Expr<'db>),
    Subrange { lower: Expr<'db>, upper: Expr<'db> },
}
