use auto_lsp::default::db::{BaseDatabase, File};
use crate::parser::Parse;
use crate::hir;

impl<'db> Parse<'db> for ast::generated::FuncDecl {
    type Output = hir::function::Function<'db>;

    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> Self::Output {
        let doc = file.document(db);
        hir::function::Function::new(db)
    }
}