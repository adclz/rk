use auto_lsp::{
    core::span::Span,
    default::db::{file::File, BaseDatabase},
    lsp_types::{MarkupContent, MarkupKind},
};

use crate::{
    hir::{
        expressions::{expression::Expr, spec::Spec}, interned::identifier::Ident,
        scopes::scope::{Scope, FileScopeId}, semantic_index::SemanticIndex,
    },
    to_proto::{self_iter, IterToProto, SymbolInfo, ToProto},
};

#[salsa::tracked(debug)]
pub struct Variable<'db> {
    pub file: File,

    #[returns(ref)]
    pub name: Ident,

    #[returns(ref)]
    pub range: Span,

    #[returns(ref)]
    pub name_span: Span,

    #[tracked]
    pub kind: VariableKind,

    #[tracked]
    #[returns(ref)]
    pub spec: Spec<'db>,

    #[tracked]
    #[returns(as_ref)]
    pub init: Option<Expr<'db>>,

    pub scope_id: FileScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VariableKind {
    Input,
    Output,
    InOut,
    Temp,
    Local,
    External,
    Global,
    Retain,
    NoRetain,
    LocPartly,
}

impl<'db> ToProto<'db> for Variable<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.range(db)
    }

    fn get_named_span(&'db self, db: &'db dyn crate::BaseDatabase) -> Option<&'db Span> {
        Some(self.name_span(db))
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::VARIABLE)
                .name(self.name(db).text(db).to_string())
                .range(self.range(db).clone())
                .name_range(self.name_span(db).clone())
                .spec(*self.spec(db))
                .maybe_init(self.init(db).cloned())
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
                value: format!("Variable {}", self.name(db).text(db)).to_string(),
            }),
            range: Some(self.name_span(db).into()),
        })
    }
}

impl<'db> IterToProto<'db> for Variable<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self_iter(self)
            .chain(self.spec(db).iter(db, sema))
            .chain(self.init(db).into_iter().map(|i| i as _))
    }
}
