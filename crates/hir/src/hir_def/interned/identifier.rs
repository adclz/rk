// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::{
    anyhow,
    core::ast::{AstNode, AstNodeId},
    default::db::file::File,
};
use compact_str::CompactString;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use std::hash::Hash;

use crate::{
    builder::semantic_index::SemanticIndexBuilder,
    check::errors::{ToIdeDiagnostic, e00_syntax::SyntaxError},
    hir_def::scope::ScopeId,
    {AstId, HirNodeInfo},
};

/// One occurrence of a name in the source.
#[derive(Clone, Copy, Eq, salsa::Update, Debug)]
pub struct SpanIdent<'db> {
    pub id: AstId,
    pub scope_id: ScopeId<'db>,
    /// The name as the author wrote it here. Matching reads [`Self::ident`].
    pub with_case: Ident,
}

// Equal when the name is written the same, wherever: an occurrence that only
// moved is not a change.
impl PartialEq for SpanIdent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.with_case == other.with_case
    }
}

impl Hash for SpanIdent<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.with_case.hash(state);
    }
}

impl<'db> SpanIdent<'db> {
    pub fn new<T: AstNode>(
        db: &'db dyn WorkspaceDataBase,
        sema: &SemanticIndexBuilder<'db>,
        ident: &AstNodeId<T>,
    ) -> anyhow::Result<Self, IdeDiagnostic> {
        let ast = &db::syntax::parse(db, sema.file).ast;
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
            with_case: Ident::from_node(db, sema.file, node)?,
        })
    }

    /// The name, as names are matched: case folded.
    pub fn ident(&self, db: &dyn WorkspaceDataBase) -> Ident {
        self.with_case.folded(db)
    }

    /// The name as written here, for what is shown.
    pub fn as_str(&'db self, db: &'db dyn WorkspaceDataBase) -> &'db str {
        self.with_case.text(db)
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

#[salsa::tracked]
impl Ident {
    /// This identifier with case out of the way: how names are matched
    /// (IEC 61131-3 §6.1.2, Unicode caseless matching, §3.13). The accessors
    /// that hand out a name — `name(db)`, `get_name_ident`, `SpanIdent::ident`
    /// — return it through here, so code that compares or looks up names
    /// never folds; the spelling stays reachable as `*_with_case`.
    ///
    /// `to_lowercase`, not `to_ascii_lowercase`: identifiers are Unicode here
    /// (the grammar admits `XID_Start`/`XID_Continue`), so an ASCII fold would
    /// leave `MÄX` and `mäx` as two names.
    #[salsa::tracked]
    pub fn folded(self, db: &dyn WorkspaceDataBase) -> Ident {
        let text = self.text(db);
        // The overwhelmingly common case is already folded, and interning the
        // same bytes back is cheaper than allocating a copy of them.
        if text.chars().all(|c| !c.is_uppercase()) {
            return self;
        }
        Ident::new(db, text.to_lowercase())
    }
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
                    span: node.get_range().to_owned(),
                    err: e.to_string(),
                }
                .to_diagnostic(db, file)
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
