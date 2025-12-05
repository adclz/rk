#![recursion_limit = "256"]
#![allow(deprecated)]
#![allow(unused_variables)]

use auto_lsp::{
    core::{ast::AstNode, span::Span},
    default::db::BaseDatabase,
};

use crate::hir_def::{interned::identifier::Ident, scope::ScopeId, semantic_index::semantic_index};

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

pub trait HirNodeInfo<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId;

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db>;

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

    /*fn get_name_span(&'db self, db: &'db dyn BaseDatabase) -> Option<Span> {
        self.get_name_id(db).map(|name_id| {
            semantic_index(db, self.get_scope_id(db).file(db))
                .ast
                .get(name_id.0)
                .unwrap_or_else(|| {
                    panic!(
                        "Invalid name ID {} when attempting to retrieve name span",
                        name_id.0
                    )
                })
                .get_span()
        })
    }
    
    fn get_name_id(&'db self, _db: &'db dyn BaseDatabase) -> Option<AstId> {
        None
    }
    */
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
