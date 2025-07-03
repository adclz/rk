use auto_lsp::anyhow;
use auto_lsp::default::db::{BaseDatabase, File};
use crate::hir::expression::Expr;
use crate::parser::Parse;
use crate::hir;

impl<'db> Parse<'db> for ast::generated::InterfaceDecl {
    type Output = hir::interface::Interface<'db>;

    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let extends = self
            .extends
            .as_ref()
            .map(|i| i.children.iter().map(|i| Expr::new_target(db, file, i)).collect())
            .transpose()?;
 
        Ok(hir::interface::Interface::new(db, extends))
    }
}
