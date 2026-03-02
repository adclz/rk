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
        expressions::spec::SpecKind,
        interned::namespace::NamespacePath,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        head::{inheritance::MethodRef, signature::infer_signature},
        index_graphs::namespace_index,
        ty::Type,
    },
    query_string::{query::Query, scope::SymbolSearch},
};
use rustc_hash::FxHashSet;
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

    /// Complete POUs inside a namespace and sub-namespace fragments.
    /// Used for qualified name completion like `System.Math.` in body context.
    pub fn namespace_completion(
        &mut self,
        path: NamespacePath,
        db: &'_ dyn WorkspaceDataBase,
    ) -> &mut Self {
        let builder = CompletionBuilder::default().with_mode(self.mode);
        let path_fragments = path.fragments(db);

        // 1. Get all POUs inside this namespace
        for ns_decl in namespace_index(db, path).iter() {
            for pou in ns_decl.pous(db).iter() {
                builder.build_pou(db, pou, None, &mut self.items);
            }
        }

        // 2. Get sub-namespace fragments via prefix search
        let mut ns_query = Query::new(path.to_string(db));
        ns_query.prefix();
        let results = SymbolSearch::new(|_, _| true)
            .with_query(ns_query)
            .only_namespaces()
            .search(db);

        let mut seen = FxHashSet::default();
        for ns_decl in results.namespaces() {
            let ns_fragments = ns_decl.path(db).fragments(db);
            // Show the next fragment after the current path depth
            if let Some(frag) = ns_fragments.get(path_fragments.len()) {
                let label = frag.text(db).to_string();
                if seen.insert(label.clone()) {
                    self.items.push(CompletionItem {
                        label,
                        detail: Some("(NAMESPACE)".into()),
                        kind: Some(CompletionItemKind::MODULE),
                        ..Default::default()
                    });
                }
            }
        }

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
            QueryMode::Head => {
                |pou: &Pou<'db>, db: &'db dyn WorkspaceDataBase| !matches!(pou, Pou::Function(_))
            }
            QueryMode::Body => |pou: &Pou<'db>, db: &'db dyn WorkspaceDataBase| {
                match pou {
                    Pou::Function(_) => true,
                    // enum types are allowed and all variants should be suggested
                    Pou::DataType(typ) => matches!(typ.spec(db).kind(db), SpecKind::Enum(_)),
                    _ => false,
                }
            },
        };

        let scope_search = match self.mode {
            QueryMode::Head => {
                // in head mode, we also want to include variables from parent scopes
                SymbolSearch::new(filter)
                    .with_scope(self.scope)
                    .only_pous()
                    .with_query(self.query.clone())
            }
            QueryMode::Body => {
                // in body mode, we want to include local variables and parameters
                SymbolSearch::new(filter)
                    .with_scope(self.scope)
                    .with_pous(true)
                    .with_variables(true)
                    .with_query(self.query.clone())
            }
        };

        let pous = scope_search.search(db);

        for pou in pous.local_pous() {
            builder.build_pou(db, pou, None, &mut self.items);
        }

        for (ns, pou) in pous.imported_pous() {
            builder.build_pou(db, &pou, Some(&ns), &mut self.items);
        }

        if self.mode == QueryMode::Body {
            for var in pous.variables() {
                self.items.push(builder.build_variable(db, var));
            }
        }

        // Include root-level namespace fragments (e.g. "System")
        // so typing `S` shows `System` alongside variables/POUs in both head and body.
        let ns_results = SymbolSearch::new(|_, _| true)
            .with_query(self.query.clone())
            .only_namespaces()
            .search(db);

        let mut seen = FxHashSet::default();
        for ns_decl in ns_results.namespaces() {
            let ns_fragments = ns_decl.path(db).fragments(db);
            if let Some(frag) = ns_fragments.first() {
                let label = frag.text(db).to_string();
                if seen.insert(label.clone()) {
                    self.items.push(CompletionItem {
                        label,
                        detail: Some("(NAMESPACE)".into()),
                        kind: Some(CompletionItemKind::MODULE),
                        ..Default::default()
                    });
                }
            }
        }
    }
}
