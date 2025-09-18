use crate::{
    hir_def::{
        expressions::expression::{InitExprKind, Integer},
        interned::identifier::SpanIdent,
        scope::FileScopeId,
    },
    hir_ty::expr_resolver::{ResolvedExpr, resolve_expr},
    to_proto::{AstId, ToProto},
};
use auto_lsp::{default::db::BaseDatabase, lsp_types::request::GotoDeclarationResponse};

use crate::hir_def::expressions::expression::InitExpr;

#[salsa::tracked(no_eq, returns(ref))]
pub fn resolve_init_expr<'db>(
    db: &'db dyn BaseDatabase,
    expr: InitExpr<'db>,
) -> ResolvedInitExpr<'db> {
    let kind = match expr.kind(db) {
        InitExprKind::ArrayInit { values } => {
            let resolved = values.iter().map(|v| *resolve_init_expr(db, *v)).collect();
            ResolvedInitExprKind::ArrayInit { values: resolved }
        }
        InitExprKind::ArrayIndexedElement { index, values } => {
            let resolved = values.iter().map(|v| *resolve_init_expr(db, *v)).collect();
            ResolvedInitExprKind::ArrayIndexedElement {
                index,
                values: resolved,
            }
        }
        InitExprKind::StructInit { values } => {
            let resolved = values.iter().map(|v| *resolve_init_expr(db, *v)).collect();
            ResolvedInitExprKind::StructInit { values: resolved }
        }
        InitExprKind::StructElement { name, value } => {
            let resolved = Box::new(*resolve_init_expr(db, *value));
            ResolvedInitExprKind::StructElement {
                name: name.clone(),
                value: resolved,
            }
        }
        InitExprKind::ConstantExpr(expr) => {
            ResolvedInitExprKind::ConstantExpr(*resolve_expr(db, expr))
        }
    };
    ResolvedInitExpr::new(db, expr, kind)
}

#[salsa::tracked(debug)]
pub struct ResolvedInitExpr<'db> {
    pub expr: InitExpr<'db>,
    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub kind: ResolvedInitExprKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum ResolvedInitExprKind<'db> {
    ArrayInit {
        values: Vec<ResolvedInitExpr<'db>>,
    },
    ArrayIndexedElement {
        index: Integer,
        values: Vec<ResolvedInitExpr<'db>>,
    },
    StructInit {
        values: Vec<ResolvedInitExpr<'db>>,
    },
    StructElement {
        name: SpanIdent<'db>,
        value: Box<ResolvedInitExpr<'db>>,
    },
    ConstantExpr(ResolvedExpr<'db>),
}

impl<'db> ToProto<'db> for ResolvedInitExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.expr(db).id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.expr(db).scope_id(db)
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        if let InitExprKind::ConstantExpr(expr) = self.expr(db).kind(db) {
            expr.declaration(db)
        } else {
            None
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        if let InitExprKind::ConstantExpr(expr) = self.expr(db).kind(db) {
            expr.definition(db)
        } else {
            None
        }
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: Option<usize>,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        if let InitExprKind::ConstantExpr(expr) = self.expr(db).kind(db) {
            expr.hover(db, None)
        } else {
            None
        }
    }
}
