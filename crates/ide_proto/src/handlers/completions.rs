use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    CallSite, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Expr, InitExpr, PathExpr, PathExprKind, VariableAccess},
            invocation::Invocation,
            spec::Spec,
        }, hir_node::HirNode, namespace::NamespaceDecl, pous::pou::Pou, program::ProgramDecl, scope::ScopeKind, semantic_index::{NodeKey, get_scope, semantic_index}, using::Using
    },
    hir_ty::infer::Infer,
    query_string::namespace::NamespaceSearchCtx,
};
use rustc_hash::FxHashSet;

use crate::{
    handlers::{
        CompletionHandler, CompletionRequest,
        completions_utils::{CompletionCtx, QueryMode, pou_context::HeadLocation, static_snippets},
    },
};

impl<'db> CompletionHandler<'db> for HirNode<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        match self {
            HirNode::InitExpr(i) => i.completion(
                db,
                &req.with_query(i.to_string(db).to_owned()),
            ),
            HirNode::PathExpr(p) => {
                // For non-leaf path expressions (field/index/deref), derive the parent
                // on-demand and use its completions (the current node is likely incomplete)
                let child_req = req.with_query(p.ident(db).text(db).to_string());
                match p.expr(db) {
                    PathExprKind::Field(f) => Some(
                        f.path
                            .completion(db, &child_req)
                            .unwrap_or_default(),
                    ),
                    PathExprKind::Index(i) => Some(
                        i.path
                            .completion(db, &child_req)
                            .unwrap_or_default(),
                    ),
                    PathExprKind::Deref(d) => Some(
                        d.path
                            .completion(db, &child_req)
                            .unwrap_or_default(),
                    ),
                    PathExprKind::VarAccess(_) => {
                        p.completion(db, &child_req)
                    }
                }
            }
            HirNode::Spec(s) => s.completion(
                db,
                &req.with_query(CallSite::from_scoped(db, s).to_string(db).to_string()),
            ),
            HirNode::Expr(e) => e.completion(
                db,
                &req.with_query(CallSite::from_scoped(db, e).to_string(db).to_string()),
            ),
            HirNode::Using(u) => u.completion(db, req),
            HirNode::Namespace(ns) => ns.completion(db, req),
            HirNode::PouDecl(pou) => pou.completion(db, req),
            HirNode::Program(p) => p.completion(db, req),
            HirNode::Invocation(i) => i.completion(db, req),
            HirNode::VariableAccess(v) => v.completion(
                db,
                &req.with_query(CallSite::from_scoped(db, v).to_string(db).to_string()),
            ),
            _ => None,
        }
    }
}

impl<'db> CompletionHandler<'db> for NamespaceDecl<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        // only trigger comletion if we're not typing the namespace name
        if self.name_span(db).end_byte >= req.offset {
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
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
        let head_result = ctx.located_pou_completion(*self, db);

        if head_result.head_location == HeadLocation::InBody {
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
            ctx.items.extend(static_snippets::all_stmts());
        }

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for ProgramDecl<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
        let head_result = ctx.located_program_completion(*self, db);

        if head_result.head_location == HeadLocation::InBody {
            ctx.scope_completion(self.scope_id(db), &req.query, db);
            ctx.items.extend(static_snippets::all_stmts());
        }

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for Spec<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Head);
        ctx.scope_completion(self.get_scope_id(db), &req.query, db);
        ctx.items.extend(static_snippets::elem_type_names());
        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for PathExpr<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);

        if req.is_last_before {
            // Current node can't resolve - check if node_index_pos points to
            // a nearby PathExpr that did resolve, and use its type.
            if let Some(key) = req.node_index_pos {
                let sema = semantic_index(db, self.get_scope_id(db).file(db));
                let mut last_path = None;
                let mut key = key;
                while let Some(node) = sema.node_index.get(key) {
                    if let HirNode::PathExpr(p) = node && !p.infer(db).is_never() {
                        last_path = Some(p); 
                        key = NodeKey::from_usize(key.index().saturating_add(1));
                    } else {
                        break
                    }
                }
                if let Some(p) = last_path {
                    ctx.field_completion(p.infer(db), db);
                    return Some(ctx.take_items());
                }
            }
        }

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
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
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
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);

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
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
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
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
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
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
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
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);

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
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Head);

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
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut results = vec![];
        let search = NamespaceSearchCtx::new(self.path(db).path);
        let current_fragments = self.path(db).path.fragments(db);

        let items = search.search(db);
        let mut seen = FxHashSet::default();

        // If dot was the trigger character, show the next level of namespace fragments.
        // Otherwise, complete the last (current) fragment.
        let show_index = if req.trigger_character.as_deref() == Some(".") {
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
