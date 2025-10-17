use hir::{hir_def::expressions::spec::Spec, hir_ty::stmt_resolver::ResolvedStmt};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Spec<'db> {}
