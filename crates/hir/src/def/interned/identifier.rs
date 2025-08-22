use auto_lsp::{
    anyhow,
    core::ast::{AstNode, AstNodeId},
    default::db::{BaseDatabase, file::File, tracked::get_ast},
};
use compact_str::CompactString;
use std::{hash::Hash, ops::Deref};

use crate::{
    builder::semantic_index::SemanticIndexBuilder,
    check::errors::sem_errors::AnalysisError,
    def::scope::FileScopeId,
    to_proto::{AstId, ToProto},
};

#[derive(Clone, Eq, salsa::Update, Debug)]
pub struct SpannedIdent {
    pub id: AstId,
    pub scope_id: FileScopeId,
    pub ident: Ident,
}

impl Deref for SpannedIdent {
    type Target = Ident;

    fn deref(&self) -> &Self::Target {
        &self.ident
    }
}

impl PartialEq for SpannedIdent {
    fn eq(&self, other: &Self) -> bool {
        self.ident == other.ident
    }
}

impl PartialEq<Ident> for SpannedIdent {
    fn eq(&self, other: &Ident) -> bool {
        self.ident == *other
    }
}

impl Hash for SpannedIdent {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ident.hash(state);
    }
}

impl<'db> SpannedIdent {
    pub fn new<T: AstNode>(
        db: &'db dyn BaseDatabase,
        sema: &SemanticIndexBuilder,
        ident: &AstNodeId<T>,
    ) -> anyhow::Result<Self, AnalysisError<'db>> {
        let ast = get_ast(db, sema.file);
        let ident = ident.cast(ast);
        Self::from_node(db, sema, ident)
    }

    pub fn from_node(
        db: &'db dyn BaseDatabase,
        sema: &SemanticIndexBuilder,
        node: &impl AstNode,
    ) -> anyhow::Result<Self, AnalysisError<'db>> {
        Ok(SpannedIdent {
            id: node.into(),
            scope_id: sema.current_scope,
            ident: Ident::from_node(db, sema.file, node)?,
        })
    }

    pub fn to_string(&'db self, db: &'db dyn BaseDatabase) -> &'db str {
        self.ident.text(db)
    }
}

impl<'db> ToProto<'db> for SpannedIdent {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.scope_id
    }
}

/// Interned identifier
#[salsa::interned(debug, no_lifetime)]
pub struct Ident {
    #[returns(ref)]
    pub text: CompactString,
}

impl<'db> Ident {
    pub fn from_node(
        db: &dyn BaseDatabase,
        file: File,
        node: &impl AstNode,
    ) -> anyhow::Result<Self, AnalysisError<'db>> {
        Ok(Ident::new(
            db,
            CompactString::from(node.get_text(file.document(db).as_bytes())?),
        ))
    }

    pub fn from_slice(db: &dyn BaseDatabase, text: &str) -> Self {
        Ident::new(db, CompactString::from(text))
    }

    pub fn join(db: &dyn BaseDatabase, other: &[Ident]) -> Ident {
        Ident::new(
            db,
            other
                .iter()
                .map(|i| i.text(db).to_owned())
                .collect::<CompactString>(),
        )
    }
}
