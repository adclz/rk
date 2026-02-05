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
    },
    hir_ty::{body::infer_body, signature::infer_signature},
};

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
        let ty = infer
            .init_expr_result
            .type_of_init_expr
            .get(self)
            .copied();
        
        // Try field completion
        if let Some(ty) = ty
            && !ty.is_never()
        {
            ctx.field_completion(ty, db);
        }
        Some(ctx.take_items())
    }
}
