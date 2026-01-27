use auto_lsp::lsp_types::Hover;
use db::WorkspaceDataBase;
use hir::hir_def::{namespace::NamespaceDecl, pous::{pou::Pou, variable::VariableDecl}};

use crate::handlers::HoverHandler;

impl<'db> HoverHandler<'db> for Pou<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        None
    }
}

impl<'db> HoverHandler<'db> for VariableDecl<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        None
    }
}