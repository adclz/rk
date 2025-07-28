use std::ops::Deref;

use auto_lsp::{anyhow, core::span::Span, default::db::{file::File, BaseDatabase}};
use auto_lsp::core::ast::AstNode;
use crate::{hir::interned::identifier::SpannedIdent, to_proto::ToProto};

/// Interned namespace path
#[salsa::interned(debug, no_lifetime)]
pub struct NamespacePath {
    #[returns(ref)]
    pub fragments: Vec<SpannedIdent>,
}

impl<'db> NamespacePath {
    pub fn concat(&self, db: &dyn BaseDatabase, other: &NamespacePath) -> NamespacePath {
        let mut path = self.fragments(db).to_owned();
        path.extend_from_slice(&other.fragments(db));
        NamespacePath::new(db, path)
    }

    pub fn extend(&self, db: &dyn BaseDatabase, ident: SpannedIdent) -> NamespacePath {
        let mut path = self.fragments(db).to_owned();
        path.push(ident);
        NamespacePath::new(db, path)
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        self.fragments(db)
            .iter()
            .map(|i| i.ident.text(db))
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl From<(&dyn BaseDatabase, &SpannedIdent)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &SpannedIdent)) -> Self {
        NamespacePath::new(from.0, vec![from.1.clone()])
    }
}

impl From<(&dyn BaseDatabase, &[SpannedIdent])> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &[SpannedIdent])) -> Self {
        NamespacePath::new(from.0, from.1.to_vec())
    }
}

impl From<(&dyn BaseDatabase, Vec<SpannedIdent>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, Vec<SpannedIdent>)) -> Self {
        NamespacePath::new(from.0, from.1)
    }
}

impl From<(&dyn BaseDatabase, &Vec<SpannedIdent>)> for NamespacePath {
    fn from(from: (&dyn BaseDatabase, &Vec<SpannedIdent>)) -> Self {
        NamespacePath::new(from.0, from.1.clone())
    }
}


/// A [`SpannedPath`] is a wrapper around a [`NamespaceAccess`] that includes a span
#[derive(Clone, Hash, salsa::Update, Debug)]
pub struct SpannedNamespaceAccess {
    pub span: Span,
    pub path: NamespaceAccess,
}

impl PartialEq for SpannedNamespaceAccess {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}
 
impl<'db> ToProto<'db> for SpannedNamespaceAccess {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        &self.span
    }
}

impl Eq for SpannedNamespaceAccess {}

impl SpannedNamespaceAccess {
    pub fn from_ast(
        db: &dyn BaseDatabase,
        file: File,
        fq_name: &ast::generated::NamespaceAccess,
    ) -> anyhow::Result<Self> {
        Ok(SpannedNamespaceAccess {
            span: fq_name.get_span(),
            path: NamespaceAccess::from_ast(db, file, &fq_name)?,
        })
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        self.path.to_string(db)
    }
}

/// A [`PathTarget`] represents a fully qualified path to something
/// 
/// If no namespace is present, it is a simple identifier
#[salsa::interned(debug, no_lifetime)]
pub struct NamespaceAccess {
    pub namespace: Option<NamespacePath>,
    pub target: SpannedIdent,
}

impl NamespaceAccess {
    pub fn from_ast(
        db: &dyn BaseDatabase,
        file: File,
        fq_name: &ast::generated::NamespaceAccess,
    ) -> anyhow::Result<Self> {
        let mut fragments = Vec::new();

        // Walk the tree from root down `.path` fields
        let mut current = match fq_name.children.deref() {
            ast::generated::Identifier_ScopedIdentifier::ScopedIdentifier(scoped) => scoped,
            ast::generated::Identifier_ScopedIdentifier::Identifier(ident) => {
                let target = SpannedIdent::new(db, file, ident)?;
                return Ok(NamespaceAccess::new(db, None, target));
            }
        };

        loop {
            // Extract the target of the current scoped_identifier (e.g. m1, m2, m3...)
            fragments.push(SpannedIdent::new(db, file, current.target.deref())?);

            match current.path.deref() {
                ast::generated::Identifier_ScopedIdentifier::ScopedIdentifier(next) => {
                    current = next;
                }
                ast::generated::Identifier_ScopedIdentifier::Identifier(base) => {
                    // Reached the bottom-most path (e.g. "system")
                    fragments.push(SpannedIdent::new(db, file, base)?);
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
            .unwrap_or_else(String::default);
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(self.target(db).ident.text(db).as_str());
        path
    }
}