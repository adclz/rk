use auto_lsp::{default::db::BaseDatabase};

use crate::{to_proto::AstId, ty::ty::{Ty, TyKind, TyDecl}};

pub mod expr_resolver;
pub mod literals;
pub mod name_res;
pub mod stmt_resolver;
pub mod ty;
pub mod ty_path_expr_resolver;
pub mod ty_var_access_resolver;

pub trait TyInfo<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>>;
    
    // Where the type is being used
    fn place(&self, db: &'db dyn BaseDatabase) -> AstId;
}
