use auto_enums::auto_enum;
use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{
        CompletionItem, InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind,
    },
};

use crate::{
    completions,
    hir::{
        comment_index::comment_index,
        interned::identifier::Ident,
        pous::{
            class::Class, data_type::DataType, function::Function, function_block::FunctionBlock,
            interface::Interface,
        },
        scopes::scope::{FilePouId, ScopeId},
        semantic_index::SemanticIndex,
        signature::type_signature,
    },
    to_proto::{self_iter, Extends, IterToProto, SymbolInfo, ToProto},
};

#[salsa::tracked(debug)]
pub struct PouDecl<'db> {
    #[tracked]
    #[returns(ref)]
    pub pou: Pou<'db>,

    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub name: Ident,

    #[tracked]
    #[returns(ref)]
    pub name_span: Span,

    pub pou_id: FilePouId,

    pub scope_id: ScopeId,
}

impl<'db> IterToProto<'db> for PouDecl<'db> {
    #[auto_enum(Iterator)]
    fn iter(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self.pou(db) {
            Pou::Function(f) => self_iter(self).chain(f.iter(db, sema)),
            Pou::FunctionBlock(fb) => self_iter(self).chain(fb.iter(db, sema)),
            Pou::Class(c) => self_iter(self).chain(c.iter(db, sema)),
            Pou::DataType(d) => self_iter(self).chain(d.iter(db, sema)),
            Pou::Interface(i) => self_iter(self).chain(i.iter(db, sema)),
        }
    }
}

impl<'db> ToProto<'db> for PouDecl<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn get_named_span(&'db self, db: &'db dyn BaseDatabase) -> Option<&'db Span> {
        Some(self.name_span(db))
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
                .maybe_spec(match self.pou(db) {
                    Pou::DataType(d) => Some(d.spec(db)),
                    _ => None,
                })
                .maybe_init(match self.pou(db) {
                    Pou::DataType(d) => d.init(db),
                    _ => None,
                })
                .name_range(self.name_span(db).clone())
                .maybe_extends(match self.pou(db) {
                    Pou::Class(c) => c.extends(db).map(|a| Extends::Single(a.clone())),
                    Pou::FunctionBlock(fb) => fb.extends(db).map(|a| Extends::Single(a.clone())),
                    Pou::Interface(i) => i.extends(db).map(Extends::Multiple),
                    _ => None,
                })
                .maybe_implements(match self.pou(db) {
                    Pou::Class(c) => c.implements(db).cloned(),
                    Pou::FunctionBlock(fb) => fb.implements(db).cloned(),
                    _ => None,
                })
                .build(),
        )
    }

    fn completion(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        match self.pou(db) {
            Pou::Function(f) => f.completion_ctx(db, sema, offset),
            Pou::FunctionBlock(_) => Some(vec![completions::snippets::var_input()]),
            Pou::Class(_) => Some(vec![completions::snippets::var_input()]),
            Pou::Interface(_) => Some(vec![completions::snippets::var_input()]),
            Pou::DataType(_) => None,
        }
    }

    fn inlay_hint(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<InlayHint> {
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
            position: self.span(db).lsp().end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }

    fn hover(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        let comment = comment_index(db, sema.file);
        let comment = comment
            .find_nearby_comment(sema.file.document(db), self.span(db))
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
                    type_signature(db, *self)
                        .map(|s| s.signature_to_string(db, sema, self.scope_id(db)))
                        .unwrap_or_else(|| "{unknown}".to_string()),
                ),
            }),
            range: Some(
                self.get_named_span(db)
                    .map(|span| span.lsp())
                    .unwrap_or_default(),
            ),
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
