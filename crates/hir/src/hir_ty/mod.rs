use auto_lsp::default::db::BaseDatabase;

use crate::{AstId, check::errors::sem_errors::AnalysisError, hir_ty::ty::Ty};

pub mod array_resolver;
pub mod expr_resolver;
pub mod implementation;
pub mod inheritance_solver;
pub mod init_expr_resolver;
pub mod literals;
pub mod name_res;
pub mod signature;
pub mod stmt_resolver;
pub mod ty;
pub mod ty_path_expr_resolver;
pub mod ty_var_access_resolver;

pub trait TyInfo<'db> {
    fn ty(&self, db: &'db dyn BaseDatabase) -> Result<Ty<'db>, AnalysisError<'db>>;

    // Where the type is being used
    fn place(&self, db: &'db dyn BaseDatabase) -> AstId;
}
