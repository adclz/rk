use std::ops::ControlFlow;

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        hir_node::HirNode, semantic_index::{SemanticIndex, semantic_index},
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
        db: &'db dyn WorkspaceDataBase,
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
) -> Option<HirNode<'db>> {
    let mut best_match: Option<HirNode<'db>> = None;
    let mut best_size: usize = usize::MAX;

    let _ = semantic_index(db, file).walk_hir(db, &mut |node| {
        let range = node.get_span(db);
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
