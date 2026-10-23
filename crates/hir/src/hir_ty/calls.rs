// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Which bodies each body calls, and which ones are reached again from
//! themselves.
//!
//! A call's targets follow dispatch the way lowering does: `THIS.m()` and a
//! bare `m()` run the method of whatever instance `THIS` is, so they reach
//! the overrides in every POU deriving from the body's own; a call through
//! an INTERFACE reaches every implementer; any other call names one body.
//! The graph may hold a call that never happens, never miss one: lowering
//! gives each body on a cycle a stack frame, and a body it missed would
//! share its memory between activations.

use std::collections::VecDeque;

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    CallSite, HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, PathExprKind},
            invocation::InvocationKind,
        },
        pous::{class::MethodDecl, function::Function, function_block::FunctionBlock, pou::Pou},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::ScopeInference,
        oop::{MethodRef, class_members, descendants, explicit_bases},
        ty::CallableType,
    },
};

/// A body that runs when it is called: a FUNCTION's, a METHOD's, or a
/// FUNCTION_BLOCK's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum CallNode<'db> {
    Function(Function<'db>),
    Method(MethodDecl<'db>),
    Body(FunctionBlock<'db>),
}

impl<'db> CallNode<'db> {
    pub fn scope(self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            CallNode::Function(f) => f.scope_id(db),
            CallNode::Method(m) => m.get_scope_id(db),
            CallNode::Body(fb) => fb.scope_id(db),
        }
    }

    /// The POU `THIS` is in the body: a METHOD's owner, or the FB itself.
    pub fn this_pou(self, db: &'db dyn WorkspaceDataBase) -> Option<Pou<'db>> {
        match self {
            CallNode::Function(_) => None,
            CallNode::Body(fb) => Some(Pou::FunctionBlock(fb)),
            CallNode::Method(m) => {
                let parent = get_scope(db, m.get_scope_id(db)).parent?;
                match get_scope(db, parent).kind {
                    ScopeKind::Pou(pou) => Some(pou),
                    _ => None,
                }
            }
        }
    }

    /// The name a diagnostic shows: `Owner.method` for a METHOD.
    pub fn display_name(self, db: &'db dyn WorkspaceDataBase) -> String {
        match self {
            CallNode::Function(f) => f.get_name_with_case(db).text(db).to_string(),
            CallNode::Body(fb) => fb.get_name_with_case(db).text(db).to_string(),
            CallNode::Method(m) => {
                let name = m.get_name_with_case(db).text(db).to_string();
                match self.this_pou(db) {
                    Some(owner) => format!("{}.{name}", owner.get_name_with_case(db).text(db)),
                    None => name,
                }
            }
        }
    }

    /// Where the body is declared, to order bodies the same way on every run.
    fn position(self, db: &'db dyn WorkspaceDataBase) -> (&'db str, usize) {
        let span = match self {
            CallNode::Function(f) => f.get_name_span(db),
            CallNode::Method(m) => m.get_name_span(db),
            CallNode::Body(fb) => fb.get_name_span(db),
        };
        (self.scope(db).file(db).url(db).as_str(), span.start_byte)
    }
}

/// One call a body makes, where it is written, and the bodies it may run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call<'db> {
    pub site: CallSite<'db>,
    pub targets: Vec<CallNode<'db>>,
}

/// Every call `node` makes: in its statements, and in its locals'
/// initializers, which run at each call too. A POU member's initializer
/// cannot call (E0401), so an instance local brings no call of its own.
pub fn calls<'db>(db: &'db dyn WorkspaceDataBase, node: CallNode<'db>) -> Vec<Call<'db>> {
    scope_calls(db, node.scope(db), node.this_pou(db))
}

/// Every call the statements and initializers of `scope` make, where
/// `THIS` is `this`: a body's, or a PROGRAM's, which has no `THIS`.
fn scope_calls<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    this: Option<Pou<'db>>,
) -> Vec<Call<'db>> {
    let inference = scope.inference(db);
    let mut calls = Vec::new();
    for (call, resolved) in inference.resolved_calls() {
        let path = call.path(db);
        let targets = match resolved.callable {
            CallableType::Function(f) => vec![CallNode::Function(f)],
            CallableType::FunctionBlock(fb) => vec![CallNode::Body(fb)],
            CallableType::MethodDecl(method) => method_targets(db, inference, path, method, this),
        };
        calls.push(Call {
            site: CallSite::from_scoped(db, &path),
            targets,
        });
    }
    // `SUPER()` runs the base FB's body, which a CLASS has none of.
    for invocation in inference.invocations() {
        if invocation.kind(db) == InvocationKind::SuperBody
            && let Some(pou) = this
            && let Some(Pou::FunctionBlock(base)) = explicit_bases(db, pou).extends
        {
            calls.push(Call {
                site: CallSite::new(invocation.scope_id(db), invocation.keyword_id(db)),
                targets: vec![CallNode::Body(base)],
            });
        }
    }
    calls
}

