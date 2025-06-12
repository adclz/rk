use auto_lsp::default::db::File;
use salsa::Update;

use crate::BaseDatabase;

pub mod namespace;
pub mod function;
pub mod function_block;
pub mod class;

trait Parse<'db>: Sized {
    type Output: Update;

    fn parse(&self, db: &'db dyn BaseDatabase, file: File) -> Self::Output;
}
