use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover, request::GotoDeclarationResponse,
    },
};
use db::WorkspaceDataBase;
use hir::{
    hir_def::expressions::expression::VariableAccess,
    hir_ty::body_inference::infer_body_scope,
};

use crate::{to_proto::ToProtocol, typ::TypeProto};

impl<'db> ToProtocol<'db> for VariableAccess<'db> {
    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
            .and_then(|typ| typ.declaration(db))
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
            .and_then(|typ| typ.definition(db))
    }

    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let infer = infer_body_scope(db, self.scope_id(db));
        infer
            .type_of_variable_access_with_adjustments(db, *self)
            .and_then(|typ| typ.hover(db, offset, self))
    }
}
