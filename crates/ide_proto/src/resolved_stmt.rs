use hir::hir_ty::stmt_resolver::ResolvedStmt;

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedStmt<'db> {}
