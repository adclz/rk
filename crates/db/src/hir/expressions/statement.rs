use auto_enums::auto_enum;
use auto_lsp::{core::span::Span, default::db::BaseDatabase};

use crate::{hir::{expressions::expression::{
    Expr, ParamAssign, PathExpr, SymbolicVariable, VariableAccess
}, semantic_index::SemanticIndex}, to_proto::{self_iter, IterToProto, ToProto}};

#[salsa::tracked(debug)]
pub struct Stmt<'db> {
    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub stmt: StmtKind<'db>,
}

impl<'db> ToProto<'db> for Stmt<'db> {
    fn get_span(&'db self, db: &'db dyn crate::BaseDatabase) -> &'db Span {
        self.span(db)
    }
}

impl<'db> IterToProto<'db> for Stmt<'db> {
    #[auto_enum(Iterator)]
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self.stmt(db) {
            StmtKind::Assignment { var, target } => {
                Box::new(self_iter(self)
                    .chain(var.iter(db, sema))
                    .chain(target.iter(db, sema)))
            },
            _ => std::iter::empty()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum StmtKind<'db> {
    Assignment {
        var: VariableAccess<'db>,
        target: Expr<'db>,
    },
    AssignmentAttempt {
        var: VariableAccess<'db>,
        target: Expr<'db>, // todo: replace with ref or identifier
    },
    FuncCall {
        target: PathExpr<'db>,
        params: Vec<ParamAssign<'db>>, // parameter_list
    },
    Invocation {
        target: SymbolicVariable<'db>,
        params: Vec<ParamAssign<'db>>, // parameter_list
    },
    Super,
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
        body: Vec<Stmt<'db>>,
        condition: Expr<'db>,
    },
    Exit,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum CaseKind<'db> {
    Expression(Expr<'db>),
    Subrange {
        lower: Expr<'db>,
        upper: Expr<'db>,
    }
}
