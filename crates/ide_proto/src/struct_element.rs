use auto_lsp::lsp_types::{
    GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent, MarkupKind,
    request::GotoDeclarationResponse,
};
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::spec::StructElement, hir_ty::ty::Type};

use crate::to_proto::{HasComment, ToProtocol};

impl<'db> ToProtocol<'db> for StructElement<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, _offset: usize) -> Option<Hover> {
        let comment = self.get_comment(db).unwrap_or_default();
        let name = self.name(db).text(db);
        let type_name = Type::new_spec(db, self.spec(db)).type_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{name}: {type_name}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }

    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}
