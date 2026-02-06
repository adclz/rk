use auto_lsp::lsp_types::{CodeLens, Command};
use db::WorkspaceDataBase;
use hir::{HasName, HirNodeInfo, hir_def::pous::pou::Pou};
use serde_json::to_value;

use crate::{handlers::{CodeLensHandler, implementation::find_all_implementations}, hir_node::HirNode};

impl<'db> HirNode<'db> {
        pub fn code_lens(&self, db: &'db dyn WorkspaceDataBase) -> Option<CodeLens> {
        match self {
            HirNode::PouDecl(pou) => pou.code_lens(db),
            _ => None,
        }
    }
}

impl<'db> CodeLensHandler<'db> for Pou<'db> {
    fn code_lens(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<CodeLens> {
        match self {
            Pou::Class(_) | Pou::Interface(_) => {
                let implementations = find_all_implementations(db, *self);
                if implementations.is_empty() {
                    None
                } else {
                    Some(CodeLens {
                        range: self.get_span(db).lsp(),
                        command: Some(Command {
                            title: format!(
                                "{} implementation{}",
                                implementations.len(),
                                if implementations.len() > 1 { "s" } else { "" }
                            ),
                            command: "rk.showImplementations".into(),
                            arguments: Some(vec![
                                to_value(self.get_scope_id(db).file(db).url(db).as_str()).unwrap(),
                                to_value(self.get_name_span(db).lsp().start).unwrap(),
                            ]),
                        }),
                        data: None,
                    })
                }
            }
            _ => None,
        }
    }
}
