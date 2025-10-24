use auto_lsp::{
    core::document_symbols_builder::DocumentSymbolsBuilder,
    default::db::BaseDatabase,
    lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, SymbolKind},
};
use hir::{
    HirNodeInfo, TypeInfo,
    hir_ty::inheritance_solver::MethodRef,
};

use crate::{HasComment, ToProtocol};

impl<'db> ToProtocol<'db> for MethodRef<'db> {
    fn document_symbols(&self, db: &'db dyn BaseDatabase, builder: &mut DocumentSymbolsBuilder) {
        let mut nested_builder = DocumentSymbolsBuilder::default();
        self.variables(db)
            .iter()
            .for_each(|var| var.document_symbols(db, &mut nested_builder));

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name: self.name(db).text(db).to_string(),
            detail: Some(format!("METHOD{}", match self.return_type(db) {
                Some(dt) => format!(" : {}", dt.to_ty(db).type_name(db)),
                None => "".into()
            })),
            kind: SymbolKind::METHOD,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.name_span(db).lsp(),
            children: Some(nested_builder.finalize()),
            tags: None,
        });
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        let kind = match self {
            MethodRef::Declared(_) => "METHOD",
            MethodRef::Prototype(_) => "METHOD PROTOTYPE",
        };
        let name = self.name(db).text(db);
        let return_type = match self {
            MethodRef::Declared(decl) => match decl.return_type(db) {
                Some(ret_ty) => format!(": {}", ret_ty.to_ty(db).type_name(db)),
                None => "".to_string(),
            },
            MethodRef::Prototype(proto) => match proto.return_type(db) {
                Some(ret_ty) => format!(": {}", ret_ty.to_ty(db).type_name(db)),
                None => "".to_string(),
            },
        };
        let comment = self.get_comment(db).unwrap_or_default();

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
[{kind}] {name}{return_type}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }
}
