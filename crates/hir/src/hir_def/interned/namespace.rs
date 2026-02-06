use crate::AstId;
use crate::builder::semantic_index::SemanticIndexBuilder;
use crate::check::errors::analysis_error::AnalysisError;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::scope::ScopeId;
use crate::{HirNodeInfo, hir_def::interned::identifier::SpanIdent};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::tracked::get_ast;
use db::WorkspaceDataBase;
use std::hash::Hash;
use std::ops::Deref;

// Namespace path with span information
#[derive(Debug, Clone, salsa::Update)]
pub struct SpanNamespacePath<'db> {
    pub scope_id: ScopeId<'db>,
    pub spans: Vec<AstId>,
    pub path: NamespacePath,
}

impl PartialEq for SpanNamespacePath<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for SpanNamespacePath<'_> {}

impl Hash for SpanNamespacePath<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.path.hash(state);
    }
}

impl Deref for SpanNamespacePath<'_> {
    type Target = NamespacePath;

    fn deref(&self) -> &Self::Target {
        &self.path
    }
}

impl<'db> SpanNamespacePath<'db> {
    pub fn concat(
        &self,
        db: &dyn WorkspaceDataBase,
        other: &'db SpanNamespacePath,
        scope_id: ScopeId<'db>,
    ) -> SpanNamespacePath<'db> {
        let mut fragments = self.path.fragments(db).to_owned();
        let mut spans = self.spans.clone();
        fragments.extend_from_slice(other.path.fragments(db));
        spans.extend_from_slice(&other.spans);
        SpanNamespacePath {
            scope_id,
            path: NamespacePath::new(db, fragments),
            spans,
        }
    }

    pub fn extend(
        &self,
        db: &dyn WorkspaceDataBase,
        ident: SpanIdent<'db>,
        scope_id: ScopeId<'db>,
    ) -> SpanNamespacePath<'db> {
        let mut path = self.path.fragments(db).to_owned();
        let mut spans = self.spans.clone();
        spans.push(ident.id);
        path.push(ident.ident);
        SpanNamespacePath {
            scope_id,
            path: NamespacePath::new(db, path),
            spans: self.spans.clone(),
        }
    }

    pub fn to_string(&self, db: &dyn WorkspaceDataBase) -> String {
        self.path
            .fragments(db)
            .iter()
            .map(|i| i.text(db).to_string())
            .collect::<Vec<_>>()
            .join(".")
    }

    pub fn get_fragment_ast_node(
        &self,
        db: &'db dyn WorkspaceDataBase,
        index: usize,
    ) -> &'db Box<dyn AstNode> {
        &get_ast(db, self.scope_id.file(db))[self.spans[index].0]
    }
}

impl<'db> From<(&dyn WorkspaceDataBase, &SpanIdent<'db>)> for SpanNamespacePath<'db> {
    fn from(from: (&dyn WorkspaceDataBase, &SpanIdent<'db>)) -> Self {
        SpanNamespacePath {
            scope_id: from.1.scope_id,
            spans: vec![from.1.id],
            path: NamespacePath::new(from.0, vec![from.1.ident]),
        }
    }
}

