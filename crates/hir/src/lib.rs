#![recursion_limit = "256"]
#![allow(unused_variables)]

use auto_lsp::{
    core::{ast::AstNode, span::Span},
    default::db::BaseDatabase,
};
use bitflags::bitflags;
use compact_str::CompactString;

use crate::hir_def::{
    expressions::{
        expression::{Expr, InitExpr, VariableAccess},
        spec::Spec,
    },
    interned::identifier::Ident,
    pous::variable::VariableDecl,
    scope::ScopeId,
    semantic_index::semantic_index,
};

pub mod builder;
pub mod check;
pub mod hir_def;
pub mod hir_ty;
pub mod query_string;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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

pub trait MyTrait {
    fn example_method(&self);
}

impl MyTrait for Vec<u8> {
    fn example_method(&self) {
        // Implementation goes here
    }
}

pub struct WrapperVec(pub Vec<u8>);

impl MyTrait for WrapperVec {
    fn example_method(&self) {
        self.0.example_method();
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

    pub fn from_expr(db: &'db dyn BaseDatabase, expr: Expr<'db>) -> Self {
        Self {
            scope: expr.get_scope_id(db),
            id: expr.get_id(db),
        }
    }

    pub fn from_init_expr(db: &'db dyn BaseDatabase, expr: InitExpr<'db>) -> Self {
        Self {
            scope: expr.get_scope_id(db),
            id: expr.get_id(db),
        }
    }

    pub fn from_var_access(db: &'db dyn BaseDatabase, var_access: VariableAccess<'db>) -> Self {
        Self {
            scope: var_access.get_scope_id(db),
            id: var_access.get_id(db),
        }
    }

    pub fn from_var_decl(db: &'db dyn BaseDatabase, var_access: VariableDecl<'db>) -> Self {
        Self {
            scope: var_access.get_scope_id(db),
            id: var_access.get_id(db),
        }
    }

    pub fn from_spec(db: &'db dyn BaseDatabase, spec: Spec<'db>) -> Self {
        Self {
            scope: spec.get_scope_id(db),
            id: spec.get_id(db),
        }
    }

    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> CompactString {
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
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope
    }
}

pub trait HirNodeInfo<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId;

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db>;

    fn as_call_site(&self, db: &'db dyn BaseDatabase) -> CallSite<'db> {
        CallSite {
            scope: self.get_scope_id(db),
            id: self.get_id(db),
        }
    }

    fn get_span(&self, db: &'db dyn BaseDatabase) -> Span {
        semantic_index(db, self.get_scope_id(db).file(db))
            .ast
            .get(self.get_id(db).0)
            .unwrap_or_else(|| {
                panic!(
                    "Invalid ID {} when attempting to retrieve span",
                    self.get_id(db).0
                )
            })
            .get_span()
    }
}

pub trait HasName<'db>: HirNodeInfo<'db> {
    fn get_name_ident(&self, db: &'db dyn BaseDatabase) -> Ident;

    fn get_name_id(&self, db: &'db dyn BaseDatabase) -> AstId;

    fn get_name_span(&'db self, db: &'db dyn BaseDatabase) -> Span {
        semantic_index(db, self.get_scope_id(db).file(db))
            .ast
            .get(self.get_name_id(db).0)
            .unwrap_or_else(|| {
                panic!(
                    "Invalid name ID {} when attempting to retrieve name span",
                    self.get_name_id(db).0
                )
            })
            .get_span()
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
    fn get_modifiers(&self, db: &'db dyn BaseDatabase) -> Modifier;
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

impl<'db> Visibility {
    pub const EMPTY: Visibility = Visibility::empty();
}

pub trait HasVisibility<'db>: HirNodeInfo<'db> {
    fn get_visibility(&self, db: &'db dyn BaseDatabase) -> Visibility;
}
