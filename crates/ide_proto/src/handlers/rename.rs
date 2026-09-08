use std::collections::HashMap;

use auto_lsp::lsp_types::{TextEdit, WorkspaceEdit};
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::hir_node::HirNode};

use crate::handlers::{ReferencesHandler, RenameHandler};

impl<'db> RenameHandler<'db> for HirNode<'db> {
    fn rename(&'db self, db: &'db dyn WorkspaceDataBase, new_name: &str) -> Option<WorkspaceEdit> {
        // A library file is not the workspace's to edit, and what matters is
        // where the name is DECLARED, not where this use of it sits. Renaming
        // rewrote every use and left the declaration behind, so the next
        // check reported E0210 on code the reader had not touched.
        if declared_in_a_library(self, db) {
            return None;
        }

        let locations = self.locations(db)?;

        let mut changes: HashMap<_, Vec<TextEdit>> = HashMap::new();
        for loc in locations {
            changes
                .entry(loc.file.url(db).clone())
                .or_default()
                .push(TextEdit {
                    range: hir::denormalize(db, loc.file, &loc.span).unwrap_or_default(),
                    new_text: new_name.to_string(),
                });
        }

        Some(WorkspaceEdit::new(changes))
    }
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
