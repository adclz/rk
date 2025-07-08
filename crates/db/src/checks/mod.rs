use auto_lsp::default::db::File;

pub mod literals;
pub mod spec;

pub trait Check<'db> {
    fn check(&self, db: &'db dyn crate::BaseDatabase, file: File);
}