/// The METHODs a call of `method` may run, sorted as lowering sorts the
/// forms: `THIS.inner.m()` and `receiver.m()` on the receiver's type,
/// `SUPER.m()` on the base's method, `THIS.m()` and a bare `m()` on the
/// instance's.
fn method_targets<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    path: BeginPathExpr<'db>,
    method: MethodRef<'db>,
    this: Option<Pou<'db>>,
) -> Vec<CallNode<'db>> {
    let invocation = path.invocation(db).map(|i| i.kind(db));
    let dispatch_on = match (invocation, path.expr(db).map(|pe| pe.expr(db))) {
        (_, Some(PathExprKind::Field(field))) => inference
            .type_of_path_expr(field.path)
            .normalize(db)
            .as_pou(db)
            .filter(|receiver| matches!(receiver, Pou::Interface(_)) || method.is_prototype()),
        (Some(InvocationKind::Super), _) => None,
        (Some(InvocationKind::This), _) | (None, Some(PathExprKind::VarAccess(_))) => this,
        _ => None,
    };
    // A PRIVATE method is its POU's own, which no override reaches.
    let private = matches!(method, MethodRef::Declared(decl)
        if decl.visibility(db).contains(crate::Visibility::PRIVATE));
    let Some(pou) = dispatch_on.filter(|_| !private) else {
        return match method {
            MethodRef::Declared(decl) => vec![CallNode::Method(decl)],
            MethodRef::Prototype(_) => Vec::new(),
        };
    };
    let name = method.get_name_ident(db);
    std::iter::once(pou)
        .chain(descendants(db, pou))
        .filter_map(|pou| class_members(db, pou).implementation(&name))
        .map(CallNode::Method)
        .collect::<FxHashSet<_>>()
        .into_iter()
        .collect()
}

/// The bodies of `pou`: a FUNCTION's or a FUNCTION_BLOCK's, and those of
/// the METHODs it declares.
pub fn bodies<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Vec<CallNode<'db>> {
    let mut bodies = match pou {
        Pou::Function(f) => vec![CallNode::Function(f)],
        Pou::FunctionBlock(fb) => vec![CallNode::Body(fb)],
        _ => Vec::new(),
    };
    if let Some(methods) = pou.get_scope_id(db).method_declarations(db) {
        bodies.extend(methods.iter().map(|m| CallNode::Method(*m)));
    }
    bodies
}

/// The bodies `file` declares, and those its PROGRAMs call: what a check of
/// the file asks a graph about, the bodies its calls copy for the arrays
/// they bind included.
pub fn roots<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> Vec<CallNode<'db>> {
    let index = crate::hir_def::semantic_index::semantic_index(db, file);
    let mut roots: Vec<CallNode<'db>> = index
        .global_pous
        .iter()
        .copied()
        .chain(
            index
                .namespaces
                .iter()
                .flat_map(|ns| ns.pous(db).iter().copied()),
        )
        .flat_map(|pou| bodies(db, pou))
        .collect();
    for program in index.programs.iter() {
        roots.extend(
            scope_calls(db, program.get_scope_id(db), None)
                .into_iter()
                .flat_map(|call| call.targets),
        );
    }
    roots
}

/// The calls between the bodies some roots reach, directly or through
/// others, and the cycles they make. A pass builds it once, from the files
/// it checks or the bodies it lowers, and asks it about any body those
/// reach. It is no salsa query: it reaches across files, so it would run
/// again for an edit of any of them, and the set of bodies each body
/// reached grew with the square of a chain of calls.
pub struct CallGraph<'db> {
    nodes: Vec<CallNode<'db>>,
    index: FxHashMap<CallNode<'db>, usize>,
    /// What each body may call, in the order of where the callees are
    /// declared.
    callees: Vec<Vec<usize>>,
    /// The strongly connected component of each body: two bodies reach each
    /// other exactly when they share one.
    component: Vec<usize>,
    /// Whether the body may call itself: its component holds another body,
    /// or it calls itself.
    recursive: Vec<bool>,
}

