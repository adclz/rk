use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    CallSite, HasName, HirNodeInfo,
    hir_def::{
        config::{ConfigDecl, ResourceDecl},
        expressions::{
            expression::{Expr, InitExpr, PathExpr, PathExprKind, VariableAccess},
            invocation::Invocation,
            spec::{Spec, SpecKind},
        }, hir_node::HirNode, interned::namespace::NamespacePath, namespace::NamespaceDecl, pous::pou::Pou, program::ProgramDecl, scope::ScopeKind, semantic_index::{NodeKey, get_scope, semantic_index}, using::Using
    },
    hir_ty::{infer::Infer, index_graphs::namespace_index},
    query_string::{query::Query, scope::SymbolSearch},
};
use rustc_hash::FxHashSet;

use crate::{
    handlers::{
        CompletionHandler, CompletionRequest,
        completions_utils::{CompletionCtx, QueryMode, pou_context::HeadLocation, static_snippets},
    },
};

/// Walk up a PathExpr's field chain to collect the full ident path.
/// For `System.Math.Sin`, this returns `["System", "Math", "Sin"]` as a NamespacePath.
/// Returns None if the chain contains Index or Deref (can't be namespace paths).
pub(crate) fn try_build_namespace_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: &PathExpr<'db>,
) -> Option<NamespacePath> {
    let mut fragments = vec![path.ident(db).ident];
    let mut current = path.expr(db);
    loop {
        match current {
            PathExprKind::Field(f) => {
                fragments.push(f.path.ident(db).ident);
                current = f.path.expr(db);
            }
            PathExprKind::VarAccess(_) => break,
            _ => return None,
        }
    }
    fragments.reverse();
    Some(NamespacePath::new(db, fragments))
}

