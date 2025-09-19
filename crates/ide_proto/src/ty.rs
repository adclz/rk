use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        CodeLens, GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent,
        MarkupKind,
        request::{GotoDeclarationResponse, GotoImplementationResponse},
    },
};
use hir::{
    hir_def::comment_index::comment_index,
    hir_ty::ty::{Ty, TyDef},
    {HirNodeInfo, TypeInfo},
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Ty<'db> {
    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.decl(db).scope_id(db).file(db).url(db).clone(),
            self.decl(db).span(db).into(),
        )))
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match self.def(db) {
            TyDef::Pou(pou) => Some(GotoDefinitionResponse::Scalar(Location::new(
                pou.get_scope_id(db).file(db).url(db).clone(),
                pou.get_span(db).into(),
            ))),
            TyDef::Method(method) => Some(GotoDefinitionResponse::Scalar(Location::new(
                method.get_scope_id(db).file(db).url(db).clone(),
                method.get_span(db).into(),
            ))),
            TyDef::MethodProt(method) => Some(GotoDefinitionResponse::Scalar(Location::new(
                method.get_scope_id(db).file(db).url(db).clone(),
                method.get_span(db).into(),
            ))),
            TyDef::Spec(spec) => Some(GotoDefinitionResponse::Scalar(Location::new(
                spec.scope_id(db).file(db).url(db).clone(),
                spec.get_span(db).into(),
            ))),
            TyDef::Invalid => None,
        }
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: Option<usize>) -> Option<Hover> {
        let name_span = self.decl(db).name_span(db);

        // We need to check if the cursor is over the name of the type
        if let Some(offset) = offset {
            if name_span.start_byte > offset || name_span.end_byte < offset {
                return None;
            }
        }

        let type_name = self.type_name(db);
        let name = self.decl(db).name(db).text(db).to_string();

        let comment = match comment_index(db, self.get_scope_id(db).file(db)).find_nearby_comment(
            self.get_scope_id(db).file(db).document(db),
            &self.get_span(db),
        ) {
            Some(c) => c.to_string(self.get_scope_id(db).file(db).document(db)),
            None => "".to_string(),
        };

        Some(auto_lsp::lsp_types::Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{type_name} {name}
```
"#
                ),
            }),
            range: Some(name_span.into()),
        })
    }

    fn code_lens(&self, db: &'db dyn BaseDatabase) -> Option<CodeLens> {
        match self.def(db) {
            TyDef::Pou(pou) => pou.code_lens(db),
            _ => None,
        }
    }

    fn implementation(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoImplementationResponse> {
        match self.def(db) {
            TyDef::Pou(pou) => pou.implementation(db),
            _ => None,
        }
    }
}
