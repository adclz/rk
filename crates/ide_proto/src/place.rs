use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        Hover, HoverContents, MarkupContent, MarkupKind, request::GotoDeclarationResponse,
    },
};
use hir::{
    hir_def::comment_index::comment_index, hir_ty::ty_var_access_resolver::{Place, PlaceKind}, HirNodeInfo, TypeInfo
};


pub(crate) trait PlaceInfo<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover>;
    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse>;
    fn get_comment(&'db self, db: &'db dyn BaseDatabase) -> Option<String>;
}

impl<'db> PlaceInfo<'db> for PlaceKind<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        let decl_name = self.place_name(db);
        let decl_kind = match self.place_kind(db) {
            s if s.is_empty() => "".to_string(),
            s => format!("({s}) "),
        };
        let comment = self.get_comment(db).unwrap_or_default();
        let def_name = self
            .ty(db)
            .and_then(|s| {
                Some(match s.has_return_type(db) {
                    Some(ret) => format!(": {}", ret.type_name(db)),
                    None => format!(": {}", s.type_name(db)),
                })
            })
            .unwrap_or_default();
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{decl_kind}{decl_name}{def_name}
```
                "#
                ),
            }),
            range: Some(self.get_span(db)?.into()),
        })
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        let location = match self {
            PlaceKind::ReferenceToVariable((var, _)) => (
                var.get_scope_id(db).file(db).url(db),
                var.get_span(db).into(),
            ),
            PlaceKind::ReferenceToField((field, _)) => (
                field.get_scope_id(db).file(db).url(db),
                field.get_span(db).into(),
            ),
            PlaceKind::Variable(var) => (
                var.get_scope_id(db).file(db).url(db),
                var.get_span(db).into(),
            ),
            PlaceKind::StructField(field) => (
                field.get_scope_id(db).file(db).url(db),
                field.get_span(db).into(),
            ),
            _ => return None,
        };

        Some(GotoDeclarationResponse::Scalar(
            auto_lsp::lsp_types::Location::new(location.0.to_owned(), location.1),
        ))
    }

    fn get_comment(&self, db: &'db dyn BaseDatabase) -> Option<String> {
        let comment = match comment_index(db, self.get_scope_id(db)?.file(db)).find_nearby_comment(
            self.get_scope_id(db)?.file(db).document(db),
            &*self.get_span(db)?,
        ) {
            Some(c) => c.to_string(self.get_scope_id(db)?.file(db).document(db)),
            None => "".to_string(),
        };
        Some(comment)
    }
}
