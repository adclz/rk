use crate::{
    completions,
    def::{expressions::statement::Stmt, modifier::Modifier, semantic_index::semantic_index},
};
use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind,
    },
};

use crate::{
    def::{
        comment_index::comment_index,
        interned::identifier::Ident,
        pous::{
            class::Class, data_type::DataType, function::Function, function_block::FunctionBlock,
            interface::Interface,
        },
        scope::FileScopeId,
    },
    to_proto::{AstId, SymbolInfo, ToProto},
};

#[salsa::tracked(debug)]
pub struct PouDecl<'db> {
    #[tracked]
    #[returns(ref)]
    pub pou: Pou<'db>,

    #[returns(ref)]
    pub name: Ident,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> PouDecl<'db> {
    pub fn get_stmts(&'db self, db: &'db dyn BaseDatabase) -> Option<&'db Vec<Stmt<'db>>> {
        match self.pou(db) {
            Pou::Function(f) => Some(f.statements(db)),
            Pou::FunctionBlock(fb) => Some(fb.statements(db)),
            _ => None,
        }
    }

    pub fn modifier(&'db self, db: &'db dyn BaseDatabase) -> Modifier {
        match self.pou(db) {
            Pou::Class(class) => class.modifier(db),
            Pou::FunctionBlock(fb) => fb.modifier(db),
            _ => Modifier::empty(),
        }
    }
}

impl<'db> ToProto<'db> for PouDecl<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(match self.pou(db) {
                    Pou::Function(_) => auto_lsp::lsp_types::SymbolKind::FUNCTION,
                    Pou::FunctionBlock(_) => auto_lsp::lsp_types::SymbolKind::FUNCTION,
                    Pou::Class(_) => auto_lsp::lsp_types::SymbolKind::CLASS,
                    Pou::Interface(_) => auto_lsp::lsp_types::SymbolKind::INTERFACE,
                    Pou::DataType(_) => auto_lsp::lsp_types::SymbolKind::TYPE_PARAMETER,
                })
                .name(self.name(db).text(db).to_string())
                .range(self.get_span(db).clone())
                .name_range(self.get_name_span(db)?.clone())
                .maybe_spec(match self.pou(db) {
                    Pou::DataType(d) => Some(d.spec(db)),
                    _ => None,
                })
                .maybe_init(match self.pou(db) {
                    Pou::DataType(d) => d.init(db),
                    _ => None,
                })
                .build(),
        )
    }

    fn completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match self.pou(db) {
            Pou::Function(f) => f.completion_ctx(db, offset),
            Pou::FunctionBlock(_) => Some(vec![completions::snippets::var_input()]),
            Pou::Class(_) => Some(vec![completions::snippets::var_input()]),
            Pou::Interface(_) => Some(vec![completions::snippets::var_input()]),
            Pou::DataType(_) => None,
        }
    }

    fn inlay_hint(&'db self, db: &'db dyn BaseDatabase) -> Option<InlayHint> {
        Some(InlayHint {
            label: InlayHintLabel::String(format!(
                "{} {}",
                match self.pou(db) {
                    Pou::Function(_) => "function",
                    Pou::FunctionBlock(_) => "function block",
                    Pou::Class(_) => "class",
                    Pou::Interface(_) => "interface",
                    Pou::DataType(_) => "data type",
                },
                self.name(db).text(db)
            )),
            position: self.get_span(db).lsp().end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }

    fn hover(&'db self, db: &'db dyn BaseDatabase) -> Option<auto_lsp::lsp_types::Hover> {
        let sema = semantic_index(db, self.get_scope_id(db).file(db));
        let name_span = self.get_name_span(db)?;
        let comment = comment_index(db, sema.file);
        let comment = comment
            .find_nearby_comment(sema.file.document(db), &name_span)
            .map(|c| format!("{}\n&nbsp;", c.to_string(sema.file.document(db))))
            .unwrap_or_default();

        Some(auto_lsp::lsp_types::Hover {
            contents: auto_lsp::lsp_types::HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!(
                    r#"{comment}
```typescript
{}
```
"#,
                    "signature"
                ),
            }),
            range: Some(name_span.lsp()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update, salsa::Supertype)]
pub enum Pou<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    DataType(DataType<'db>),
}
