use auto_lsp::{default::db::BaseDatabase};

use crate::{to_proto::AstId, ty::ty::{Ty, TyKind, TyOrigin}};

pub mod expr_resolver;
pub mod literals;
pub mod name_res;
pub mod stmt_resolver;
pub mod ty;
pub mod ty_path_expr_resolver;
pub mod ty_var_access_resolver;

pub trait TyInfo<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Option<Ty<'db>>;

    // Full Type declaration (POU, but can be variables in case of elementary types)
    fn declaration(&self, db: &'db dyn BaseDatabase) -> Option<TyOrigin<'db>> {
        match self.ty(db) {
            Some(ty) => match ty.origin(db) {
                TyOrigin::FromPou(_) | TyOrigin::FromMethod(_) => Some(ty.origin(db)),
                TyOrigin::FromVariable(_) => if let TyKind::Simple(_) = ty.kind(db) {
                    Some(ty.origin(db))
                } else {
                    None
                },
            }
            None => None,
        }
    }
    
    // Type definition (usually variables that refers to a specific type)
    fn definition(&self, db: &'db dyn BaseDatabase) -> Option<TyOrigin<'db>> {
        match self.ty(db) {
            Some(ty) => match ty.origin(db) {
                TyOrigin::FromVariable(_) => Some(ty.origin(db)),
                _ => None,
            }
            None => None,
        }
    }
    
    // Where the type is being used
    fn place(&self, db: &'db dyn BaseDatabase) -> AstId;
}
