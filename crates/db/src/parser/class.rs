use crate::hir;
use crate::hir::expression::Expr;
use crate::parser::Parse;
use auto_lsp::anyhow;
use auto_lsp::default::db::{BaseDatabase, File};

impl<'db> Parse<'db> for ast::generated::ClassDecl {
    type Output = hir::class::Class<'db>;

    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> anyhow::Result<Self::Output> {
        let extends = self
            .extends
            .as_ref()
            .map(|e| Expr::new_target(db, file, e))
            .transpose()?;

        let implements = self
            .implements
            .as_ref()
            .map(|i| i.children.iter().map(|i| Expr::new_target(db, file, i)).collect())
            .transpose()?;

        Ok(hir::class::Class::new(db, extends, implements))
    }
}
