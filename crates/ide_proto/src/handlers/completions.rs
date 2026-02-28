use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    CallSite, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Expr, InitExpr, PathExpr, PathExprKind, VariableAccess},
            invocation::Invocation,
            spec::Spec,
        }, hir_node::HirNode, namespace::NamespaceDecl, pous::pou::Pou, program::ProgramDecl, scope::ScopeKind, semantic_index::get_scope, using::Using
    },
    hir_ty::infer::Infer,
    query_string::namespace::NamespaceSearchCtx,
};
use rustc_hash::FxHashSet;

use crate::{
    handlers::{
        CompletionHandler,
        completions_utils::{CompletionCtx, QueryMode, pou_context::HeadLocation, static_snippets},
    },
};

impl<'db> CompletionHandler<'db> for  HirNode<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        trigger_character: Option<String>,
        _query: String,
    ) -> Option<Vec<CompletionItem>> {
        match self { 
            HirNode::InitExpr(i) => i.completion(
                db,
                offset,
                trigger_character,
                i.to_string(db).to_owned(),
            ),
            HirNode::PathExpr(p) => {
                // For non-leaf path expressions (field/index/deref), derive the parent
                // on-demand and use its completions (the current node is likely incomplete)
                let query = p.ident(db).text(db).to_string();
                match p.expr(db) {
                    PathExprKind::Field(f) => Some(
                        f.path
                            .completion(db, offset, trigger_character, query)
                            .unwrap_or_default(),
                    ),
                    PathExprKind::Index(i) => Some(
                        i.path
                            .completion(db, offset, trigger_character, query)
                            .unwrap_or_default(),
                    ),
                    PathExprKind::Deref(d) => Some(
                        d.path
                            .completion(db, offset, trigger_character, query)
                            .unwrap_or_default(),
                    ),
                    PathExprKind::VarAccess(_) => {
                        p.completion(db, offset, trigger_character, query)
                    }
                }
            }
            HirNode::Spec(s) => s.completion(
                db,
                offset,
                trigger_character,
                CallSite::from_scoped(db, s).to_string(db).to_string(),
            ),
            HirNode::Expr(e) => e.completion(
                db,
                offset,
                trigger_character,
                CallSite::from_scoped(db, e).to_string(db).to_string(),
            ),
            HirNode::Using(u) => u.completion(db, offset, trigger_character, "".into()),
            HirNode::Namespace(ns) => ns.completion(db, offset, trigger_character, "".into()),
            HirNode::PouDecl(pou) => pou.completion(db, offset, trigger_character, "".into()),
            HirNode::Program(p) => p.completion(db, offset, trigger_character, "".into()),
            HirNode::Invocation(i) => i.completion(db, offset, trigger_character, "".into()),
            HirNode::VariableAccess(v) => v.completion(
                db,
                offset,
                trigger_character,
                CallSite::from_scoped(db, v).to_string(db).to_string(),
            ),
            _ => None,
        }
    }
}

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

impl<'db> CompletionHandler<'db> for ProgramDecl<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        _trigger_character: Option<String>,
        query: String,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(offset, QueryMode::Body);
        let head_result = ctx.located_program_completion(*self, db);

        eprintln!("Program completion: head location = {:?}, items = {:?}", head_result.head_location, ctx.items);
        if head_result.head_location == HeadLocation::InBody {
            ctx.scope_completion(self.scope_id(db), &query, db);
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

        let ty = self.infer(db);

        if !ty.is_never() {
            ctx.field_completion(ty, db);
            return Some(ctx.take_items());
        }

        // check if we're in a pou/program body, if so add all statements as completion items
        let scope = get_scope(db, self.get_scope_id(db));
        let in_body = match scope.kind {
            ScopeKind::Pou(pou) => ctx.located_pou_completion(pou, db).head_location.is_in_body(),
            ScopeKind::Program(prog) => ctx.located_program_completion(prog, db).head_location.is_in_body(),
            _ => false,
        };
        if in_body {
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

        let ty: hir::hir_ty::ty::Type<'_> = self.infer(db);

        if !ty.is_never() {
            ctx.field_completion(ty, db);
            return Some(ctx.take_items());
        }

        // check if we're in a pou/program body, if so add all statements as completion items
        let scope = get_scope(db, self.get_scope_id(db));
        let in_body = match scope.kind {
            ScopeKind::Pou(pou) => ctx.located_pou_completion(pou, db).head_location.is_in_body(),
            ScopeKind::Program(prog) => ctx.located_program_completion(prog, db).head_location.is_in_body(),
            _ => false,
        };
        if in_body {
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
        let ty = self.infer(db);

        if !ty.is_never() {
            ctx.field_completion(ty, db);
            return Some(ctx.take_items());
        }

        // check if we're in a pou/program body, if so add all statements as completion items
        let scope = get_scope(db, self.get_scope_id(db));
        let in_body = match scope.kind {
            ScopeKind::Pou(pou) => ctx.located_pou_completion(pou, db).head_location.is_in_body(),
            ScopeKind::Program(prog) => ctx.located_program_completion(prog, db).head_location.is_in_body(),
            _ => false,
        };
        if in_body {
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

        let ty = self.infer(db);

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

        let ty = self.infer(db);

        // Try field completion
        if !ty.is_never() {
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