/// Check if a path matches any namespace (exact) or is a prefix of any namespace.
/// E.g. "System" matches even if only "System.Math" exists.
pub(crate) fn is_namespace_prefix(db: &dyn WorkspaceDataBase, path: NamespacePath) -> bool {
    // Exact match
    if !namespace_index(db, path).is_empty() {
        return true;
    }
    // Prefix match: check if any namespace starts with this path
    let mut query = Query::new(path.to_string(db));
    query.prefix();
    let results = SymbolSearch::new(|_, _| true)
        .with_query(query)
        .only_namespaces()
        .search(db);
    results.namespaces().next().is_some()
}

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
                // For is_last_before with trailing dot, try namespace completion
                // before delegating to parent (Field delegation loses namespace context)
                if req.is_last_before {
                    if let Some(ns_path) = try_build_namespace_path(db, p) {
                        if is_namespace_prefix(db, ns_path) {
                            let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
                            ctx.namespace_completion(ns_path, db);
                            return Some(ctx.take_items());
                        }
                    }
                }

                // For non-leaf path expressions (field/index/deref), derive the parent
                // on-demand and use its completions (the current node is likely incomplete)
                let child_req = req.with_query(p.ident(db).text(db).to_string());
                match p.expr(db) {
                    PathExprKind::Field(f) => {
                        // Check if the parent path is a namespace before delegating.
                        // e.g. `System.M|` → parent is `System` → show namespace children
                        if let Some(parent_ns) = try_build_namespace_path(db, &f.path) {
                            if is_namespace_prefix(db, parent_ns) {
                                let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
                                ctx.namespace_completion(parent_ns, db);
                                return Some(ctx.take_items());
                            }
                        }
                        // Not a namespace → regular field delegation
                        Some(f.path.completion(db, &child_req).unwrap_or_default())
                    }
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
            HirNode::Config(c) => c.completion(db, req),
            HirNode::Resource(r) => r.completion(db, req),
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
        // only trigger completion if we're not typing the namespace name
        if self.name_span(db).end_byte >= req.offset {
            return None;
        }
        // Don't trigger on dot — it's the user typing a dotted namespace name (e.g. System.|)
        if req.trigger_character.as_deref() == Some(".") {
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
        // A dot trigger that lands on the POU itself (no specific child node)
        // means the dot is not after a valid field-access target (e.g. `0.`).
        if req.trigger_character.as_deref() == Some(".") {
            return Some(vec![]);
        }

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
        // A dot trigger that lands on the POU itself (no specific child node)
        // means the dot is not after a valid field-access target (e.g. `0.`).
        if req.trigger_character.as_deref() == Some(".") {
            return Some(vec![]);
        }

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
        if let SpecKind::Target(target) = self.kind(db) {
            let is_dot_trigger = req.trigger_character.as_deref() == Some(".");

            if is_dot_trigger {
                // Build full path: namespace fragments + target ident (skip empty/MISSING target)
                let mut fragments: Vec<_> = target
                    .path
                    .namespace
                    .as_ref()
                    .map(|ns| ns.fragments(db).to_vec())
                    .unwrap_or_default();
                if !target.path.target.ident.text(db).is_empty() {
                    fragments.push(target.path.target.ident);
                }
                let full_path = NamespacePath::new(db, fragments);

                if is_namespace_prefix(db, full_path) {
                    let mut ctx = CompletionCtx::new(req.offset, QueryMode::Head);
                    ctx.namespace_completion(full_path, db);
                    return Some(ctx.take_items());
                }
            } else if let Some(namespace) = &target.path.namespace {
                // Editing a fragment in a namespace path (e.g., Std.C|.Timers or Std.C|)
                // Determine which fragment the cursor is in and use the prefix before it.
                let ns_fragments = namespace.fragments(db);

                // Find how many namespace fragments precede the cursor
                let mut prefix_len = ns_fragments.len();
                for (i, ast_id) in namespace.spans.iter().enumerate() {
                    let site = CallSite::new(namespace.scope_id, *ast_id);
                    if req.offset <= site.get_span(db).end_byte {
                        prefix_len = i;
                        break;
                    }
                }

                if prefix_len > 0 {
                    let prefix = NamespacePath::new(db, ns_fragments[..prefix_len].to_vec());
                    if is_namespace_prefix(db, prefix) {
                        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Head);
                        ctx.namespace_completion(prefix, db);
                        return Some(ctx.take_items());
                    }
                }
            }
        }

        // Default: scope completion + elementary types
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
            let sema = semantic_index(db, self.get_scope_id(db).file(db));

            // Current node can't resolve - check if node_index_pos points to
            // a nearby PathExpr that did resolve, and use its type.
            if let Some(key) = req.node_index_pos {
                let mut last_path = None;
                let mut key = key;
                loop {
                    if let Some(node) = sema.node_index.get(key) {
                        // checks this is both a PathExpr and that it resolved to a non-never type
                        if let HirNode::PathExpr(p) = node && !p.infer(db).is_never() {
                            last_path = Some(p); 
                            key = NodeKey::from_usize(key.index().saturating_add(1));
                        } else {
                            break
                        }
                    } else {
                        break;
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

        // Check if path chain forms a namespace (e.g. typing `System.Ma|`)
        // Only for multi-fragment paths - single identifiers like `S` are handled
        // by scope_completion which includes root-level namespace fragments.
        if let Some(ns_path) = try_build_namespace_path(db, self) {
            if ns_path.fragments(db).len() > 1 && is_namespace_prefix(db, ns_path) {
                ctx.namespace_completion(ns_path, db);
                return Some(ctx.take_items());
            }
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

impl<'db> CompletionHandler<'db> for ConfigDecl<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        // Don't trigger on the name
        if self.get_name_span(db).end_byte >= req.offset {
            return None;
        }

        Some(vec![
            static_snippets::var_global(),
            static_snippets::resource(),
            static_snippets::task_config(),
            static_snippets::prog_config(),
            static_snippets::var_access(),
        ])
    }
}

impl<'db> CompletionHandler<'db> for ResourceDecl<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        // Don't trigger on the name
        if self.name(db).get_span(db).end_byte >= req.offset {
            return None;
        }

        Some(vec![
            static_snippets::var_global(),
            static_snippets::task_config(),
            static_snippets::prog_config(),
        ])
    }
}

impl<'db> CompletionHandler<'db> for Using<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        let mut results = vec![];
        let mut ns_query = Query::new(self.path(db).path.to_string(db));
        ns_query.prefix();
        let current_fragments = self.path(db).path.fragments(db);

        let search_result = SymbolSearch::new(|_, _| true)
            .with_query(ns_query)
            .only_namespaces()
            .search(db);
        let items: Vec<_> = search_result.namespaces().copied().collect();
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
