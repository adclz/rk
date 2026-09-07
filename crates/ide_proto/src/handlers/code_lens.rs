use auto_lsp::lsp_types::{CodeLens, Command};
use db::WorkspaceDataBase;
use hir::hir_ty::ty::Type;
use hir::{HasName, HasPragmas, HirNodeInfo, hir_def::hir_node::HirNode, hir_def::pous::pou::Pou};
use serde_json::to_value;

use crate::handlers::{CodeLensHandler, implementation::find_all_implementations};

impl<'db> CodeLensHandler<'db> for HirNode<'db> {
    fn code_lens(&self, db: &'db dyn WorkspaceDataBase) -> Option<CodeLens> {
        match self {
            HirNode::PouDecl(pou) => pou.code_lens(db),
            // The lens sits ON the `{test}` pragma, so the editor draws it
            // directly above the mark it runs: a declaration's own span opens
            // at its FIRST pragma, and an `{allow ...}` above `{test}` used to
            // carry the lens onto that instead.
            HirNode::Program(prog) => {
                let marker = prog.test_pragma(db)?;
                let qualified = Type::Program(*prog).qualified_path(db);
                Some(test_code_lens(
                    hir::denormalize(db, prog.get_scope_id(db).file(db), &marker.get_span(db))
                        .unwrap_or_default(),
                    prog.get_scope_id(db).file(db).url(db).as_str(),
                    &qualified,
                ))
            }
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
                        range: hir::denormalize(
                            db,
                            self.get_scope_id(db).file(db),
                            &self.get_span(db),
                        )
                        .unwrap_or_default(),
                        command: Some(Command {
                            title: format!(
                                "{} implementation{}",
                                implementations.len(),
                                if implementations.len() > 1 { "s" } else { "" }
                            ),
                            command: "rk.showImplementations".into(),
                            arguments: Some(vec![
                                to_value(self.get_scope_id(db).file(db).url(db).as_str()).unwrap(),
                                to_value(
                                    hir::denormalize(
                                        db,
                                        self.get_scope_id(db).file(db),
                                        &self.get_name_span(db),
                                    )
                                    .unwrap_or_default()
                                    .start,
                                )
                                .unwrap(),
                            ]),
                        }),
                        data: None,
                    })
                }
            }
            Pou::Function(f) => {
                let marker = f.test_pragma(db)?;
                let qualified = Type::new_pou(db, *self).qualified_path(db);
                Some(test_code_lens(
                    hir::denormalize(db, self.get_scope_id(db).file(db), &marker.get_span(db))
                        .unwrap_or_default(),
                    self.get_scope_id(db).file(db).url(db).as_str(),
                    &qualified,
                ))
            }
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
