use auto_lsp::{
    core::span::Span,
    default::db::{BaseDatabase},
    lsp_types::{MarkupContent, MarkupKind},
};

use crate::{
    hir::{
        expressions::{expression::InitExpr, spec::Spec},
        interned::identifier::Ident,
        scope::FileScopeId,
        semantic_index::{semantic_index, SemanticIndex},
    },
    to_proto::{self_iter, AstId, IterToProto, SymbolInfo, ToProto},
};

#[salsa::tracked(debug)]
pub struct Variable<'db> {
    #[returns(ref)]
    pub name: Ident,

    #[tracked]
    pub kind: VariableKind,

    #[tracked]
    #[returns(ref)]
    pub spec: Spec<'db>,

    #[tracked]
    #[returns(as_ref)]
    pub init: Option<InitExpr<'db>>,

    pub id: AstId,

    pub name_id: AstId,

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
    fn get_id(&'db self, db: &'db dyn crate::BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&'db self, db: &'db dyn crate::BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }

    fn get_name_span(&'db self, db: &'db dyn crate::BaseDatabase) -> Option<&'db Span> {
        let file = self.get_scope_id(db).file();
        semantic_index(db, file).span_map.get(&self.name_id(db).0)
    }

    fn symbol_info(&'db self, db: &'db dyn BaseDatabase) -> Option<SymbolInfo<'db>> {
        Some(
            SymbolInfo::builder()
                .kind(auto_lsp::lsp_types::SymbolKind::VARIABLE)
                .name(self.name(db).text(db).to_string())
                .range(self.get_span(db).clone())
                .name_range(self.get_name_span(db).unwrap().clone())
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
            range: self.get_name_span(db).map(|s| s.lsp()),
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
