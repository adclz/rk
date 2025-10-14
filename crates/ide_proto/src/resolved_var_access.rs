use std::fmt::format;

use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{Hover, HoverContents, Location, MarkupContent, MarkupKind},
};
use hir::{
    hir_def::comment_index::comment_index, hir_ty::{ty::TyKind, ty_var_access_resolver::{Place, ResolvedAccess}}, HirNodeInfo, TypeInfo
};

use crate::{ToProtocol, ty::TyHover};

impl<'db> ToProtocol<'db> for ResolvedAccess<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        let decl_name = self.decl_name(db);
        let decl_kind = match self.decl_kind(db) {
            s if s.is_empty() => "".to_string(),
            s => format!("({s}) "),
        };
        let def_name = self
            .ty(db)
            .ok()
            .and_then(|s| Some(match s.has_return_type(db) {
                Some(ret) => format!(": {}", ret.type_name(db)),
                None => match s.kind(db) {
                    TyKind::RefTo(ref_) => format!(": REF_TO {}", ref_.spec_to_ty(db).type_name(db)),
                    _ => format!(": {}", s.type_name(db)),
                }
            }))
            .unwrap_or_default();
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
        match self.kind(db) {
            Place::Symbolic { target, .. } => Some(
                auto_lsp::lsp_types::request::GotoDeclarationResponse::Scalar(Location::new(
                    target.get_scope_id(db).file(db).url(db).to_owned(),
                    target.get_span(db).into(),
                )),
            ),
            _ => None,
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        self.ty(db).ok().and_then(|ty| ty.definition(db))
    }
}
