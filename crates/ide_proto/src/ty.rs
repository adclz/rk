use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        request::{GotoDeclarationResponse, GotoImplementationResponse}, CodeLens, GotoDefinitionResponse, Hover, HoverContents, InlayHint, Location, MarkupContent, MarkupKind
    },
};
use hir::{
    HirNodeInfo, TypeInfo,
    hir_def::comment_index::comment_index,
    hir_ty::ty::{Ty, TyDef, TyKind},
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
        if let TyKind::Target(target) = self.kind(db) {
            return target.definition(db);
        }
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

    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        if let TyKind::Target(_) = self.kind(db) {
            // don't show inlay hints for target types
            return None;
        }
        match self.def(db) {
            TyDef::Pou(pou) => pou.inlay_hint(db),
            _ => None,
        }
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        let name_span = self.decl(db).name_span(db);

        // Check if the offset is within the span of the whole type
        if offset < name_span.start_byte || offset > name_span.end_byte {
            // type name not hovered
            // if target type, hover the target type
            if let TyKind::Target(target) = self.kind(db) {
                return target.force_hover(db);
            }
            // we check if we're not hovering the definition of this type (could be a Pou body)
            match self.def(db).def_as_ty(db) {
                Some(ty) => {
                    // we are hovering the definition but no the name, therefore return None
                    if ty == *self {
                        return None;
                    }
                    return ty.force_hover(db);
                },
                None => return None,
            }
        }

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
        let name_span = self.decl(db).name_span(db);
        let name = self.decl(db).name(db).text(db).to_string();
        let comment = self.get_comment(db);
        let decl = match self.decl_name(db) {
            s if s.is_empty() => "".to_string(),
            s => format!("({s}) "),
        };
        let def_name = match self.kind(db) {
            TyKind::Target(target) => target.decl(db).name(db).text(db).to_string(),
            _ => self.type_name(db).to_string(),
        };
        let value = format!(
            r#"
{comment}
```iecst
{decl}{name}: {def_name}
```
"#
        );

        Some(auto_lsp::lsp_types::Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(name_span.into()),
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
