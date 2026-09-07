use ast::generated::DataTypeDecl;
use auto_lsp::{
    default::db::{file::File, tracked::get_ast},
    lsp_types::CompletionItem,
};
use db::WorkspaceDataBase;
use hir::{
    CallSite, HasName, HirNodeInfo,
    hir_def::{
        config::{ConfigDecl, ResourceDecl, TaskConfig},
        expressions::{
            expression::{Expr, InitExpr, PathExpr, PathExprKind, VariableAccess},
            invocation::Invocation,
            spec::{Spec, SpecKind},
        },
        hir_node::HirNode,
        interned::namespace::NamespacePath,
        namespace::NamespaceDecl,
        pous::pou::Pou,
        program::ProgramDecl,
        scope::{Scope, ScopeKind},
        semantic_index::{NodeKey, get_scope, semantic_index},
        using::Using,
    },
    hir_ty::{
        head::inheritance::MethodRef,
        index_graphs::{namespace_index, namespace_path_candidates},
        infer::Infer,
    },
    query_string::{query::Query, scope::SymbolSearch},
};
use rustc_hash::FxHashSet;

use crate::{
    comment_index::comment_index,
    handlers::{
        CompletionHandler, CompletionRequest,
        completions_utils::{CompletionCtx, QueryMode, pou_context::HeadLocation, static_snippets},
    },
    walk::completion_descendant_at,
};

/// Main entry point for completions. Resolves the node at the given offset
/// and dispatches to the appropriate handler.
pub fn complete(
    db: &dyn WorkspaceDataBase,
    file: File,
    offset: usize,
    trigger_character: Option<String>,
) -> Vec<CompletionItem> {
    // Suppress completions inside comments
    let in_comment = comment_index(db, file)
        .map
        .values()
        .any(|comment| comment.range.start_byte <= offset && offset <= comment.range.end_byte);

    if in_comment {
        return vec![];
    }

    let (target, node_key, is_last_before) = match completion_descendant_at(db, file, offset) {
        Some(result) => result,
        None => {
            // Check if cursor is inside a TYPE declaration at the AST level.
            // When the TYPE body is incomplete (no spec yet), no HIR node covers
            // the cursor, but we should still offer type-level completions
            // instead of POU-level snippets.
            let ast = get_ast(db, file);
            let in_type_decl = ast.iter().any(|node| {
                let range = node.get_range();
                range.start_byte <= offset
                    && offset <= range.end_byte
                    && node.lower().downcast_ref::<DataTypeDecl>().is_some()
            });

            if in_type_decl {
                return vec![];
            }

            // No target node — show general completions (namespaces, POU snippets, etc.)
            return vec![
                static_snippets::namespace(),
                static_snippets::using(),
                static_snippets::function(),
                static_snippets::function_block(),
                static_snippets::program(),
                static_snippets::class(),
                static_snippets::interface(),
                static_snippets::type_(),
                static_snippets::configuration(),
            ];
        }
    };

    let req = CompletionRequest {
        offset,
        trigger_character,
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };

    target.completion(db, &req).unwrap_or_default()
}

impl<'db> CompletionHandler<'db> for HirNode<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        match self {
            HirNode::InitExpr(i) => i.completion(db, &req.with_query(i.to_string(db).to_owned())),
            HirNode::PathExpr(p) => {
                // For is_last_before with trailing dot, try namespace completion
                // before delegating to parent (Field delegation loses namespace context)
                if req.is_last_before
                    && let Some(written) = try_build_namespace_path(db, p)
                    && let Some(ns_path) = resolve_namespace_prefix(db, p.get_scope_id(db), written)
                {
                    let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
                    ctx.namespace_completion(ns_path, db);
                    return Some(ctx.take_items());
                }

                // For non-leaf path expressions (field/index/deref), derive the parent
                // on-demand and use its completions (the current node is likely incomplete)
                let child_req = req.with_query(p.ident(db).text(db).to_string());
                match p.expr(db) {
                    PathExprKind::Field(f) => {
                        // Check if the parent path is a namespace before delegating.
                        // e.g. `System.M|` → parent is `System` → show namespace children
                        if let Some(written) = try_build_namespace_path(db, &f.path)
                            && let Some(parent_ns) =
                                resolve_namespace_prefix(db, p.get_scope_id(db), written)
                        {
                            let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
                            ctx.namespace_completion(parent_ns, db);
                            return Some(ctx.take_items());
                        }
                        // Not a namespace → regular field delegation
                        Some(f.path.completion(db, &child_req).unwrap_or_default())
                    }
                    PathExprKind::Index(i) => {
                        Some(i.path.completion(db, &child_req).unwrap_or_default())
                    }
                    PathExprKind::Deref(d) => {
                        Some(d.path.completion(db, &child_req).unwrap_or_default())
                    }
                    PathExprKind::VarAccess(_) => p.completion(db, &child_req),
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
            HirNode::Task(t) => t.completion(db, req),
            HirNode::Invocation(i) => i.completion(db, req),
            HirNode::MethodRef(m) => m.completion(db, req),
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

        // TYPE declarations have no body - completions are handled by their Spec children
        if matches!(self, Pou::DataType(_)) {
            return None;
        }

        // Don't trigger completions while typing the POU name
        if self.get_name_span(db).end_byte >= req.offset {
            return None;
        }

        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
        let head_result = ctx.located_pou_completion(*self, db);

        if matches!(
            head_result.head_location,
            HeadLocation::BeforeVars | HeadLocation::InBodyAfterVars
        ) {
            ctx.items.push(static_snippets::using());
        }

        if head_result.head_location.is_in_body() {
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
            ctx.items.extend(static_snippets::all_stmts());

            let scope = get_scope(db, self.get_scope_id(db));
            maybe_add_self_return(db, scope, &mut ctx.items);
        }

        Some(ctx.take_items())
    }
}

impl<'db> CompletionHandler<'db> for MethodRef<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        if req.trigger_character.as_deref() == Some(".") {
            return Some(vec![]);
        }

