use auto_lsp::{default::db::file::File};
use bitflags::bitflags;

use crate::hir::{namespace::Namespace, pous::pou::PouDecl, using::Using};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub struct FileScopeId((File, usize));

impl From<(File, usize)> for FileScopeId {
    fn from(data: (File, usize)) -> Self {
        FileScopeId(data)
    }
}

impl FileScopeId {
    #[inline]
    pub const fn global(file: File) -> Self {
        FileScopeId((file, usize::MAX))
    }

    pub fn is_global(&self) -> bool {
        self.0 .1 == usize::MAX
    }

    pub fn file(&self) -> File {
        self.0 .0
    }

    pub fn scope(&self) -> usize {
        self.0 .1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Scope<'db> {
    pub file: File,

    // from the standard: "A USING namespace directive enables the types contained in the given namespace,
    // but specifically does not enable types contained in nested namespaces."

    // TLDR: Using directives are not recursive
    pub usings: Vec<Using<'db>>,

    pub kind: ScopeKind<'db>,

    pub id: FileScopeId,

    // If None, this is the global scope
    pub parent: Option<FileScopeId>,

    pub visibility: Visibility,
}

impl<'db> Scope<'db> {
    pub fn new(
        file: File,
        kind: ScopeKind<'db>,
        usings: Vec<Using<'db>>,
        id: FileScopeId,
        visibility: Visibility,
        parent: Option<FileScopeId>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum ScopeKind<'db> {
    Global,
    Namespace(Namespace<'db>),
    Pou(PouDecl<'db>),
}
