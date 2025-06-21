use auto_lsp::anyhow;
use auto_lsp::default::db::{BaseDatabase, File};
use crate::parser::Parse;
use crate::hir;

impl<'db> Parse<'db> for ast::generated::ClassDecl {
    type Output = hir::class::Class<'db>;

    fn parse(&self, db: &'db dyn BaseDatabase, _file: File) -> anyhow::Result<Self::Output> {
        Ok(hir::class::Class::new(db))
    }
}