impl<'db> CallGraph<'db> {
    /// The graph of what `files` call: from their bodies, and from the
    /// calls of their PROGRAMs.
    pub fn of_files(db: &'db dyn WorkspaceDataBase, files: impl IntoIterator<Item = File>) -> Self {
        Self::new(db, files.into_iter().flat_map(|file| roots(db, file)))
    }

    /// The graph of what `roots` call, directly or through others.
    pub fn new(
        db: &'db dyn WorkspaceDataBase,
        roots: impl IntoIterator<Item = CallNode<'db>>,
    ) -> Self {
        let mut graph = CallGraph {
            nodes: Vec::new(),
            index: FxHashMap::default(),
            callees: Vec::new(),
            component: Vec::new(),
            recursive: Vec::new(),
        };
        // The bodies whose callees are still to ask for.
        let mut pending: Vec<usize> = roots
            .into_iter()
            .filter_map(|root| graph.add(root))
            .collect();
        while let Some(at) = pending.pop() {
            let mut targets: Vec<CallNode<'db>> = calls(db, graph.nodes[at])
                .into_iter()
                .flat_map(|call| call.targets)
                .collect();
            targets.sort_by_cached_key(|target| target.position(db));
            targets.dedup();
            graph.callees[at] = targets
                .into_iter()
                .map(|target| {
                    graph.add(target).inspect(|new| pending.push(*new));
                    graph.index[&target]
                })
                .collect();
        }
        graph.component = components(&graph.callees);
        let mut size = vec![0usize; graph.nodes.len()];
        for &component in &graph.component {
            size[component] += 1;
        }
        graph.recursive = (0..graph.nodes.len())
            .map(|at| size[graph.component[at]] > 1 || graph.callees[at].contains(&at))
            .collect();
        graph
    }

    /// `node`'s place in the graph, when it is new there.
    fn add(&mut self, node: CallNode<'db>) -> Option<usize> {
        if self.index.contains_key(&node) {
            return None;
        }
        let at = self.nodes.len();
        self.nodes.push(node);
        self.index.insert(node, at);
        self.callees.push(Vec::new());
        Some(at)
    }

    /// Whether `node` may call itself, directly or through others: each of
    /// its activations then needs memory of its own. `node` is one the
    /// roots reach.
    pub fn is_recursive(&self, node: CallNode<'db>) -> bool {
        debug_assert!(
            self.index.contains_key(&node),
            "{node:?} is no body the graph's roots reach"
        );
        self.index.get(&node).is_some_and(|&i| self.recursive[i])
    }

    /// Whether `a` and `b` are on one cycle of calls: each reaches the
    /// other, and either reaches itself through it.
    pub fn on_one_cycle(&self, a: CallNode<'db>, b: CallNode<'db>) -> bool {
        match (self.index.get(&a), self.index.get(&b)) {
            (Some(&a), Some(&b)) => self.component[a] == self.component[b] && self.recursive[a],
            _ => false,
        }
    }

    /// Every body that may call itself.
    pub fn recursive(&self) -> impl Iterator<Item = CallNode<'db>> + '_ {
        self.nodes
            .iter()
            .zip(&self.recursive)
            .filter(|(_, recursive)| **recursive)
            .map(|(node, _)| *node)
    }

    /// The shortest chain of calls from `from` to `to`, both included, the
    /// same on every run; `None` when they are on no cycle together. Such a
    /// chain stays on the cycle, so only its bodies are searched.
    pub fn chain(&self, from: CallNode<'db>, to: CallNode<'db>) -> Option<Vec<CallNode<'db>>> {
        let (&start, &end) = (self.index.get(&from)?, self.index.get(&to)?);
        let component = self.component[start];
        if self.component[end] != component {
            return None;
        }
        let mut came_from: FxHashMap<usize, usize> = FxHashMap::default();
        let mut pending = VecDeque::from([start]);
        while let Some(at) = pending.pop_front() {
            if at == end {
                let mut chain = vec![self.nodes[end]];
                let mut at = end;
                while at != start {
                    at = came_from[&at];
                    chain.push(self.nodes[at]);
                }
                chain.reverse();
                return Some(chain);
            }
            for &callee in &self.callees[at] {
                if self.component[callee] == component
                    && callee != start
                    && !came_from.contains_key(&callee)
                {
                    came_from.insert(callee, at);
                    pending.push_back(callee);
                }
            }
        }
        None
    }
}

