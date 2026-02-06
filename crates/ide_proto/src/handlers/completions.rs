use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Expr, InitExpr, PathExpr, VariableAccess},
            invocation::Invocation,
            spec::Spec,
        },
        namespace::NamespaceDecl,
        pous::pou::Pou,
        scope::ScopeKind,
        semantic_index::get_scope,
        using::Using,
    },
    hir_ty::{body::infer_body, signature::infer_signature},
    query_string::namespace::NamespaceSearchCtx,
};
use rustc_hash::FxHashSet;

use crate::handlers::{
    CompletionHandler,
    completions_utils::{CompletionCtx, QueryMode, pou_context::HeadLocation, static_snippets},
};

impl<'db> CompletionHandler<'db> for NamespaceDecl<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        _query: String,
    ) -> Option<Vec<CompletionItem>> {
        // only trigger comletion if we're not typing the namespace name
        if self.name_span(db).end_byte >= offset {
            return None;
        }

        Some(vec![
            static_snippets::namespace(),
            static_snippets::using(),
            static_snippets::function(),
            static_snippets::function_block(),
            static_snippets::class(),
            static_snippets::interface(),
            static_snippets::type_(),
        ])
    }
}

impl<'db> CompletionHandler<'db> for Pou<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
        let head_result = ctx.located_pou_completion(*self, db);

        if head_result.head_location == HeadLocation::InBody {
            ctx.scope_completion(self.get_scope_id(db), &query, db);
            ctx.items.extend(static_snippets::all_stmts());
        }

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for Spec<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Head);
        ctx.scope_completion(self.get_scope_id(db), &query, db);
        ctx.items.extend(static_snippets::elem_type_names());
        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for PathExpr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);

        let infer = infer_body(db, self.get_scope_id(db));
        let ty = infer.get_type_of_path_expr(db, *self);

        if let Some(ty) = ty
            && !ty.is_never()
        {
            ctx.field_completion(ty, db);
            return Some(ctx.take_items());
        }

        // check if we're in a pou body, if so add all statements as completion items
        if let ScopeKind::Pou(pou) = get_scope(db, self.get_scope_id(db)).kind
            && ctx
                .located_pou_completion(pou, db)
                .head_location
                .is_in_body()
        {
            ctx.scope_completion(self.get_scope_id(db), &query, db);
            ctx.items.extend(static_snippets::all_stmts());
            ctx.items.extend(static_snippets::elem_type_names_init());
        }

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for VariableAccess<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);

        let infer = infer_body(db, self.get_scope_id(db));
        let ty = infer.get_type_of_variable_access(db, *self);

        if let Some(ty) = ty
            && !ty.is_never()
        {
            ctx.field_completion(ty, db);
            return Some(ctx.take_items());
        }

        // check if we're in a pou body, if so add all statements as completion items
        if let ScopeKind::Pou(pou) = get_scope(db, self.get_scope_id(db)).kind
            && ctx
                .located_pou_completion(pou, db)
                .head_location
                .is_in_body()
        {
            ctx.scope_completion(self.get_scope_id(db), &query, db);
            ctx.items.extend(static_snippets::all_stmts());
            ctx.items.extend(static_snippets::elem_type_names_init());
        }

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for Expr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);

        let infer = infer_body(db, self.get_scope_id(db));
        let ty = infer.get_type_of_expr(*self);

        if let Some(ty) = ty
            && !ty.is_never()
        {
            ctx.field_completion(ty, db);
            return Some(ctx.take_items());
        }

        // check if we're in a pou body, if so add all statements as completion items
        if let ScopeKind::Pou(pou) = get_scope(db, self.get_scope_id(db)).kind
            && ctx
                .located_pou_completion(pou, db)
                .head_location
                .is_in_body()
        {
            ctx.scope_completion(self.get_scope_id(db), &query, db);
            ctx.items.extend(static_snippets::all_stmts());
            ctx.items.extend(static_snippets::elem_type_names_init());
        }

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for Invocation<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        _query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);

        let infer = infer_body(db, self.get_scope_id(db));
        let ty = infer.get_type_of_invocation(db, *self).unwrap_or_default();

        // Try field completion, fall back to scope if type is unavailable
        ctx.field_completion(ty, db);

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for InitExpr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        _query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Head);

        let infer = infer_signature(db, self.get_scope_id(db));
        let ty = infer.init_expr_result.type_of_init_expr.get(self).copied();

        // Try field completion
        if let Some(ty) = ty
            && !ty.is_never()
        {
            ctx.field_completion(ty, db);
        }

        ctx.items.extend(static_snippets::elem_type_names_init());

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for Using<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
        trigger_character: Option<String>,
        _query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut results = vec![];
        let search = NamespaceSearchCtx::new(self.path(db).path);
        let current_fragments = self.path(db).path.fragments(db);

        let items = search.search(db);
        let mut seen = FxHashSet::default();

        // If dot was the trigger character, show the next level of namespace fragments.
        // Otherwise, complete the last (current) fragment.
        let show_index = if trigger_character.as_deref() == Some(".") {
            current_fragments.len()
        } else {
            current_fragments.len().saturating_sub(1)
        };

        for item in items {
            let ns_fragments = item.path(db).fragments(db);
            if let Some(frag) = ns_fragments.get(show_index) {
                let label = frag.text(db).to_string();
                if seen.insert(label.clone()) {
                    results.push(CompletionItem::new_simple(label, "NAMESPACE".to_string()));
                }
            }
        }
        Some(results)
    }
}
