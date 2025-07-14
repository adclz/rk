use auto_lsp::core::span::Span;

use crate::hir::expression::{Expr, Variable};

#[salsa::tracked(debug)]
pub struct Stmt<'db> {
    span: Span,

    #[return_ref]
    pub stmt: StmtKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum StmtKind<'db> {
    Assignment {
        var: Variable<'db>,
        target: Expr<'db>,
    },
    RefAssign{
        var: Variable<'db>,
        target: Expr<'db>,
    },
    AssignmentAttempt {
        var: Variable<'db>,
        target: Expr<'db>,
    },
    FuncCall{
        target: Expr<'db>, // fq_path
        params: Vec<ParamAssign>, // parameter_list
    },
    // Invocation(Invocation<'db>), // todo: must be fixed in the grammar
    Super,
    Return,
    If {
        condition: Expr<'db>,
        then: Option<Vec<Stmt<'db>>>,
        else_if: Vec<(Expr<'db>, Vec<Stmt<'db>>)>,
        else_: Option<Vec<Stmt<'db>>>,
    },
    Case{
        condition: Expr<'db>,
        cases: Vec<(Vec<Expr<'db>>, Vec<Stmt<'db>>)>,
        else_: Option<Vec<Stmt<'db>>>,
    },
    For{
        control_variable: Expr<'db>,
        start: Expr<'db>,
        end: Expr<'db>,
        step: Option<Expr<'db>>,
        body: Vec<Stmt<'db>>,
    },
    While{
        condition: Expr<'db>,
        body: Vec<Stmt<'db>>,
    },
    Repeat{
        body: Vec<Stmt<'db>>,
        condition: Expr<'db>,
    },
    Exit,
    Continue
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ParamAssign {
    ParamAssignInput,
    RefAssign,
    ParamAssignOutput,
}