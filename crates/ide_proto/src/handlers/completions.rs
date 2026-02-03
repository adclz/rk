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
        scope::ScopeKind,
        semantic_index::get_scope,
    },
    hir_ty::body::infer_body,
};

use crate::handlers::{
    CompletionHandler,
    completions_utils::{
        field::FieldCompletion,
        pou_strategy::{self, complete_pou},
        scope::{QueryMode, ScopeCompletionCtx},
        static_snippets,
    },
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
        let mut items = vec![];
        pou_strategy::complete_pou(*self, db, offset, &mut items)?;
        Some(items)
    }
}

impl<'db> CompletionHandler<'db> for Spec<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let mut items = vec![];
        items.extend(static_snippets::elem_type_names());

        let mut scope_ctx =
            ScopeCompletionCtx::new(QueryMode::Signature, self.get_scope_id(db), offset, "");
        scope_ctx.query_scope_items(db);
        items.extend(scope_ctx.take_items());
        Some(items)
    }
}

impl<'db> CompletionHandler<'db> for InitExpr<'db> {
    fn completion(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        Some(static_snippets::elem_type_names())
    }
}

impl<'db> CompletionHandler<'db> for PathExpr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let infer = infer_body(db, self.get_scope_id(db));
        if let Some(ty) = infer.get_type_of_path_expr(db, *self) {
            return ty.normalize(db).field_completion(db, offset);
        }

        let mut items = vec![];
        // Checks if the body is empty
        if infer.type_of_path_expr.is_empty() {
            match get_scope(db, self.get_scope_id(db)).kind {
                ScopeKind::Pou(p) => {
                    complete_pou(p, db, offset, &mut items);
                }
                _ => items.extend(static_snippets::all_stmts()),
            }
        }

        // Fallback to scope-based completions
        let mut scope_ctx =
            ScopeCompletionCtx::new(QueryMode::Body, self.get_scope_id(db), offset, "");
        scope_ctx.query_scope_items(db);
        items.extend(scope_ctx.take_items());
        Some(items)
    }
}

impl<'db> CompletionHandler<'db> for VariableAccess<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let infer = infer_body(db, self.get_scope_id(db));
        if let Some(ty) = infer.get_type_of_variable_access(db, *self) {
            return ty.normalize(db).field_completion(db, offset);
        }

        let mut items = vec![];

        // Fallback to scope-based completions
        let mut scope_ctx =
            ScopeCompletionCtx::new(QueryMode::Body, self.get_scope_id(db), offset, "");
        scope_ctx.query_scope_items(db);
        items.extend(scope_ctx.take_items());
        Some(items)
    }
}

impl<'db> CompletionHandler<'db> for Expr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let infer = infer_body(db, self.get_scope_id(db));
        if let Some(ty) = infer.get_type_of_expr(*self)
            && !ty.is_never()
        {
            return ty.normalize(db).field_completion(db, offset);
        }

        // Fallback to scope-based completions
        let mut items = vec![];
        let mut scope_ctx =
            ScopeCompletionCtx::new(QueryMode::Body, self.get_scope_id(db), offset, "");
        scope_ctx.query_scope_items(db);
        items.extend(scope_ctx.take_items());
        Some(items)
    }
}
