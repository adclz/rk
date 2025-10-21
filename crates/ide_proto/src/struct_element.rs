use auto_lsp::{default::db::BaseDatabase, lsp_types::{request::GotoDeclarationResponse, GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent, MarkupKind}};
use hir::{hir_def::expressions::spec::StructElement, hir_ty::stmt_resolver::ResolvedStmt, HirNodeInfo, TypeInfo};

use crate::{HasComment, ToProtocol};

impl<'db> ToProtocol<'db> for StructElement<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        let comment = self.get_comment(db).unwrap_or_default();
        let name = self.name(db).text(db);
        let type_name = self.spec(db).to_ty(db).type_name(db);

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

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }
}
