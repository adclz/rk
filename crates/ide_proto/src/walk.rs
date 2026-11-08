// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::ops::ControlFlow;

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::expression::{PathExpr, PathExprKind},
        hir_node::HirNode,
        semantic_index::{NodeKey, SemanticIndex, semantic_index},
    },
};

/// Converts an LSP [`lsp_types::Position`] (in the client's negotiated encoding) into a UTF-8 byte
/// offset into the document.
///
/// The client always sends a position, never an offset; like rust-analyzer's `from_proto::offset`,
/// we convert at the boundary and work with byte offsets internally. The position is first
/// normalized to UTF-8 (row, byte-column) via the document, then resolved against its line starts.
///
/// Returns `None` if the position is out of bounds.
pub fn position_to_offset(
    db: &dyn WorkspaceDataBase,
    file: File,
    position: lsp_types::Position,
) -> Option<usize> {
    let document = file.document(db);
    let norm = document.normalize_position(&position).ok()?;
    // The row is the document's, which ends a line at a carriage return no
    // `\n` follows too: its start is read off the same line index, where
    // counting `\n` placed the request a line early past such a return.
    let line_start = document.texter.br_indexes.row_start(norm.line as usize)?;
    Some(line_start + norm.character as usize)
}

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

/// Where a path ends in the source. Its node spans the last name alone, and a
/// subscript or a `^` after that name is part of the path.
fn path_end<'db>(db: &'db dyn WorkspaceDataBase, path: PathExpr<'db>, source: &str) -> usize {
    let closed_by = |from: usize, closer: char| {
        let rest = source.get(from..).unwrap_or_default();
        let blank = rest.len() - rest.trim_start().len();
        if rest[blank..].starts_with(closer) {
            from + blank + closer.len_utf8()
        } else {
            from
        }
    };
    match path.expr(db) {
        PathExprKind::Index(index) => {
            let inside = match index.index.last() {
                Some(last) => last.get_span(db).end_byte,
                None => path_end(db, index.path, source),
            };
            closed_by(inside, ']')
        }
        PathExprKind::Deref(deref) => closed_by(path_end(db, deref.path, source), '^'),
        _ => path.get_span(db).end_byte,
    }
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
    let mut last_before_end: usize = 0;

    // The path ending nearest before the cursor, which is not the last node
    // before it: `0` of `rows[0].` comes after `rows[0]` and ends inside it.
    let mut path_before: Option<(HirNode<'db>, NodeKey, usize)> = None;

    let sema = semantic_index(db, file);
    let document = file.document(db);
    let source = document.as_str();

    for (idx, node) in sema.node_index.iter_enumerated() {
        let range = node.get_span(db);
        // Sorted by AstId, so once start_byte > offset no further node can contain it
        if range.start_byte > offset {
            break;
        }
        if offset <= range.end_byte {
            best_match = Some((node.clone(), idx, false));
        } else {
            last_before_end = range.end_byte;
            last_before = Some((node.clone(), idx, true));
        }
        // `THIS^` and `SUPER^` start a path as a name does.
        let end = match node {
            HirNode::PathExpr(path) => Some(path_end(db, *path, source)),
            HirNode::Invocation(_) => Some(range.end_byte),
            _ => None,
        };
        if let Some(end) = end
            && end <= offset
            && path_before.as_ref().is_none_or(|(_, _, last)| end >= *last)
        {
            path_before = Some((node.clone(), idx, end));
        }
    }

    // Prefer a node before the cursor when it appeared later in the source
    // (higher NodeKey), meaning it's more specific than the containing node
    // (e.g. a Pou): a path the cursor is past (e.g. `my_var.inner.|`) and a Using
    // (e.g. `USING ns.|` where the dot is past the Using HIR span).
    // For Using, also use last_before when best_match is None (top-level USING).
    // A path only stands in for the cursor while the cursor is still ON it:
    // nothing between them but the dot being typed. Without that, the last
    // name of one POU's body answered for a blank line several declarations
    // later, which offered statements where a declaration goes.
    let reaches_the_cursor = |end: usize| {
        source.get(end..offset).is_some_and(|gap| {
            gap.contains('.') && gap.chars().all(|c| c == '.' || c == ' ' || c == '\t')
        })
    };

    if let (Some((_, best_idx, _)), Some((path, path_idx, end))) = (&best_match, &path_before)
        && path_idx > best_idx
        && reaches_the_cursor(*end)
    {
        return Some((path.clone(), *path_idx, true));
    }

    match (&best_match, &last_before) {
        (Some((_, best_idx, _)), Some((HirNode::Using(_), last_idx, _)))
            if last_idx > best_idx && offset.saturating_sub(last_before_end) <= 2 =>
        {
            last_before
        }
        (Some(_), _) => best_match,
        (None, Some((HirNode::Using(_), _, _))) if offset.saturating_sub(last_before_end) <= 2 => {
            last_before
        }
        (None, _) => None,
    }
}