impl<'db> From<(&dyn WorkspaceDataBase, &[SpanIdent<'db>], ScopeId<'db>)>
    for SpanNamespacePath<'db>
{
    fn from(from: (&dyn WorkspaceDataBase, &[SpanIdent<'db>], ScopeId<'db>)) -> Self {
        let mut spans = vec![];
        let mut idents = vec![];
        for ident in from.1.iter() {
            spans.push(ident.id);
            idents.push(ident.ident);
        }

        SpanNamespacePath {
            scope_id: from.2,
            spans,
            path: NamespacePath::new(from.0, idents),
        }
    }
}

impl<'db> From<(&dyn WorkspaceDataBase, Vec<SpanIdent<'db>>, ScopeId<'db>)>
    for SpanNamespacePath<'db>
{
    fn from(from: (&dyn WorkspaceDataBase, Vec<SpanIdent<'db>>, ScopeId<'db>)) -> Self {
        let mut spans = vec![];
        let mut idents = vec![];
        for ident in from.1.iter() {
            spans.push(ident.id);
            idents.push(ident.ident);
        }

        SpanNamespacePath {
            scope_id: from.2,
            spans,
            path: NamespacePath::new(from.0, idents),
        }
    }
}

impl<'db> From<(&dyn WorkspaceDataBase, &Vec<SpanIdent<'db>>, ScopeId<'db>)>
    for SpanNamespacePath<'db>
{
    fn from(from: (&dyn WorkspaceDataBase, &Vec<SpanIdent<'db>>, ScopeId<'db>)) -> Self {
        let mut spans = vec![];
        let mut idents = vec![];
        for ident in from.1.iter() {
            spans.push(ident.id);
            idents.push(ident.ident);
        }

        SpanNamespacePath {
            scope_id: from.2,
            spans,
            path: NamespacePath::new(from.0, idents),
        }
    }
}

/// Interned namespace path
#[salsa::interned(debug, no_lifetime)]
pub struct NamespacePath {
    #[returns(ref)]
    pub fragments: Vec<Ident>,
}

impl NamespacePath {
    pub fn to_string(&self, db: &dyn WorkspaceDataBase) -> String {
        self.fragments(db)
            .iter()
            .map(|i| i.text(db).to_string())
            .collect::<Vec<_>>()
            .join(".")
    }

    pub fn to_string_dotted(&self, db: &dyn WorkspaceDataBase) -> String {
        let mut result = self
            .fragments(db)
            .iter()
            .map(|i| i.text(db).to_string())
            .collect::<Vec<_>>()
            .join(".");
        if self.fragments(db).len() == 1 {
            result.push('.');
        }
        result
    }
}

impl From<(&dyn WorkspaceDataBase, &Ident)> for NamespacePath {
    fn from(from: (&dyn WorkspaceDataBase, &Ident)) -> Self {
        NamespacePath::new(from.0, vec![*from.1])
    }
}

impl From<(&dyn WorkspaceDataBase, &[Ident])> for NamespacePath {
    fn from(from: (&dyn WorkspaceDataBase, &[Ident])) -> Self {
        NamespacePath::new(from.0, from.1.to_vec())
    }
}

impl From<(&dyn WorkspaceDataBase, Vec<Ident>)> for NamespacePath {
    fn from(from: (&dyn WorkspaceDataBase, Vec<Ident>)) -> Self {
        NamespacePath::new(from.0, from.1)
    }
}

impl From<(&dyn WorkspaceDataBase, &Vec<Ident>)> for NamespacePath {
    fn from(from: (&dyn WorkspaceDataBase, &Vec<Ident>)) -> Self {
        NamespacePath::new(from.0, from.1.clone())
    }
}

/// A [`SpannedPath`] is a wrapper around a [`NamespaceAccess`] that includes a span
#[derive(Debug, Clone, salsa::Update)]
pub struct SpanNamespaceAccess<'db> {
    pub id: AstId,
    pub scope_id: ScopeId<'db>,
    pub path: NamespaceAccess<'db>,
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

impl<'db> HirNodeInfo<'db> for SpanNamespaceAccess<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id
    }
}

impl Eq for SpanNamespaceAccess<'_> {}

impl<'db> SpanNamespaceAccess<'db> {
    pub fn from_ast(
        db: &'db dyn WorkspaceDataBase,
        sema: &SemanticIndexBuilder<'db>,
        fq_name: &ast::generated::NamespaceAccess,
    ) -> anyhow::Result<Self, AnalysisError<'db>> {
        Ok(SpanNamespaceAccess {
            id: fq_name.into(),
            scope_id: sema.current_scope,
            path: NamespaceAccess::from_ast(db, sema, fq_name)?,
        })
    }

    pub fn to_string(&self, db: &dyn WorkspaceDataBase) -> String {
        self.path.to_string(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct NamespaceAccess<'db> {
    pub namespace: Option<SpanNamespacePath<'db>>,
    pub target: SpanIdent<'db>,
}

impl<'db> NamespaceAccess<'db> {
    pub fn new(
        db: &'db dyn WorkspaceDataBase,
        namespace: Option<SpanNamespacePath<'db>>,
        target: SpanIdent<'db>,
    ) -> Self {
        Self { namespace, target }
    }
    pub fn from_ast(
        db: &'db dyn WorkspaceDataBase,
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
            Some(SpanNamespacePath::from((db, fragments, sema.current_scope))),
            target,
        ))
    }

    pub fn to_string(&self, db: &dyn WorkspaceDataBase) -> String {
        let mut path = self
            .namespace
            .as_ref()
            .map(|ns| ns.to_string(db))
            .unwrap_or_default();
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(self.target.ident.text(db).as_str());
        path
    }
}
