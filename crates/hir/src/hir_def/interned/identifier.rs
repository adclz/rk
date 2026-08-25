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

/// An identifier reduced to the form names are COMPARED in.
///
/// IEC 61131-3 §6.1.2: case is not significant in identifiers, so `Motor` and
/// `motor` are one name. The fold lives in the KEY rather than in the
/// comparison: every lookup map is keyed by this, so resolution stays a single
/// interned-id compare instead of a string walk.
///
/// It is a distinct TYPE on purpose. A map keyed by `FoldedIdent` cannot be
/// queried with an `Ident`, so every lookup site has to fold and the compiler
/// says which ones — the alternative is a discipline nobody can enforce, where
/// the site you forget stays case-sensitive and answers wrongly in silence.
///
/// The spelling the author wrote is NOT this: it stays on the `Ident`, which
/// is what diagnostics, hover, completion and the wasm export names read.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, salsa::Update)]
pub struct FoldedIdent(Ident);

#[salsa::tracked]
impl Ident {
    /// This identifier as names are compared.
    ///
    /// `to_lowercase`, not `to_ascii_lowercase`: identifiers are Unicode here
    /// (the grammar admits `XID_Start`/`XID_Continue`), so an ASCII fold would
    /// leave `MÄX` and `mäx` as two names.
    #[salsa::tracked]
    pub fn fold(self, db: &dyn WorkspaceDataBase) -> FoldedIdent {
        let text = self.text(db);
        // The overwhelmingly common case is already folded, and interning the
        // same bytes back is cheaper than allocating a copy of them.
        if text.chars().all(|c| !c.is_uppercase()) {
            return FoldedIdent(self);
        }
        FoldedIdent(Ident::new(db, text.to_lowercase()))
    }
}

impl FoldedIdent {
    /// The folded text itself, for the byte-oriented indexes (`fst`) that
    /// cannot hold an interned id.
    pub fn text(self, db: &dyn WorkspaceDataBase) -> &CompactString {
        self.0.text(db)
    }

    /// The folded spelling as a plain [`Ident`], for composite interned keys
    /// (namespace paths) whose element type has to stay `Ident`.
    pub fn as_ident(self) -> Ident {
        self.0
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
