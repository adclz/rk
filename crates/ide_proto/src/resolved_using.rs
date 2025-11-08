use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{Hover, HoverContents, MarkedString},
};
use hir::{HirNodeInfo, hir_def::using::Using};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for Using<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        for fragment in self.path(db).fragments(db) {
            let span = fragment.get_span(db);

            if offset < span.start_byte || offset > span.end_byte {
                continue;
            }

            let fragment_name = fragment.text(db).to_string();
            return Some(Hover {
                contents: HoverContents::Scalar(MarkedString::from_markdown(
                    format!(
                        r#"
```iecst
(using) NAMESPACE {fragment_name}
```
                    "#
                    )
                    .to_string(),
                )),
                range: None,
            });
        }
        None
    }
}
