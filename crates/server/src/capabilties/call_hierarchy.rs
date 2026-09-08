use auto_lsp::{
    anyhow,
    lsp_types::{
        CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
        CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    },
};
use db::WorkspaceDataBase;
use ide_proto::{
    handlers::call_hierarchy,
    walk::{descendant_at, position_to_offset},
};

pub fn prepare_call_hierarchy(
    db: &impl WorkspaceDataBase,
    params: CallHierarchyPrepareParams,
) -> anyhow::Result<Option<Vec<CallHierarchyItem>>> {
    let position = params.text_document_position_params;
    let Some(file) = db.get_file(&position.text_document.uri) else {
        return Ok(None);
    };
    let offset = position_to_offset(db, file, position.position)
        .ok_or_else(|| anyhow::format_err!("Invalid position, {:?}", position.position))?;
    Ok(descendant_at(db, file, offset).and_then(|node| call_hierarchy::prepare(db, &node)))
}

pub fn incoming_calls(
    db: &impl WorkspaceDataBase,
    params: CallHierarchyIncomingCallsParams,
) -> anyhow::Result<Option<Vec<CallHierarchyIncomingCall>>> {
    Ok(resolve_item(db, &params.item)?.and_then(|node| call_hierarchy::incoming(db, &node)))
}

pub fn outgoing_calls(
    db: &impl WorkspaceDataBase,
    params: CallHierarchyOutgoingCallsParams,
) -> anyhow::Result<Option<Vec<CallHierarchyOutgoingCall>>> {
    Ok(resolve_item(db, &params.item)?.and_then(|node| call_hierarchy::outgoing(db, &node)))
}

/// The declaration an item stands for, found again from where it points.
///
/// The item travels to the client and back, so nothing in it can be a handle
/// into the database; `selection_range` is the name, which is enough to find
/// the node the same way a request at that position would.
fn resolve_item<'db>(
    db: &'db impl WorkspaceDataBase,
    item: &CallHierarchyItem,
) -> anyhow::Result<Option<hir::hir_def::hir_node::HirNode<'db>>> {
    let Some(file) = db.get_file(&item.uri) else {
        return Ok(None);
    };
    let offset = position_to_offset(db, file, item.selection_range.start)
        .ok_or_else(|| anyhow::format_err!("Invalid position, {:?}", item.selection_range.start))?;
    Ok(descendant_at(db, file, offset))
}
