use std::collections::HashMap;

use auto_lsp::lsp_types::{TextEdit, WorkspaceEdit};
use db::WorkspaceDataBase;
use hir::hir_def::hir_node::HirNode;

use crate::handlers::{ReferencesHandler, RenameHandler};

impl<'db> RenameHandler<'db> for HirNode<'db> {
    fn rename(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        let locations = self.locations(db)?;

        let mut changes: HashMap<_, Vec<TextEdit>> = HashMap::new();
        for loc in locations {
            changes
                .entry(loc.file.url(db).clone())
                .or_default()
                .push(TextEdit {
                    range: loc.span.into(),
                    new_text: new_name.to_string(),
                });
        }

        Some(WorkspaceEdit::new(changes))
    }
}
