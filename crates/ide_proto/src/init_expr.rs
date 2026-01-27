use auto_lsp::lsp_types::{
    GotoDefinitionResponse, InlayHint, InlayHintKind, InlayHintLabel,
    request::GotoDeclarationResponse,
};
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::expression::InitExprKind};

use crate::{
    hir_node::InitExprWithTypeContext,
    to_proto::{ToProtocol},
    typ::TypeProto,
};

impl<'db> ToProtocol<'db> for InitExprWithTypeContext<'db> {
    fn inlay_hint(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<InlayHint> {
        match self.init_expr.kind(db) {
            InitExprKind::StructElement { name, value: _ } => Some(InlayHint {
                position: name.get_span(db).lsp().end,
                label: InlayHintLabel::String(format!(": {}", self.ty.type_name(db))),
                kind: Some(InlayHintKind::TYPE),
                padding_left: Some(false),
                padding_right: Some(false),
                text_edits: None,
                tooltip: None,
                data: None,
            }),
            _ => None,
        }
    }

    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        self.ty.declaration(db)
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.ty.definition(db)
    }

    fn hover(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        self.ty.hover(db, offset, &self.init_expr)
    }
}
