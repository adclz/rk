use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{CompletionItem, CompletionItemKind, CompletionItemLabelDetails, InsertTextFormat},
};
use hir::{
    hir_def::{
        pous::{
            pou::{Pou, PouDecl},
            variable::VariableKind,
        },
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    hir_ty::{
        name_res::{all_global_pous, all_pous_in_scope},
        signatures::LocalVariables,
    },
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
                .flat_map(|(_, pou)| signature_pou_completion(db, *pou))
                .for_each(|(pou)| results.push(pou));
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
            all_global_pous(db)
                .iter()
                .flat_map(|(_, pou)| signature_pou_completion(db, *pou))
                .collect(),
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

pub fn signature_pou_completion<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> Option<CompletionItem> {
    let sep = match pou.local_variables(db).len() {
        0..5 => "",
        _ => "\n",
    };
    let signature = format!(
        "{}({sep}{}{sep});",
        pou.name(db).text(db),
        pou.local_variables(db)
            .iter()
            .enumerate()
            .filter_map(|(i, (n, v))| {
                Some(match v.kind(db) {
                    VariableKind::Input => {
                        format!(
                            "\t{} := ${{{}:{}}}",
                            v.name(db).text(db),
                            i,
                            v.name(db).text(db)
                        )
                    }
                    VariableKind::InOut => {
                        format!(
                            "\t{} := ${{{}:{}}}",
                            v.name(db).text(db),
                            i,
                            v.name(db).text(db)
                        )
                    }
                    VariableKind::Output => {
                        format!(
                            "\t{} => ${{{}:{}}}",
                            v.name(db).text(db),
                            i,
                            v.name(db).text(db)
                        )
                    }
                    _ => None?,
                })
            })
            .collect::<Vec<_>>()
            .join(match pou.local_variables(db).len() {
                0..5 => ", ",
                _ => ",\n",
            })
    );
    Some(match pou.pou(db) {
        Pou::FunctionBlock(fb) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("FUNCTION BLOCK".into()),
            label_details: Some(CompletionItemLabelDetails {
                detail: Some("CALL".into()),
                ..Default::default()
            }),
            kind: Some(CompletionItemKind::STRUCT),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            insert_text: Some(signature),
            ..Default::default()
        },
        Pou::Class(cl) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("CLASS".into()),
            label_details: Some(CompletionItemLabelDetails {
                detail: Some("CALL".into()),
                ..Default::default()
            }),
            kind: Some(CompletionItemKind::CLASS),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            insert_text: Some(signature),
            ..Default::default()
        },
        Pou::Function(f) => CompletionItem {
            label: pou.name(db).text(db).to_string(),
            detail: Some("FUNCTION".into()),
            label_details: Some(CompletionItemLabelDetails {
                detail: Some("CALL".into()),
                ..Default::default()
            }),
            kind: Some(CompletionItemKind::FUNCTION),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            insert_text: Some(signature),
            ..Default::default()
        },
        _ => None?,
    })
}
