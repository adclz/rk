use auto_lsp::default::db::{BaseDatabase, File};
use crate::parser::Parse;
use crate::hir;

impl<'db> Parse<'db> for ast::generated::ClassDecl {
    type Output = hir::class::Class<'db>;

    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> Self::Output {
        let doc = file.document(db);
        hir::class::Class::new(db)
    }
}