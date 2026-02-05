use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Expr, InitExpr, PathExpr, VariableAccess},
            spec::Spec,
        },
        namespace::NamespaceDecl,
        pous::pou::Pou,
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
        _db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
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
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
        let head_result = ctx.located_pou_completion(*self, db);

        if head_result.inside_head == HeadLocation::InBody {
            ctx.scope_completion(self.get_scope_id(db), "", db);
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
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Head);
        ctx.scope_completion(self.get_scope_id(db), "", db)
            .items
            .extend(static_snippets::elem_type_names());
        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for PathExpr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);

        let infer = infer_body(db, self.get_scope_id(db));
        let ty = infer.get_type_of_path_expr(db, *self);

        ctx.field_or_scope(ty, self.get_scope_id(db), "", db);

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for VariableAccess<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);

        let infer = infer_body(db, self.get_scope_id(db));
        let ty = infer.get_type_of_variable_access(db, *self);

        // Try field completion, fall back to scope if type is unavailable
        ctx.field_or_scope(ty, self.get_scope_id(db), "", db);

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for Expr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);

        let infer = infer_body(db, self.get_scope_id(db));
        let ty = infer.get_type_of_expr(*self);

        // Try field completion, fall back to scope if type is unavailable
        ctx.field_or_scope(ty, self.get_scope_id(db), "", db);

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for InitExpr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
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
        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for Using<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let mut results = vec![];
        let search = NamespaceSearchCtx::new(self.path(db).path);
        let current_fragments = self.path(db).path.fragments(db);

        let items = search.search(db);
        let mut seen = FxHashSet::default();
        for item in items {
            let ns_fragments = item.path(db).fragments(db);
            
            // Determine the fragment index to show
            // Count how many fragments match exactly
            let mut exact_match_count = 0;
            for (i, current_frag) in current_fragments.iter().enumerate() {
                if let Some(ns_frag) = ns_fragments.get(i) {
                    if current_frag.text(db) == ns_frag.text(db) {
                        exact_match_count += 1;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            
            // If we have exact matches and the namespace goes deeper, show the next fragment
            // Otherwise, show the first fragment (we're completing the first level)
            let show_index = if exact_match_count == current_fragments.len() && exact_match_count > 0 {
                // All current fragments match exactly, show the next one
                exact_match_count
            } else {
                // We're completing the current (first) fragment
                0
            };
            
            if let Some(frag) = ns_fragments.get(show_index) {
                let label = frag.text(db).to_string();
                if !seen.contains(&label) {
                    seen.insert(label.clone());
                    results.push(CompletionItem::new_simple(label, "NAMESPACE".to_string()));
                }
            }
        }
        Some(results)
    }
}
