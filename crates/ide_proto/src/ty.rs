use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        CodeLens, GotoDefinitionResponse, Hover, HoverContents, InlayHint, Location, MarkupContent,
        MarkupKind,
        request::{GotoDeclarationResponse, GotoImplementationResponse},
    },
};
use hir::{
    HirNodeInfo, TypeInfo,
    hir_def::comment_index::comment_index,
    hir_ty::ty::{Ty, TyDef, TyKind},
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Ty<'db> {
    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match self.def(db) {
            TyDef::Pou(pou) => Some(GotoDefinitionResponse::Scalar(Location::new(
                pou.get_scope_id(db).file(db).url(db).clone(),
                pou.get_span(db).into(),
            ))),
            TyDef::MethodRef(method) => Some(GotoDefinitionResponse::Scalar(Location::new(
                method.get_scope_id(db).file(db).url(db).clone(),
                method.get_span(db).into(),
            ))),
            TyDef::Spec(spec) => Some(GotoDefinitionResponse::Scalar(Location::new(
                spec.scope_id(db).file(db).url(db).clone(),
                spec.get_span(db).into(),
            ))),
        }
    }

    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        match self.def(db) {
            TyDef::Pou(pou) => pou.inlay_hint(db),
            _ => None,
        }
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        self.force_hover(db)
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

pub trait TyHover<'ty> {
    /// Force a hover for this type, even if the offset is not on the type name.
    /// This is mostly used by resolved structs in statements and expressions.
    fn force_hover(&self, db: &'ty dyn BaseDatabase) -> Option<Hover>;
    fn get_comment(&self, db: &'ty dyn BaseDatabase) -> String;
}

impl<'ty> TyHover<'ty> for Ty<'ty> {
    fn force_hover(&self, db: &'ty dyn BaseDatabase) -> Option<Hover> {
        let name = self.name(db);
        let comment = self.get_comment(db);
        let def_name = match self.has_return_type(db) {
            Some(ret) => format!(": {}", ret.type_name(db).to_string()),
            None => match self.kind(db) {
                TyKind::RefTo(ref_) => format!(": REF_TO {}", ref_.spec_to_ty(db).type_name(db).to_string()),
                _ => "".to_string(),
            },
        };
        let value = format!(
            r#"
{comment}
```iecst
{name}{def_name}
```
"#
        );

        Some(auto_lsp::lsp_types::Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(self.name_span(db).into()),
        })
    }

    fn get_comment(&self, db: &'ty dyn BaseDatabase) -> String {
        let comment = match comment_index(db, self.get_scope_id(db).file(db)).find_nearby_comment(
            self.get_scope_id(db).file(db).document(db),
            &self.get_span(db),
        ) {
            Some(c) => c.to_string(self.get_scope_id(db).file(db).document(db)),
            None => "".to_string(),
        };
        comment
    }
}
