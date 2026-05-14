use auto_lsp::{
    anyhow,
    core::ast::{AstNode, AstNodeId},
    default::db::{file::File, tracked::get_ast},
};
use compact_str::CompactString;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use std::{hash::Hash, ops::Deref};

use crate::{
    builder::semantic_index::SemanticIndexBuilder,
    check::errors::{ToIdeDiagnostic, e0_syntax::SyntaxError},
    hir_def::scope::ScopeId,
    {AstId, HirNodeInfo},
};

#[derive(Clone, Copy, Eq, salsa::Update, Debug)]
pub struct SpanIdent<'db> {
    pub id: AstId,
    pub scope_id: ScopeId<'db>,
    pub ident: Ident,
}

impl Deref for SpanIdent<'_> {
    type Target = Ident;

    fn deref(&self) -> &Self::Target {
        &self.ident
    }
}

impl PartialEq for SpanIdent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.ident == other.ident
    }
}

impl PartialEq<Ident> for SpanIdent<'_> {
    fn eq(&self, other: &Ident) -> bool {
        self.ident == *other
    }
}

impl Hash for SpanIdent<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ident.hash(state);
    }
}

impl<'db> SpanIdent<'db> {
    pub fn new<T: AstNode>(
        db: &'db dyn WorkspaceDataBase,
        sema: &SemanticIndexBuilder<'db>,
        ident: &AstNodeId<T>,
    ) -> anyhow::Result<Self, IdeDiagnostic> {
        let ast = get_ast(db, sema.file);
        let ident = ident.cast(ast);
        Self::from_node(db, sema, ident)
    }

    pub fn from_node(
        db: &'db dyn WorkspaceDataBase,
        sema: &SemanticIndexBuilder<'db>,
        node: &impl AstNode,
    ) -> anyhow::Result<Self, IdeDiagnostic> {
        Ok(SpanIdent {
            id: node.into(),
            scope_id: sema.current_scope,
            ident: Ident::from_node(db, sema.file, node)?,
        })
    }

    pub fn as_str(&'db self, db: &'db dyn WorkspaceDataBase) -> &'db str {
        self.ident.text(db)
    }
}

impl<'db> HirNodeInfo<'db> for SpanIdent<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id
    }
}

/// Interned identifier
#[salsa::interned(debug, no_lifetime)]
#[derive(PartialOrd, Ord)]
pub struct Ident {
    #[returns(ref)]
    pub text: CompactString,
}

impl Ident {
    pub fn from_node(
        db: &dyn WorkspaceDataBase,
        file: File,
        node: &impl AstNode,
    ) -> anyhow::Result<Self, IdeDiagnostic> {
        Ok(Ident::new(
            db,
            CompactString::from(node.get_text(file.document(db).as_bytes()).map_err(|e| {
                SyntaxError::SyntaxError {
                    span: node.get_span(),
                    err: e.to_string(),
                }
                .to_diagnostic(db)
            })?),
        ))
    }

    pub fn from_slice(db: &dyn WorkspaceDataBase, text: &str) -> Self {
        Ident::new(db, CompactString::from(text))
    }

    pub fn join(db: &dyn WorkspaceDataBase, other: &[Ident]) -> Ident {
        Ident::new(
            db,
            other
                .iter()
                .map(|i| i.text(db).to_owned())
                .collect::<CompactString>(),
        )
    }
}