/// The strongly connected component of each node of `edges`, by Tarjan's
/// search: two nodes share one exactly when each reaches the other. The
/// search keeps its own stack: a chain of thousands of calls would overflow
/// the thread's.
fn components(edges: &[Vec<usize>]) -> Vec<usize> {
    const UNSEEN: usize = usize::MAX;
    // When the search reached each node, and the earliest node still on the
    // stack each one reaches.
    let mut order = vec![UNSEEN; edges.len()];
    let mut low = vec![UNSEEN; edges.len()];
    let mut on_stack = vec![false; edges.len()];
    let mut stack = Vec::new();
    let mut component = vec![UNSEEN; edges.len()];
    let mut reached = 0;
    let mut components = 0;
    for root in 0..edges.len() {
        if order[root] != UNSEEN {
            continue;
        }
        // Each node on the way down, with the next of its edges to follow.
        let mut path = vec![(root, 0)];
        order[root] = reached;
        low[root] = reached;
        reached += 1;
        stack.push(root);
        on_stack[root] = true;
        while let Some(&(at, next)) = path.last() {
            if let Some(&to) = edges[at].get(next) {
                path.last_mut().expect("the node just read").1 += 1;
                if order[to] == UNSEEN {
                    order[to] = reached;
                    low[to] = reached;
                    reached += 1;
                    stack.push(to);
                    on_stack[to] = true;
                    path.push((to, 0));
                } else if on_stack[to] {
                    low[at] = low[at].min(order[to]);
                }
                continue;
            }
            path.pop();
            if let Some(&(from, _)) = path.last() {
                low[from] = low[from].min(low[at]);
            }
            if low[at] == order[at] {
                loop {
                    let member = stack.pop().expect("a component on the stack");
                    on_stack[member] = false;
                    component[member] = components;
                    if member == at {
                        break;
                    }
                }
                components += 1;
            }
        }
    }
    component
}

/// Every instance a file's bodies reach through a path of two steps or more:
/// `o.t()` runs the body of `o`'s member `t`, `f(io := o.t)` and
/// `REF(o.t)` hand it on. A member its own block never calls may be called,
/// or handed on, from outside the block this way.
#[salsa::tracked(returns(ref))]
pub fn instances_reached_through_paths<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: auto_lsp::default::db::file::File,
) -> Vec<crate::hir_def::pous::variable::VariableDecl<'db>> {
    use crate::hir_def::expressions::expression::{
        ExprKind, PrimaryExpr, RefValue, VariableAccessKind,
    };
    use crate::hir_ty::body::ParamBinding;

    let index = crate::hir_def::semantic_index::semantic_index(db, file);
    let mut scopes: Vec<ScopeId<'db>> = Vec::new();
    let pous = index.global_pous.iter().copied().chain(
        index
            .namespaces
            .iter()
            .flat_map(|ns| ns.pous(db).iter().copied()),
    );
    for pou in pous {
        let scope = pou.get_scope_id(db);
        scopes.push(scope);
        if let Some(methods) = scope.method_declarations(db) {
            scopes.extend(methods.iter().map(|m| m.get_scope_id(db)));
        }
    }
    scopes.extend(
        index
            .programs
            .iter()
            .map(|program| program.get_scope_id(db)),
    );

    let mut reached = Vec::new();
    for scope in scopes {
        let inference = scope.inference(db);
        let mut reach = |path| {
            if let Some(member) = member_of(db, inference, path)
                && !reached.contains(&member)
            {
                reached.push(member);
            }
        };
        for (call, resolved) in inference.resolved_calls() {
            if matches!(resolved.callable, CallableType::FunctionBlock(_))
                && let Some(path) = call.path(db).expr(db)
            {
                reach(path);
            }
            for (param, binding) in &resolved.params {
                let access = match binding {
                    ParamBinding::Values(values) if param.is_in_out(db) => {
                        values.first().and_then(|value| match value.expr(db) {
                            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => {
                                Some(*access)
                            }
                            _ => None,
                        })
                    }
                    ParamBinding::Output { variable, .. } => Some(*variable),
                    _ => None,
                };
                if let Some(access) = access
                    && let VariableAccessKind::Symbolic(begin) = access.kind(db)
                    && let Some(path) = begin.expr(db)
                {
                    reach(path);
                }
            }
        }
        for (expr, _) in inference.typed_exprs() {
            if let ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                value: RefValue::Address(begin),
            }) = expr.expr(db)
                && let Some(path) = begin.expr(db)
            {
                reach(path);
            }
        }
    }
    reached
}

/// The member a path of two steps or more ends on: `t` for `o.t`, `ts` for
/// `o.ts[i]`. `None` for a single name.
fn member_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    path: crate::hir_def::expressions::expression::PathExpr<'db>,
) -> Option<crate::hir_def::pous::variable::VariableDecl<'db>> {
    use crate::hir_ty::expr_store::PathExprWalkStep;
    let steps = path.flatten(db);
    steps
        .iter()
        .enumerate()
        .skip(1)
        .rev()
        .find_map(|(_, step)| match step {
            PathExprWalkStep::Field { expr, .. } => inference.variable_for_path_expr(*expr),
            _ => None,
        })
}

