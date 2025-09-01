use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::sem_errors::AnalysisError;
use crate::def::scope::FileScopeId;
use crate::to_proto::AstId;
use crate::{def::interned::identifier::SpanIdent, to_proto::ToProto};
use auto_lsp::default::db::tracked::get_ast;
use auto_lsp::{anyhow, default::db::BaseDatabase};
use std::hash::Hash;

/// Interned namespace path
#[salsa::interned(debug, no_lifetime)]
pub struct NamespacePath {
    #[returns(ref)]
    pub fragments: Vec<SpanIdent<'db>>,
}

impl NamespacePath {
    pub fn concat(&self, db: &dyn BaseDatabase, other: &NamespacePath) -> NamespacePath {
        let mut path = self.fragments(db).to_owned();
        path.extend_from_slice(other.fragments(db));
        NamespacePath::new(db, path)
    }

    pub fn extend(&self, db: &dyn BaseDatabase, ident: SpanIdent) -> NamespacePath {
        let mut path = self.fragments(db).to_owned();
        path.push(ident);
        NamespacePath::new(db, path)
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        self.fragments(db)
            .iter()
            .map(|i| i.ident.text(db).to_string())
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl From<(&dyn BaseDatabase, &SpanIdent<'_>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &SpanIdent<'_>)) -> Self {
        NamespacePath::new(from.0, vec![from.1.clone()])
    }
}

impl From<(&dyn BaseDatabase, &[SpanIdent<'_>])> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &[SpanIdent<'_>])) -> Self {
        NamespacePath::new(from.0, from.1.to_vec())
    }
}

impl From<(&dyn BaseDatabase, Vec<SpanIdent<'_>>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, Vec<SpanIdent<'_>>)) -> Self {
        NamespacePath::new(from.0, from.1)
    }
}

impl From<(&dyn BaseDatabase, &Vec<SpanIdent<'_>>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &Vec<SpanIdent<'_>>)) -> Self {
        NamespacePath::new(from.0, from.1.clone())
    }
}

/// A [`SpannedPath`] is a wrapper around a [`NamespaceAccess`] that includes a span
#[derive(Clone, salsa::Update, Debug)]
pub struct SpanNamespaceAccess<'db> {
    pub id: AstId,
    pub scope_id: FileScopeId<'db>,
    pub path: NamespaceAccess,
}

impl PartialEq for SpanNamespaceAccess<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Hash for SpanNamespaceAccess<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl<'db> ToProto<'db> for SpanNamespaceAccess<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id
    }
}

impl Eq for SpanNamespaceAccess<'_> {}

impl<'db> SpanNamespaceAccess<'db> {
    pub fn from_ast(
        db: &'db dyn BaseDatabase,
        sema: &SemanticIndexBuilder<'db>,
        fq_name: &ast::generated::NamespaceAccess,
    ) -> anyhow::Result<Self, AnalysisError<'db>> {
        Ok(SpanNamespaceAccess {
            id: fq_name.into(),
            scope_id: sema.current_scope,
            path: NamespaceAccess::from_ast(db, sema, fq_name)?,
        })
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        self.path.to_string(db)
    }
}

#[salsa::interned(debug, no_lifetime)]
pub struct NamespaceAccess {
    pub namespace: Option<NamespacePath>,
    pub target: SpanIdent<'db>,
}

impl NamespaceAccess {
    pub fn from_ast<'db>(
        db: &'db dyn BaseDatabase,
        sema: &SemanticIndexBuilder<'db>,
        fq_name: &ast::generated::NamespaceAccess,
    ) -> anyhow::Result<Self, AnalysisError<'db>> {
        let ast = get_ast(db, sema.file);
        let mut fragments = Vec::new();

        // Walk the tree from root down `.path` fields
        let mut current = match fq_name.children.cast(ast) {
            ast::generated::Identifier_ScopedIdentifier::ScopedIdentifier(scoped) => scoped,
            ast::generated::Identifier_ScopedIdentifier::Identifier(ident) => {
                let target = SpanIdent::from_node(db, sema, ident)?;
                return Ok(NamespaceAccess::new(db, None, target));
            }
        };

        loop {
            // Extract the target of the current scoped_identifier (e.g. m1, m2, m3...)
            fragments.push(SpanIdent::new(db, sema, &current.target)?);

            match current.path.cast(ast) {
                ast::generated::Identifier_ScopedIdentifier::ScopedIdentifier(next) => {
                    current = next;
                }
                ast::generated::Identifier_ScopedIdentifier::Identifier(base) => {
                    // Reached the bottom-most path (e.g. "system")
                    fragments.push(SpanIdent::from_node(db, sema, base)?);
                    break;
                }
            }
        }

        fragments.reverse(); // Because we walked right-to-left

        // The last identifier pushed is the *rightmost*, so pop it off to become the target
        let target = fragments
            .pop()
            .expect("There must be at least one identifier");

        Ok(NamespaceAccess::new(
            db,
            Some(NamespacePath::from((db, fragments))),
            target,
        ))
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        let mut path = self
            .namespace(db)
            .map(|ns| ns.to_string(db))
            .unwrap_or_default();
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(self.target(db).ident.text(db).as_str());
        path
    }
}
