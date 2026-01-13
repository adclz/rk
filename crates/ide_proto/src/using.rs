use auto_lsp::{
    core::semantic_tokens_builder::SemanticTokensBuilder,
    default::db::BaseDatabase,
    lsp_types::{Hover, HoverContents, MarkedString},
};
use db::WorkspaceDataBase;
use hir::{
    hir_def::using::Using,
};

use crate::{NAMESPACE, SUPPORTED_TYPES, to_proto::ToProtocol};

impl<'db> ToProtocol<'db> for Using<'db> {
    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Hover> {
        let mut accumulated_path = Vec::new();

        for (index, fragment) in self.path(db).fragments(db).iter().enumerate() {
            let span = self.path(db).get_fragment_ast_node(db, index).get_span();
            accumulated_path.push(fragment.text(db).to_string());

            if offset >= span.start_byte && offset <= span.end_byte {
                let full_path = accumulated_path.join(".");
                return Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::from_markdown(format!(
                        r#"
```iecst
(using) NAMESPACE {}
```
"#,
                        full_path
                    ))),
                    range: None,
                });
            }
        }

        None
    }

    fn semantic_tokens(&'db self, db: &'db dyn WorkspaceDataBase, builder: &mut SemanticTokensBuilder) {
        for (index, _fragment) in self.path(db).fragments(db).iter().enumerate() {
            let span = self.path(db).get_fragment_ast_node(db, index).get_span();
            builder.push(
                span.lsp(),
                SUPPORTED_TYPES
                    .iter()
                    .position(|x| *x == NAMESPACE)
                    .unwrap() as u32,
                0,
            );
        }
    }
}
