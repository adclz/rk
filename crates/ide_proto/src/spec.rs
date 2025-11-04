use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent,
        MarkupKind, request::GotoDeclarationResponse,
    },
};
use hir::{
    HirNodeInfo, TypeInfo,
    hir_def::expressions::spec::{Spec, SpecKind},
    hir_ty::name_res::{global_pou_index, resolve_namespace_access},
};

use crate::{
    HasComment, ToProtocol,
    completions::{
        per_scope::{simple_pou_completion, scoped_completions},
        static_snippets,
    },
};

impl<'db> ToProtocol<'db> for Spec<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        let comment = match self.kind(db) {
            SpecKind::Target(t) => {
                let pou = resolve_namespace_access(db, &t.path)?;
                pou.get_comment(db).unwrap_or_default()
            }
            SpecKind::Ref(r) => r.get_comment(db).unwrap_or_default(),
            _ => Default::default(),
        };
        let desc = self.full_type_name(db);

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

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
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

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
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
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match self.kind(db) {
            SpecKind::Struct(st) => st
                .elements(db)
                .iter()
                .map(|el| CompletionItem {
                    label: el.name(db).text(db).to_string(),
                    kind: Some(auto_lsp::lsp_types::CompletionItemKind::FIELD),
                    detail: Some(el.spec(db).to_ty(db).type_name(db).to_string()),
                    ..Default::default()
                })
                .collect::<Vec<_>>()
                .into(),
            SpecKind::Ref(r) => r.completion(db, offset),
            SpecKind::Target(target) => {
                match resolve_namespace_access(db, &target.path) {
                    Some(pou) => pou.completion(db, offset),
                    None => {
                        let mut results = vec![];
                        results.extend(static_snippets::elem_type_names());
                        results.extend(
                            global_pou_index(db)
                                .iter()
                                .map(|(_, pou)| simple_pou_completion(db, *pou))
                                .collect::<Vec<_>>(),
                        );

                        Some(results)
                    }
                }
            }
            _ => None,
        }
    }
}
