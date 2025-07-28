use auto_lsp::{
    core::ast::AstNode, default::db::{file::File}
};
use bitflags::bitflags;

use crate::hir::{using::Using};


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::Update)]
pub struct NamespaceId(pub(crate) usize);

impl<T: AstNode> From<&T> for NamespaceId {
    fn from(node: &T) -> Self {
        NamespaceId(node.get_id())
    }
}

impl From<usize> for NamespaceId {
    fn from(node: usize) -> Self {
        NamespaceId(node)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, salsa::Update)]
pub struct PouId(pub(crate) usize);

impl<T: AstNode> From<&T> for PouId {
    fn from(node: &T) -> Self {
        PouId(node.get_id())
    }
}

impl From<usize> for PouId {
    fn from(node: usize) -> Self {
        PouId(node)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ScopedNamespaceId(pub NamespaceId, pub File);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct ScopedPouId(pub PouId, pub File);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub struct ScopeId(usize);

impl From<usize> for ScopeId {
    fn from(id: usize) -> Self {
        ScopeId(id)
    }
}

impl ScopeId {
    #[inline]
    pub const fn global() -> Self {
        ScopeId(usize::MAX)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Scope<'db> {
    pub file: File,

    // from the standard: "A USING namespace directive enables the types contained in the given namespace,
    // but specifically does not enable types contained in nested namespaces."

    // TLDR: Using directives are not recursive
    pub usings: Vec<Using<'db>>,

    pub kind: ScopeKind,

    pub id: ScopeId,

    // If None, this is the global scope
    pub parent: Option<ScopeId>,

    pub visibility: Visibility,
}

#[salsa::tracked]
impl<'db> Scope<'db> {
    pub fn new(
        file: File,
        kind: ScopeKind,
        usings: Vec<Using<'db>>,
        id: ScopeId,
        visibility: Visibility,
        parent: Option<ScopeId>,
    ) -> Self {
        Self {
            file,
            usings,
            kind,
            visibility,
            id,
            parent,
        }
    }

    pub fn is_global(&self) -> bool {
        matches!(self.kind, ScopeKind::Global)
    }

    pub fn is_namespace(&self) -> bool {
        matches!(self.kind, ScopeKind::Namespace(_))
    }

    pub fn is_pou(&self) -> bool {
        matches!(self.kind, ScopeKind::Pou(_))
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Visibility: u16 {
        const PUBLIC = 1 << 0;
        const PROTECTED = 1 << 1;
        const INTERNAL = 1 << 2;
        const PRIVATE = 1 << 3;
    }
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::PUBLIC
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    Global,
    Namespace(NamespaceId),
    Pou(PouId),
}
