use std::fmt::format;

use ast::generated::Target;
use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{Hover, HoverContents, Location, MarkupContent, MarkupKind},
};
use hir::{
    hir_def::comment_index::comment_index, hir_ty::{ty::TyKind, ty_var_access_resolver::{ResolvedAccess}}, HirNodeInfo, TypeInfo
};

use crate::{ToProtocol};

impl<'db> ToProtocol<'db> for ResolvedAccess<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        let decl_name = self.decl_name(db);
        let decl_kind = match self.decl_kind(db) {
            s if s.is_empty() => "".to_string(),
            s => format!("({s}) "),
        };
        let def_name = "";
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
```iecst
{decl_kind}{decl_name}{def_name}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }

    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::request::GotoDeclarationResponse> {
        None
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        None
    }
}
