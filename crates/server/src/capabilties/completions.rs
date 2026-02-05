use std::ops::ControlFlow;

use auto_lsp::{
    anyhow,
    lsp_types::{CompletionParams, CompletionResponse},
};
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::expression::InitExprKind};
use ide_proto::{
    hir_node::HirNode,
    walk::descendant_at_with,
};

pub fn completions(
    db: &impl WorkspaceDataBase,
    params: CompletionParams,
) -> anyhow::Result<Option<CompletionResponse>> {
    let uri = &params.text_document_position.text_document.uri;

    let file = match db.get_file(uri) {
        Some(file) => file,
        None => return Ok(None),
    };

    let doc = file.document(db);

    let position = params.text_document_position.position;
    let offset = match doc.offset_at(position) {
        Some(offset) => match params.context.unwrap().trigger_character {
            Some(str) if str == "." || str == "#" => offset.saturating_sub(1),
            _ => offset,
        },
        None => return Ok(None),
    };

    // we need to keep track of the previous node in case we need to fallback
    let mut prev = None;
    let mut latest = None;

    Ok(descendant_at_with(db, file, offset, |hirnode| {
        match hirnode {
            HirNode::VariableAccess(_) | HirNode::PathExpr(_) => {
                prev = latest.clone();
                latest = Some(hirnode);
            }
            HirNode::InitExpr(expr) => {
                // we only provide completions for struct initializers
                if let InitExprKind::StructInit { .. } = expr.kind(db) {
                    prev = latest.clone();
                    latest = Some(hirnode);
                }
            }
            _ => (),
        }
        ControlFlow::Continue(())
    })
    .map(|target| {
        let target_hir_node = match latest {
            Some(curr) => {
                let range = curr.get_span(db);
                if range.start_byte <= offset && offset <= range.end_byte {
                    // Latest is valid (contains offset), use it
                    curr
                } else if let Some(parent) = &prev {
                    // Latest is not valid, check if parent is
                    let parent_range = parent.get_span(db);
                    if parent_range.start_byte <= offset && offset <= parent_range.end_byte {
                        parent.clone()
                    } else {
                        // Neither is valid, use target
                        target
                    }
                } else {
                    // No parent to try, use target
                    target
                }
            }
            _ => target,
        };
        eprintln!("Found HirNode: {:?}", target_hir_node);
        CompletionResponse::Array(target_hir_node.completion(db, offset).unwrap_or_default())
    })
    .or_else(|| {
        Some(CompletionResponse::Array(vec![
            ide_proto::handlers::completions_utils::static_snippets::namespace(),
            ide_proto::handlers::completions_utils::static_snippets::using(),
            ide_proto::handlers::completions_utils::static_snippets::function(),
            ide_proto::handlers::completions_utils::static_snippets::function_block(),
            ide_proto::handlers::completions_utils::static_snippets::class(),
            ide_proto::handlers::completions_utils::static_snippets::interface(),
            ide_proto::handlers::completions_utils::static_snippets::type_(),
        ]))
    }))
}
