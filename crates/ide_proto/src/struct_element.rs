use hir::{hir_def::expressions::spec::StructElement, hir_ty::stmt_resolver::ResolvedStmt};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for StructElement<'db> {}
