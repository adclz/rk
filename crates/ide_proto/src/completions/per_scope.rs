use auto_lsp::{default::db::BaseDatabase, lsp_types::{CompletionItem, CompletionItemKind, CompletionItemLabelDetails}};
use hir::{
    hir_def::{
        pous::pou::{Pou, PouDecl},
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    hir_ty::name_res::{all_global_pous, all_pous_in_scope},
};

use crate::ToProtocol;

pub fn scoped_completions<'db>(
    db: &'db dyn BaseDatabase,
    file_scope: FileScopeId<'db>,
    offset: usize,
) -> Option<Vec<CompletionItem>> {
    let sema = semantic_index(db, file_scope.file(db));
    let scope = sema.get_scope(db, file_scope);
    match scope.kind {
        ScopeKind::Pou(pou) => {
            let mut results = vec![];
            if let Some(base) = pou.completion(db, offset) {
                results.extend(base);
            }
            all_global_pous(db)
                .iter()
                .for_each(|(_, pou)| results.push(simple_pou_completion(db, *pou)));
            match pou.pou(db) {
                Pou::FunctionBlock(fb) => {
                    results.push(CompletionItem::new_simple("THIS".to_string(), "".into()));
                    results.push(CompletionItem::new_simple("SUPER".to_string(), "".into()));
                }
                Pou::Class(cl) => {
                    results.push(CompletionItem::new_simple("THIS".to_string(), "".into()));
                    results.push(CompletionItem::new_simple("SUPER".to_string(), "".into()));
                }
                Pou::Function(f) => {
                    results.push(CompletionItem::new_simple(
                        pou.name(db).text(db).to_string(),
                        "(Self)".into(),
                    ));
                }
                _ => {}
            }
            Some(results)
        }
        ScopeKind::Global => Some(
            all_pous_in_scope(db, file_scope)
                .iter()
                .map(|(_, pou)| simple_pou_completion(db, *pou))
                .collect::<Vec<_>>(),
        ),
        _ => None,
    }
}

pub fn simple_pou_completion<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> CompletionItem {
    match pou.pou(db) {
        Pou::FunctionBlock(fb) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("FUNCTION BLOCK".into()),
            kind: Some(CompletionItemKind::STRUCT),
            ..Default::default()
        },
        Pou::Class(cl) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("CLASS".into()),
            kind: Some(CompletionItemKind::CLASS),
            ..Default::default()
        },
        Pou::Function(f) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("FUNCTION".into()),
            kind: Some(CompletionItemKind::FUNCTION),
            ..Default::default()
        },
        Pou::Interface(f) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("INTERFACE".into()),
                        kind: Some(CompletionItemKind::INTERFACE),
            ..Default::default()
        },
        Pou::DataType(dt) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some(dt.spec(db).type_name(db)),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            ..Default::default()
        },
    }
}

pub fn signature_pou_completion<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Option<CompletionItem> {
    Some(match pou.pou(db) {
        Pou::FunctionBlock(fb) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("FUNCTION BLOCK".into()),
            kind: Some(CompletionItemKind::STRUCT),
            ..Default::default()
        },
        Pou::Class(cl) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("CLASS".into()),
            kind: Some(CompletionItemKind::CLASS),
            ..Default::default()
        },
        Pou::Function(f) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("FUNCTION".into()),
            kind: Some(CompletionItemKind::FUNCTION),
            ..Default::default()
        },
        _ => None?,
    })
}

