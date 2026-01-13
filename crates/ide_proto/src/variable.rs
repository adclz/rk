use auto_lsp::{
    core::{
        document_symbols_builder::DocumentSymbolsBuilder,
        semantic_tokens_builder::SemanticTokensBuilder,
    },
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, GotoDefinitionResponse, Hover, HoverContents, Location, MarkupContent,
        MarkupKind, SymbolKind, request::GotoDeclarationResponse,
    },
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::pous::variable::{VariableDecl, VariableKind},
    hir_ty::ty::Type,
};

use crate::to_proto::{HasComment, ToProtocol};

impl<'db> ToProtocol<'db> for VariableDecl<'db> {
    fn document_symbols(&self, db: &'db dyn WorkspaceDataBase, builder: &mut DocumentSymbolsBuilder) {
        let name = self.name(db).text(db).to_string();
        let name = match name.len() {
            0 => "?".into(),
            _ => name,
        };

        builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
            name,
            detail: Some(Type::new_spec(db, self.spec(db)).type_name(db)),
            kind: SymbolKind::VARIABLE,
            deprecated: None,
            range: self.get_span(db).lsp(),
            selection_range: self.get_name_span(db).lsp(),
            children: None,
            tags: None,
        });
    }

    fn hover(&'db self, db: &'db dyn WorkspaceDataBase, _offset: usize) -> Option<Hover> {
        let comment = self.get_comment(db).unwrap_or_default();
        let kind = match self.kind(db) {
            VariableKind::Input => "INPUT",
            VariableKind::Output => "OUTPUT",
            VariableKind::InOut => "IN_OUT",
            VariableKind::Var => "VAR",
            VariableKind::External => "EXTERNAL",
            VariableKind::Global => "GLOBAL",
            VariableKind::Access => "ACCESS",
            VariableKind::Config => "CONFIG",
            VariableKind::Temp => "TEMP",
        };

        let name = self.name(db).text(db);
        let type_name = Type::new_spec(db, self.spec(db)).type_name(db);

        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"
{comment}
```iecst
({kind}) {name}: {type_name}
```
                "#
                ),
            }),
            range: Some(self.get_span(db).into()),
        })
    }

    fn declaration(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDeclarationResponse> {
        Some(GotoDeclarationResponse::Scalar(Location::new(
            self.scope_id(db).file(db).url(db).to_owned(),
            self.get_span(db).into(),
        )))
    }

    fn definition(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<GotoDefinitionResponse> {
        self.spec(db).definition(db)
    }

    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        self.spec(db).completion(db, offset)
    }

    fn semantic_tokens(&'db self, _db: &'db dyn WorkspaceDataBase, _builder: &mut SemanticTokensBuilder) {
        /*match self.spec(db).tokens(db, builder) {
            Some((typ, modi)) => {
                builder.push(
                    self.spec(db).get_span(db).lsp(),
                    SUPPORTED_TYPES.iter().position(|x| *x == typ).unwrap() as u32,
                    modi,
                );
            }
            None => {}
        };*/
    }
}
