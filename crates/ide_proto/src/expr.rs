use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use hir::{
    hir_def::expressions::expression::{Expr, ExprKind, PrimaryExpr, RefValue},
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Expr<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        None
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        None
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        None
    }
}
