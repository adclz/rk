use auto_lsp::lsp_types::{
    CompletionItem, GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent,
    MarkupKind, request::GotoDeclarationResponse,
};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::spec::{Spec, SpecKind},
    hir_ty::{name_res::resolve_namespace_access, ty::Type},
};

use crate::{
    completions::static_snippets,
    to_proto::{HasComment, ToProtocol},
};

impl<'db> ToProtocol<'db> for Spec<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, _offset: usize) -> Option<Hover> {
        let comment = match self.kind(db) {
            SpecKind::Target(t) => {
                let pou = resolve_namespace_access(db, &t.path)?;
                pou.get_comment(db).unwrap_or_default()
            }
            SpecKind::Ref(r) => r.get_comment(db).unwrap_or_default(),
            _ => Default::default(),
        };
        let desc = Type::new_spec(db, *self).full_type_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
{desc}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        match self.kind(db) {
            SpecKind::Target(target) => {
                let pou = resolve_namespace_access(db, &target.path)?;
                pou.definition(db)
            }
            SpecKind::Ref(_ref) => _ref.definition(db),
            _ => Some(GotoDefinitionResponse::Scalar(Location::new(
                self.scope_id(db).file(db).url(db).to_owned(),
                self.get_span(db).into(),
            ))),
        }
    }

    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        match self.kind(db) {
            SpecKind::Target(target) => {
                let pou = resolve_namespace_access(db, &target.path)?;
                pou.definition(db)
            }
            SpecKind::Ref(_ref) => _ref.declaration(db),
            _ => Some(GotoDeclarationResponse::Scalar(Location::new(
                self.scope_id(db).file(db).url(db).to_owned(),
                self.get_span(db).into(),
            ))),
        }
    }

    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match self.kind(db) {
            SpecKind::Struct(st) => st
                .elements(db)
                .iter()
                .map(|el| CompletionItem {
                    label: el.name(db).text(db).to_string(),
                    kind: Some(auto_lsp::lsp_types::CompletionItemKind::FIELD),
                    detail: Some(Type::new_spec(db, el.spec(db)).type_name(db).to_string()),
                    ..Default::default()
                })
                .collect::<Vec<_>>()
                .into(),
            SpecKind::Ref(r) => r.completion(db, offset),
            SpecKind::Target(target) => match resolve_namespace_access(db, &target.path) {
                Some(pou) => pou.completion(db, offset),
                None => {
                    let mut results = vec![];
                    results.extend(static_snippets::elem_type_names());

                    Some(results)
                }
            },
            _ => None,
        }
    }
}
