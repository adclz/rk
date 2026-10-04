// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;

use auto_lsp::lsp_types::{TextEdit, WorkspaceEdit};
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::hir_node::HirNode};

use crate::handlers::{ReferencesHandler, RenameHandler};

impl<'db> RenameHandler<'db> for HirNode<'db> {
    fn rename(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        // A library file is not the workspace's to edit, and what matters is
        // where the name is DECLARED, not where this use of it sits. Renaming
        // rewrote every use and left the declaration behind, so the next
        // check reported E0203 on code the reader had not touched.
        if declared_in_a_library(self, db) {
            return None;
        }

        // A namespace is renamed by its last segment: on `App` of `NAMESPACE
        // App.Motors` the cursor is on the parent, which is renamed where
        // it is declared, and nothing is edited from here.
        let dotted = match self {
            HirNode::Namespace(ns) => Some((ns.get_scope_id(db).file(db), ns.name_span(db))),
            HirNode::Using(u) => Some((u.get_scope_id(db).file(db), u.get_span(db))),
            _ => None,
        };
        if let Some((file, span)) = dotted
            && offset < super::references::last_segment(db, file, span).start_byte
        {
            return None;
        }

        let locations = self.locations(db)?;

        let mut changes: HashMap<_, Vec<TextEdit>> = HashMap::new();
        for loc in locations {
            // An edit replaces a name and nothing else. A located variable
            // declared without one, `AT %QB4 : INT`, has its address where
            // a name would be, and a declaration whose name the parser had
            // to make up has nothing there at all.
            if !is_identifier(loc.file.document(db).as_str(), &loc.span) {
                continue;
            }
            changes
                .entry(loc.file.url(db).clone())
                .or_default()
                .push(TextEdit {
                    range: hir::denormalize(db, loc.file, &loc.span).unwrap_or_default(),
                    new_text: new_name.to_string(),
                });
        }

        if changes.is_empty() {
            return None;
        }
        Some(WorkspaceEdit::new(changes))
    }
}

/// Whether `span` of `source` holds one identifier.
fn is_identifier(source: &str, span: &auto_lsp::tree_sitter::Range) -> bool {
    source
        .get(span.start_byte..span.end_byte)
        .is_some_and(|text| {
            !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_')
        })
}

/// Whether the name this node refers to is declared in a library file.
fn declared_in_a_library<'db>(node: &'db HirNode<'db>, db: &'db dyn WorkspaceDataBase) -> bool {
    use crate::handlers::DefinitionHandler;
    use auto_lsp::lsp_types::GotoDefinitionResponse;

    let declared = match node.definition(db, node.get_span(db).start_byte) {
        Some(GotoDefinitionResponse::Scalar(location)) => location.uri,
        Some(GotoDefinitionResponse::Array(mut locations)) => match locations.pop() {
            Some(location) => location.uri,
            None => return false,
        },
        _ => return false,
    };
    db.get_library_files().contains_key(&declared)
}
