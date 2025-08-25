use auto_lsp::default::db::BaseDatabase;

use crate::ty::ty::Ty;

pub mod expr_resolver;
pub mod name_res;
pub mod stmt_resolver;
pub mod ty;
pub mod ty_path_expr_resolver;
pub mod ty_var_access_resolver;

pub trait TyResolved<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>>;
}
