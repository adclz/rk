#![allow(unused)]
use std::fmt::Display;

use auto_lsp::{
    core::span::Span,
    lsp_types::{
        self, CompletionItem, CompletionItemKind, CompletionItemLabelDetails, InsertTextFormat,
        InsertTextMode, Range, TextEdit,
    },
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        interned::namespace::NamespacePath,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        signature::{infer_signature, inheritance::MethodRef},
        ty::Type,
    },
    query_string::{query::Query, scope::ScopeSearchCtx},
};
use serde_json::de;

use crate::handlers::completions_utils::{
    CompletionCtx, QueryMode, completion_item_builder::CompletionBuilder,
};

impl CompletionCtx {
    pub fn scope_completion(
        &mut self,
        scope: ScopeId<'_>,
        query: &str,
        db: &'_ dyn WorkspaceDataBase,
    ) -> &mut Self {
        let mut scope_ctx = ScopeCompletionCtx::new(self.mode, scope, self.offset, query);
        scope_ctx.query_scope_items(db);
        self.items.extend(scope_ctx.take_items());
        self
    }
}

pub(super) struct ScopeCompletionCtx<'db> {
    pub scope: ScopeId<'db>,
    pub mode: QueryMode,
    pub offset: usize,
    pub query: Query,
    pub items: Vec<CompletionItem>,
}

impl<'db> ScopeCompletionCtx<'db> {
    pub fn new(mode: QueryMode, scope: ScopeId<'db>, offset: usize, query: &'db str) -> Self {
        let mut query = Query::new(query.to_owned());
        query.fuzzy();
        Self {
            mode,
            scope,
            offset,
            query,
            items: vec![],
        }
    }

    pub fn take_items(&mut self) -> Vec<CompletionItem> {
        std::mem::take(&mut self.items)
    }

    pub fn query_scope_items(&mut self, db: &'db dyn WorkspaceDataBase) {
        let builder = CompletionBuilder::default()
            .with_import(db, self.scope)
            .with_mode(self.mode);

        // Filter pous based on the query mode
        // If Head, we assume it's a datatype or variable declaration
        // everything but functions should be suggested

        // If Body, Functions are allowed but not other POUs
        // that's because they have to be declared in var sections
        let filter = match self.mode {
            QueryMode::Head => |pou: &Pou<'db>| !matches!(pou, Pou::Function(_)),
            QueryMode::Body => |pou: &Pou<'db>| matches!(pou, Pou::Function(_)),
        };

        let pous = ScopeSearchCtx::new(self.scope)
            .with_query(self.query.clone())
            .search(db, filter);

        for pou in pous.local_pous() {
            self.items.push(builder.build_pou(db, pou, None));
        }

        for (ns, pou) in pous.imported_pous() {
            self.items.push(builder.build_pou(db, &pou, Some(&ns)));
        }

        for var in pous.variables() {
            self.items.push(builder.build_variable(db, var));
        }
    }
}
