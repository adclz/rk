use std::ops::ControlFlow;

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        hir_node::HirNode,
        semantic_index::{NodeKey, SemanticIndex, semantic_index},
    },
};

pub trait WalkHir<'db> {
    fn walk_hir<F>(&self, db: &'db dyn WorkspaceDataBase, f: &mut F) -> ControlFlow<()>
    where
        F: FnMut(HirNode<'db>) -> ControlFlow<()>;
}

impl<'db> WalkHir<'db> for SemanticIndex<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        _db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        for node in self.node_index.iter() {
            if let ControlFlow::Break(()) = f(node.clone()) {
                return ControlFlow::Break(());
            }
        }
        ControlFlow::Continue(())
    }
}

pub fn descendant_at<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<HirNode<'db>> {
    let mut best_match: Option<HirNode<'db>> = None;
    let mut best_size: usize = usize::MAX;

    let _ = semantic_index(db, file).walk_hir(db, &mut |node| {
        let range = node.get_span(db);
        // Sorted by AstId, so once start_byte > offset no further node can contain it
        if range.start_byte > offset {
            return ControlFlow::Break(());
        }
        if offset <= range.end_byte {
            let size = range.end_byte - range.start_byte;
            if size < best_size {
                best_size = size;
                best_match = Some(node);
            }
        }
        ControlFlow::Continue(())
    });

    best_match
}

pub fn completion_descendant_at<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<(HirNode<'db>, NodeKey, bool)> {
    let mut best_match: Option<(HirNode<'db>, NodeKey, bool)> = None;

    // Track the closest preceding node for cases where the cursor is in
    // whitespace after a dot (e.g. `my_var.inner. |`).
    let mut last_before: Option<(HirNode<'db>, NodeKey, bool)> = None;

    let sema = semantic_index(db, file);

    for (idx, node) in sema.node_index.iter_enumerated() {
        let range = node.get_span(db);
        // Sorted by AstId, so once start_byte > offset no further node can contain it
        if range.start_byte > offset {
            break;
        }
        if offset <= range.end_byte {
            best_match = Some((node.clone(), idx, false));
        } else {
            last_before = Some((node.clone(), idx, true));
        }
    }

    // Prefer last_before when it appeared later in the source (higher NodeKey),
    // meaning it's more specific than the containing node (e.g. a Pou).
    // Only use last_before for PathExpr nodes — other node types (Using, Namespace, etc.)
    // should not trigger completions when the cursor is past them.
    match (&best_match, &last_before) {
        (Some((_, best_idx, _)), Some((HirNode::PathExpr(_), last_idx, _)))
            if last_idx > best_idx =>
        {
            last_before
        }
        (Some(_), _) => best_match,
        (None, Some((HirNode::PathExpr(_), _, _))) => last_before,
        (None, _) => None,
    }
}