/// Whether a body of the workspace reaches `instance` through a path, as
/// [`instances_reached_through_paths`] collects them. A library's bodies
/// cannot name the workspace's instances.
pub fn reached_through_a_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    instance: crate::hir_def::pous::variable::VariableDecl<'db>,
) -> bool {
    crate::hir_ty::index_graphs::workspace_files(db)
        .any(|file| instances_reached_through_paths(db, file).contains(&instance))
}

/// Every member a file's bodies name through a path: `inner` of
/// `f.inner` or `THIS.inner`, read, written or called, the configuration's
/// paths included. What a block keeps between runs, another body may read
/// this way.
#[salsa::tracked(returns(ref))]
pub fn members_named_through_paths<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: auto_lsp::default::db::file::File,
) -> Vec<crate::hir_def::pous::variable::VariableDecl<'db>> {
    let index = crate::hir_def::semantic_index::semantic_index(db, file);
    let mut scopes: Vec<ScopeId<'db>> = Vec::new();
    let pous = index.global_pous.iter().copied().chain(
        index
            .namespaces
            .iter()
            .flat_map(|ns| ns.pous(db).iter().copied()),
    );
    for pou in pous {
        if matches!(pou, Pou::DataType(_) | Pou::Interface(_)) {
            continue;
        }
        let scope = pou.get_scope_id(db);
        scopes.push(scope);
        if let Some(methods) = scope.method_declarations(db) {
            scopes.extend(methods.iter().map(|m| m.get_scope_id(db)));
        }
    }
    scopes.extend(
        index
            .programs
            .iter()
            .map(|program| program.get_scope_id(db)),
    );
    scopes.extend(index.configs.iter().map(|config| config.scope_id(db)));

    let mut named = Vec::new();
    for scope in scopes {
        let inference = scope.inference(db);
        for (path, _) in inference.typed_path_exprs() {
            if let PathExprKind::Field(_) = path.expr(db)
                && let Some(member) = inference.variable_for_path_expr(path)
                && !named.contains(&member)
            {
                named.push(member);
            }
        }
    }
    named
}

/// Whether a body of the workspace names `member` through a path, as
/// [`members_named_through_paths`] collects them. A library's bodies
/// cannot name the workspace's members.
pub fn named_through_a_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    member: crate::hir_def::pous::variable::VariableDecl<'db>,
) -> bool {
    crate::hir_ty::index_graphs::workspace_files(db)
        .any(|file| members_named_through_paths(db, file).contains(&member))
}

#[cfg(test)]
mod tests {
    use super::components;

    /// Whether each node reaches each other, through one edge or more.
    fn reaches(edges: &[Vec<usize>]) -> Vec<Vec<bool>> {
        (0..edges.len())
            .map(|start| {
                let mut seen = vec![false; edges.len()];
                let mut pending = edges[start].clone();
                while let Some(at) = pending.pop() {
                    if !std::mem::replace(&mut seen[at], true) {
                        pending.extend(&edges[at]);
                    }
                }
                seen
            })
            .collect()
    }

    /// Random graphs, self edges included: two nodes share a component
    /// exactly when each reaches the other.
    #[test]
    fn a_component_is_the_nodes_that_reach_each_other() {
        let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for size in [1, 2, 5, 12, 30] {
            for per_mille in [50, 150, 400] {
                for _ in 0..20 {
                    let edges: Vec<Vec<usize>> = (0..size)
                        .map(|_| (0..size).filter(|_| next() % 1000 < per_mille).collect())
                        .collect();
                    let component = components(&edges);
                    let reach = reaches(&edges);
                    for a in 0..size {
                        for b in 0..size {
                            assert_eq!(
                                component[a] == component[b],
                                a == b || (reach[a][b] && reach[b][a]),
                                "{a} and {b} in {edges:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// The search keeps its own stack: a cycle through a million nodes,
    /// searched on a thread of 64 KiB, is one component.
    #[test]
    fn a_cycle_of_a_million_nodes_is_one_component() {
        let edges: Vec<Vec<usize>> = (0..1_000_000)
            .map(|at| vec![(at + 1) % 1_000_000])
            .collect();
        let component = std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || components(&edges))
            .unwrap()
            .join()
            .unwrap();
        assert!(component.iter().all(|c| *c == component[0]));
    }
}
