use auto_lsp::core::span::Span;
use auto_lsp::default::db::{BaseDatabase};
use auto_lsp::lsp_types::{
    CompletionItem, InlayHint, InlayHintKind, InlayHintLabel, MarkupContent, MarkupKind,
};

use crate::completions;
use crate::hir::interned::namespace::NamespacePath;
use crate::hir::pous::pou::PouDecl;
use crate::hir::scope::{FileScopeId, Visibility};
use crate::hir::semantic_index::{semantic_index, SemanticIndex};
use crate::to_proto::{self_iter, AstId, IterToProto, SymbolInfo, ToProto};

#[salsa::tracked(debug)]
pub struct Namespace<'db> {
    #[returns(ref)]
    pub path: NamespacePath,

    #[returns(ref)]
    pub pous: Vec<PouDecl<'db>>,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId,
}

impl<'db> ToProto<'db> for Namespace<'db> {
    fn get_id(&'db self, db: &'db dyn crate::BaseDatabase) -> crate::to_proto::AstId {
        self.id(db)
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }

    fn get_name_span(&'db self, db: &'db dyn BaseDatabase) -> Option<Span> {
        let file = self.get_scope_id(db).file();
        Some(semantic_index(db, file).ast.get(self.name_id(db).0)?.get_span())
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::NAMESPACE)
                .name(self.path(db).to_string(db))
                .range(self.get_span(db).clone())
                .name_range(self.get_name_span(db)?.clone())
                .build(),
        )
    }

    fn hover(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        Some(auto_lsp::lsp_types::Hover {
            contents: auto_lsp::lsp_types::HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("Namespace `{}`", self.path(db).to_string(db)),
            }),
            range: Some(self.get_span(db).lsp()),
        })
    }

    fn inlay_hint(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
    ) -> Option<auto_lsp::lsp_types::InlayHint> {
        Some(InlayHint {
            label: InlayHintLabel::String(format!("namespace {}", self.path(db).to_string(db))),
            position: self.get_span(db).lsp().end,
            kind: Some(InlayHintKind::TYPE),
            text_edits: None,
            padding_left: Some(true),
            padding_right: None,
            data: None,
            tooltip: None,
        })
    }

    fn completion(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let scope = sema.get_scope(self.scope_id(db));

        // Don't provide completions between the namespace keyword and the namespace name
        if self.get_name_span(db)?.end_byte > offset {
            if !scope.visibility == Visibility::PUBLIC {
                return Some(vec![CompletionItem::new_simple(
                    "INTERNAL".into(),
                    "internal".into(),
                )]);
            } else {
                return None;
            }
        }

        let mut completions = vec![
            completions::snippets::namespace(),
            completions::snippets::function(),
            completions::snippets::function_block(),
            completions::snippets::type_(),
            completions::snippets::class(),
            completions::snippets::interface(),
        ];
        // Using directives can only be added before any POU declarations
        if let Some(pou) = self.pous(db).first() {
            if pou.get_span(db).end_byte >= offset {
                completions.push(completions::snippets::using());
            }
        } else {
            completions.push(completions::snippets::using());
        }
        Some(completions)
    }
}

impl<'db> IterToProto<'db> for Namespace<'db> {
    fn iter(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        let scope = sema.get_scope(self.scope_id(db));

        self_iter(self)
            .chain(scope.usings.iter().map(move |using| using as _))
            .chain(self.pous(db).iter().flat_map(move |pou| pou.iter(db, sema)))
    }
}
