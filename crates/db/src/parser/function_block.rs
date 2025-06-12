use auto_lsp::default::db::{BaseDatabase, File};
use crate::parser::Parse;
use crate::hir;

impl<'db> Parse<'db> for ast::generated::FbDecl {
    type Output = hir::function_block::FunctionBlock<'db>;

    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> Self::Output {
        let doc = file.document(db);
        hir::function_block::FunctionBlock::new(db)
    }
}