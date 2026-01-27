use auto_lsp::lsp_types::InlayHint;
use db::WorkspaceDataBase;
use hir::hir_def::{expressions::expression::{InitExpr, ParamAssign}, namespace::NamespaceDecl, pous::pou::Pou};

use crate::handlers::InlayHintHandler;

impl<'db> InlayHintHandler<'db> for NamespaceDecl<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        None
    }
}

impl<'db> InlayHintHandler<'db> for Pou<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        None
    }
}

impl<'db> InlayHintHandler<'db> for ParamAssign<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        None
    }
}

impl<'db> InlayHintHandler<'db> for InitExpr<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        None
    }
}