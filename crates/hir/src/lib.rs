#![recursion_limit = "256"]
#![allow(unused_variables)]

use std::ops::Deref;

use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::file::File;
use auto_lsp::{lsp_types, tree_sitter};
use bitflags::bitflags;
use compact_str::CompactString;
use db::WorkspaceDataBase;

/// Converts a raw tree-sitter [`tree_sitter::Range`] into an LSP range, adjusting columns to the
/// client's negotiated encoding via the document of `file`.
///
/// Returns `None` if the range cannot be denormalized (out of bounds).
pub fn denormalize(
    db: &dyn WorkspaceDataBase,
    file: impl std::borrow::Borrow<File>,
    range: &tree_sitter::Range,
) -> Option<lsp_types::Range> {
    file.borrow().document(db).denormalize_range(range).ok()
}

use crate::hir_def::{
    interned::identifier::{Ident, SpanIdent},
    pous::pragma::{ExternPragma, Pragma},
    scope::ScopeId,
    semantic_index::semantic_index,
};

pub mod builder;
pub mod check;
pub mod hir_def;
pub mod hir_ty;
pub mod query_string;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct AstId(pub(crate) usize);

impl<T: AstNode> From<&T> for AstId {
    fn from(node: &T) -> Self {
        AstId(node.get_id())
    }
}

impl AstId {
    pub fn id(&self) -> usize {
        self.0
    }
}

impl Deref for AstId {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct CallSite<'db> {
    pub scope: ScopeId<'db>,
    pub id: AstId,
}

impl<'db> CallSite<'db> {
    pub fn new(scope: ScopeId<'db>, id: AstId) -> Self {
        Self { scope, id }
    }

    pub fn from_scoped(db: &'db dyn WorkspaceDataBase, scope: &impl HirNodeInfo<'db>) -> Self {
        Self {
            scope: scope.get_scope_id(db),
            id: scope.get_id(db),
        }
    }

    pub fn to_string(&self, db: &'db dyn WorkspaceDataBase) -> CompactString {
        let file = self.scope.file(db);
        let node = semantic_index(db, file)
            .ast
            .get(self.id.0)
            .unwrap_or_else(|| panic!("Invalid ID {} when attempting to retrieve text", self.id.0));
        CompactString::from(
            node.get_text(file.document(db).as_bytes())
                .expect("Failed to get text"),
        )
    }
}

impl<'db> HirNodeInfo<'db> for CallSite<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope
    }
}

/// Core trait for retrieving information about HIR nodes, used as a bound for all HIR node types.
pub trait HirNodeInfo<'db> {
    /// Retrieves the AST ID corresponding to this HIR node.
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId;

    /// Retrieves the scope ID corresponding to this HIR node.
    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db>;

    /// Converts this HIR node into a [`CallSite`], which can be used for diagnostics and other operations that require both the node's ID and its scope.
    fn as_call_site(&self, db: &'db dyn WorkspaceDataBase) -> CallSite<'db> {
        CallSite {
            scope: self.get_scope_id(db),
            id: self.get_id(db),
        }
    }

    /// Returns the raw tree-sitter range (UTF-8 byte columns) of this node.
    ///
    /// Convert to an encoding-adjusted LSP range at the boundary with [`denormalize`].
    fn get_span(&self, db: &'db dyn WorkspaceDataBase) -> tree_sitter::Range {
        *semantic_index(db, self.get_scope_id(db).file(db))
            .ast
            .get(self.get_id(db).0)
            .unwrap_or_else(|| {
                panic!(
                    "invalid ID {} when attempting to retrieve span",
                    *self.get_id(db)
                )
            })
            .get_range()
    }
}

pub trait HasPragmas<'db>: HirNodeInfo<'db> {
    fn get_pragmas(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> &'db [hir_def::pous::pragma::Pragma<'db>];

    fn test_pragma(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<&'db hir_def::interned::identifier::SpanIdent<'db>> {
        self.get_pragmas(db).iter().find_map(|p| match p {
            hir_def::pous::pragma::Pragma::Test(s) => Some(s),
            _ => None,
        })
    }

    fn once_pragma(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<&'db hir_def::interned::identifier::SpanIdent<'db>> {
        self.get_pragmas(db).iter().find_map(|p| match p {
            hir_def::pous::pragma::Pragma::Once(s) => Some(s),
            _ => None,
        })
    }

    fn warn_pragma(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<(
        &'db hir_def::interned::identifier::SpanIdent<'db>,
        &'db hir_def::pous::pragma::WarnPragma,
    )> {
        self.get_pragmas(db).iter().find_map(|p| match p {
            hir_def::pous::pragma::Pragma::Warn(s, w) => Some((s, w)),
            _ => None,
        })
    }

    fn is_test(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.test_pragma(db).is_some()
    }

    fn is_once(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.once_pragma(db).is_some()
    }

    /// `{extern 'module' 'name'}` - this POU is a WASM import. Only legal on
    /// a FUNCTION (checked in `check_variables`); the accessor exists on the
    /// trait so both the check and MIR read the SAME fact.
    fn extern_pragma(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<(
        &'db SpanIdent<'db>,
        &'db ExternPragma,
    )> {
        self.get_pragmas(db).iter().find_map(|p| match p {
            Pragma::Extern(s, e) => Some((s, e)),
            _ => None,
        })
    }
}

pub trait HasName<'db>: HirNodeInfo<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident;

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId;

    /// Returns the raw tree-sitter range of this node's name.
    fn get_name_span(&'db self, db: &'db dyn WorkspaceDataBase) -> tree_sitter::Range {
        *semantic_index(db, self.get_scope_id(db).file(db))
            .ast
            .get(self.get_name_id(db).0)
            .unwrap_or_else(|| {
                panic!(
                    "invalid name ID {} when attempting to retrieve name span",
                    *self.get_name_id(db)
                )
            })
            .get_range()
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Modifier: u16 {
        const ABSTRACT = 1 << 0;
        const FINAL = 1 << 1;
        const OVERRIDE = 1 << 2;
    }
}

impl Modifier {
    pub const EMPTY: Modifier = Modifier::empty();
}

pub trait HasModifiers<'db>: HirNodeInfo<'db> {
    fn get_modifiers(&self, db: &'db dyn WorkspaceDataBase) -> Modifier;
}

bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Visibility: u16 {
        const PUBLIC = 1 << 0;
        const PROTECTED = 1 << 1;
        const INTERNAL = 1 << 2;
        const PRIVATE = 1 << 3;
    }
}

impl Visibility {
    pub const EMPTY: Visibility = Visibility::empty();
}

pub trait HasVisibility<'db>: HirNodeInfo<'db> {
    fn get_visibility(&self, db: &'db dyn WorkspaceDataBase) -> Visibility;
}

bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Qualifier: u16 {
        const CONSTANT = 1 << 0;
        const RETAIN = 1 << 1;
        const NON_RETAIN = 1 << 2;
    }
}

pub trait HasQualifiers<'db>: HirNodeInfo<'db> {
    fn get_qualifiers(&self, db: &'db dyn WorkspaceDataBase) -> Qualifier;
}
