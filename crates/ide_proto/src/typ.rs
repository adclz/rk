use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{GotoDefinitionResponse, Hover, HoverContents, InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind, request::GotoDeclarationResponse},
};
use hir::{HirNodeInfo, hir_ty::ty::Type};

use crate::to_proto::ToProtocol;

pub(crate) trait TypeProto<'db> {
    fn inlay_hint(
        &'db self,
        _db: &'db dyn BaseDatabase,
        _parent: &dyn HirNodeInfo<'db>,
    ) -> Option<InlayHint>;

    fn hover(
        &'db self,
        _db: &'db dyn BaseDatabase,
        _offset: usize,
        _parent: &dyn HirNodeInfo<'db>,
    ) -> Option<Hover>;

    fn definition(
        &'db self,
        _db: &'db dyn BaseDatabase,
    ) -> Option<GotoDefinitionResponse>;

    fn declaration(
        &'db self,
        _db: &'db dyn BaseDatabase,
    ) -> Option<GotoDeclarationResponse>;
}

impl<'db> TypeProto<'db> for Type<'db> {
    fn inlay_hint(
        &'db self,
        db: &'db dyn BaseDatabase,
        parent: &dyn HirNodeInfo<'db>,
    ) -> Option<InlayHint> {
        match self {
            Type::Variable(var) => Some(InlayHint {
                position: parent.get_span(db).lsp().end,
                label: InlayHintLabel::String(format!(
                    ": {}",
                    Type::new_spec(db, var.spec(db)).type_name(db)
                )),
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

    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize, parent: &dyn HirNodeInfo<'db>) -> Option<Hover> {
        match self {
            Type::Variable(var) => var.hover(db, offset),
            _ => Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: {
                        format!(
                            r#"
```iecst
{}
```"#,
                            self.full_type_name(db)
                        )
                    },
                }),
                range: Some(parent.get_span(db).lsp()),
            }),
        }
    }

    fn definition(
            &'db self,
            _db: &'db dyn BaseDatabase,
        ) -> Option<GotoDefinitionResponse> {
        match self {
            Type::Variable(var) => var.definition(_db),
            _ => None,
        }
    }

    fn declaration(
            &'db self,
            _db: &'db dyn BaseDatabase,
        ) -> Option<GotoDeclarationResponse> {
        match self {
            Type::Variable(var) => var.declaration(_db),
            _ => None,
        }
    }
}