        // Don't trigger completions while typing the method name
        if self.get_name_span(db).end_byte >= req.offset {
            return None;
        }

        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
        let head_result = ctx.located_method_completion(*self, db);

        if matches!(
            head_result.head_location,
            HeadLocation::BeforeVars | HeadLocation::InBodyAfterVars
        ) {
            ctx.items.push(static_snippets::using());
        }

        if head_result.head_location.is_in_body() {
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
            ctx.items.extend(static_snippets::all_stmts());

            let scope = get_scope(db, self.get_scope_id(db));
            maybe_add_self_return(db, scope, &mut ctx.items);
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

        // Don't trigger completions while typing the program name
        if self.get_name_span(db).end_byte >= req.offset {
            return None;
        }

        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Body);
        let head_result = ctx.located_program_completion(*self, db);

        if matches!(
            head_result.head_location,
            HeadLocation::BeforeVars | HeadLocation::InBodyAfterVars
        ) {
            ctx.items.push(static_snippets::using());
        }

        if head_result.head_location.is_in_body() {
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
                let written = NamespacePath::new(db, fragments);

                if let Some(full_path) =
                    resolve_namespace_prefix(db, self.get_scope_id(db), written)
                {
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
                    let written = NamespacePath::new(db, ns_fragments[..prefix_len].to_vec());
                    if let Some(prefix) =
                        resolve_namespace_prefix(db, self.get_scope_id(db), written)
                    {
                        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Head);
                        ctx.namespace_completion(prefix, db);
                        return Some(ctx.take_items());
                    }
                }
            }
        }

        // Default: scope completion + elementary types + compound type snippets
        let mut ctx = CompletionCtx::new(req.offset, QueryMode::Head);
        ctx.scope_completion(self.get_scope_id(db), &req.query, db);
        ctx.items.extend(static_snippets::elem_type_names());
        ctx.items.push(static_snippets::struct_());
        ctx.items.push(static_snippets::array());
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
                while let Some(node) = sema.node_index.get(key) {
                    // checks this is both a PathExpr and that it resolved to a non-never type
                    if let HirNode::PathExpr(p) = node
                        && !p.infer(db).is_never()
                    {
                        last_path = Some(p);
                        key = NodeKey::from_usize(key.index().saturating_add(1));
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
        if let Some(written) = try_build_namespace_path(db, self)
            && written.fragments(db).len() > 1
            && let Some(ns_path) = resolve_namespace_prefix(db, self.get_scope_id(db), written)
        {
            ctx.namespace_completion(ns_path, db);
            return Some(ctx.take_items());
        }

        // check if we're in a pou/program/method body, if so add all statements as completion items
        let scope = get_scope(db, self.get_scope_id(db));
        let in_body = is_in_body(scope, &mut ctx, db);
        if in_body {
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
            ctx.items.extend(static_snippets::all_stmts());
            ctx.items.extend(static_snippets::elem_type_names_init());
            maybe_add_self_return(db, scope, &mut ctx.items);
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

        // check if we're in a pou/program/method body, if so add all statements as completion items
        let scope = get_scope(db, self.get_scope_id(db));
        let in_body = is_in_body(scope, &mut ctx, db);
        if in_body {
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
            ctx.items.extend(static_snippets::all_stmts());
            ctx.items.extend(static_snippets::elem_type_names_init());
            maybe_add_self_return(db, scope, &mut ctx.items);
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

        // check if we're in a pou/program/method body, if so add all statements as completion items
        let scope = get_scope(db, self.get_scope_id(db));
        let in_body = is_in_body(scope, &mut ctx, db);
        if in_body {
            ctx.scope_completion(self.get_scope_id(db), &req.query, db);
            ctx.items.extend(static_snippets::all_stmts());
            ctx.items.extend(static_snippets::elem_type_names_init());
            maybe_add_self_return(db, scope, &mut ctx.items);
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

        // A CONFIGURATION holds globals and RESOURCEs; tasks and program
        // instances go inside a RESOURCE (E0039), so offering them here would
        // insert code the compiler rejects.
        Some(vec![
            static_snippets::var_global(),
            static_snippets::resource(),
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

        // A RESOURCE groups tasks and the programs bound to them. It holds no
        // variables — VAR_GLOBAL belongs to the CONFIGURATION (E0030).
        Some(vec![
            static_snippets::task_config(),
            static_snippets::prog_config(),
        ])
    }
}

impl<'db> CompletionHandler<'db> for TaskConfig<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        req: &CompletionRequest,
    ) -> Option<Vec<CompletionItem>> {
        if self.name(db).get_span(db).end_byte >= req.offset {
            return None;
        }

        let mut items = vec![];
        if self.single(db).is_none() {
            items.push(static_snippets::task_single());
        }
        if self.interval(db).is_none() {
            items.push(static_snippets::task_interval());
        }
        if self.priority(db).is_none() {
            items.push(static_snippets::task_priority());
        }

        Some(items)
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

/// Check if the cursor is in the body of a POU, program, or method.
fn is_in_body<'db>(
    scope: &Scope<'db>,
    ctx: &mut CompletionCtx,
    db: &'db dyn WorkspaceDataBase,
) -> bool {
    let loc = match scope.kind {
        ScopeKind::Pou(pou) => ctx.located_pou_completion(pou, db).head_location,
        ScopeKind::Program(prog) => ctx.located_program_completion(prog, db).head_location,
        ScopeKind::MethodDecl(m) => {
            ctx.located_method_completion(MethodRef::Declared(m), db)
                .head_location
        }
        _ => return false,
    };
    loc.is_in_body()
}

/// If the scope is a function or method with a return type, push a self-return completion item.
fn maybe_add_self_return(
    db: &dyn WorkspaceDataBase,
    scope: &hir::hir_def::scope::Scope<'_>,
    items: &mut Vec<CompletionItem>,
) {
    match scope.kind {
        ScopeKind::Pou(Pou::Function(f)) => {
            if let Some(ret_spec) = f.return_type(db) {
                let ret_type = ret_spec.infer(db).type_name(db);
                items.push(self_return_completion(f.name(db).text(db), &ret_type));
            }
        }
        ScopeKind::MethodDecl(m) => {
            if let Some(ret_spec) = m.return_type(db) {
                let ret_type = ret_spec.infer(db).type_name(db);
                items.push(self_return_completion(m.name(db).text(db), &ret_type));
            }
        }
        _ => {}
    }
}

/// Build a completion item for the function/method's own name (used for return value assignment).
/// e.g. inside `FUNCTION foo : INT`, typing `fo` should suggest `foo` as an assignment target.
fn self_return_completion(name: &str, return_type: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_string(),
        label_details: Some(auto_lsp::lsp_types::CompletionItemLabelDetails {
            detail: Some(format!(": {return_type}")),
            description: Some("(Self)".into()),
        }),
        kind: Some(auto_lsp::lsp_types::CompletionItemKind::VARIABLE),
        sort_text: Some(format!("0{name}")),
        ..Default::default()
    }
}

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

/// The namespace a written path names from `scope`, exactly or as a prefix:
/// `Impl` inside `NAMESPACE Lib` is `Lib.Impl`, and `Sys` at the top is a
/// prefix of `System.Math`. The candidates are tried in the checker's order,
/// innermost enclosing namespace first, so the IDE agrees with resolution.
/// `None` when no namespace answers to any spelling.
pub(crate) fn resolve_namespace_prefix<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: hir::hir_def::scope::ScopeId<'db>,
    written: NamespacePath,
) -> Option<NamespacePath> {
    namespace_path_candidates(db, scope, written)
        .into_iter()
        .find(|&candidate| {
            if !namespace_index(db, candidate).is_empty() {
                return true;
            }
            let mut query = Query::new(candidate.to_string(db));
            query.prefix();
            SymbolSearch::new(|_, _| true)
                .with_query(query)
                .only_namespaces()
                .search(db)
                .namespaces()
                .next()
                .is_some()
        })
}
