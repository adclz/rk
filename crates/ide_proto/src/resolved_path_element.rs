use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{Hover, HoverContents, InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind, Position},
};
use hir::{
    hir_ty::{
        ty::{Ty, TyKind}, ty_var_access_resolver::{ResolvedPathElement, ResolvedPathElementKind},
    }, HirNodeInfo, TypeInfo
};

use crate::{ToProtocol, ty::TyHover};

impl<'db> ToProtocol<'db> for ResolvedPathElement<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        match self.kind {
            ResolvedPathElementKind::Ty(ty) => {
                let decl_name = self.expr.ident(db).text(db).to_string();
                let def_name = match ty.has_return_type(db) {
                    Some(ret) => format!(": {}", ret.type_name(db)),
                    None => format!(": {}", ty.type_name(db))
                };
                      Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
```iecst
{decl_name}{def_name}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })

            }
            _ => None,
        }
    }

    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::request::GotoDeclarationResponse> {
        match self.kind {
            ResolvedPathElementKind::Ty(ty) => ty.declaration(db),
            _ => None,
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        match self.kind {
            ResolvedPathElementKind::Ty(ty) => ty.definition(db),
            _ => None,
        }
    }
}
