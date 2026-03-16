use auto_lsp::lsp_types::{CodeLens, Command};
use db::WorkspaceDataBase;
use hir::{HasName, HirNodeInfo, hir_def::hir_node::HirNode, hir_def::pous::pou::Pou};
use serde_json::to_value;

use crate::handlers::{CodeLensHandler, implementation::find_all_implementations};

impl<'db> CodeLensHandler<'db> for HirNode<'db> {
    fn code_lens(&self, db: &'db dyn WorkspaceDataBase) -> Option<CodeLens> {
        match self {
            HirNode::PouDecl(pou) => pou.code_lens(db),
            HirNode::Program(prog) if prog.is_test(db) => Some(test_code_lens(
                prog.get_span(db).lsp(),
                prog.get_scope_id(db).file(db).url(db).as_str(),
                &prog.name(db).text(db),
            )),
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
            Pou::Function(f) if f.is_test(db) => Some(test_code_lens(
                self.get_span(db).lsp_with_enc(self.get_scope_id(db).file(db).document(db)).unwrap(),
                self.get_scope_id(db).file(db).url(db).as_str(),
                &self.get_name_ident(db).text(db),
            )),
            _ => None,
        }
    }
}

fn test_code_lens(range: auto_lsp::lsp_types::Range, file_uri: &str, name: &str) -> CodeLens {
    CodeLens {
        range,
        command: Some(Command {
            title: "$(testing-run-icon) Run test".into(),
            command: "rk.runTest".into(),
            arguments: Some(vec![
                serde_json::to_value(file_uri).unwrap(),
                serde_json::to_value(name).unwrap(),
            ]),
        }),
        data: None,
    }
}
