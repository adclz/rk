use auto_lsp::{core::document_symbols_builder::DocumentSymbolsBuilder, default::db::BaseDatabase, lsp_types::SymbolKind};

use crate::{
    hir_def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        modifier::Modifier,
        pous::variable::VariableDecl,
        scope::FileScopeId,
        visibility::Visibility,
    },
    to_proto::{AstId, ToProto},
};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    #[tracked]
    #[returns(as_ref)]
    pub extends: Option<SpanNamespaceAccess<'db>>,

    #[tracked]
    #[returns(ref)]
    pub implements: Vec<SpanNamespaceAccess<'db>>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    pub methods: Vec<MethodDecl<'db>>,

    pub modifier: Modifier,

    pub scope_id: FileScopeId<'db>,
}

#[salsa::tracked(debug)]
pub struct MethodDecl<'db> {
    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    pub name: Ident,

    #[tracked]
    #[returns(ref)]
    pub return_type: Option<Spec<'db>>,

    pub modifier: Modifier,

    pub _override: bool,

    pub body: Vec<Stmt<'db>>,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> ToProto<'db> for MethodDecl<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }

    fn document_symbols(&self, db: &'db dyn BaseDatabase, builder: &mut DocumentSymbolsBuilder) {
    let mut nested_builder = DocumentSymbolsBuilder::default();
    self
        .variables(db)
        .iter()
        .for_each(|var| var.document_symbols(db, &mut nested_builder));

    builder.push_symbol(auto_lsp::lsp_types::DocumentSymbol {
        name: self.name(db).text(db).to_string(),
        detail: Some("method".to_string()),
        kind: SymbolKind::METHOD,
        deprecated: None,
        range: self.get_span(db).lsp(),
        selection_range: self.get_name_span(db).unwrap().lsp(),
        children: Some(nested_builder.finalize()),
        tags: None,
    });
    }
}
