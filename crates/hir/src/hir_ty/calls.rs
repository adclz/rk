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
    fn position(self, db: &'db dyn WorkspaceDataBase) -> (String, usize) {
        let span = match self {
            CallNode::Function(f) => f.get_name_span(db),
            CallNode::Method(m) => m.get_name_span(db),
            CallNode::Body(fb) => fb.get_name_span(db),
        };
        (self.scope(db).file(db).url(db).to_string(), span.start_byte)
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
    let inference = node.scope(db).inference(db);
    let this = node.this_pou(db);
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

/// The bodies `node` may call directly.
#[salsa::tracked(returns(ref))]
pub fn callees<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: CallNode<'db>,
) -> FxHashSet<CallNode<'db>> {
    calls(db, node)
        .into_iter()
        .flat_map(|call| call.targets)
        .collect()
}

/// Every body `node` may call, directly or through others. A cycle of calls
/// is a salsa cycle here: it starts from nothing reached and grows until the
/// sets stop changing.
#[salsa::tracked(returns(ref), cycle_initial = nothing_reached)]
pub fn reachable<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: CallNode<'db>,
) -> FxHashSet<CallNode<'db>> {
    let mut reached = FxHashSet::default();
    for callee in callees(db, node) {
        reached.insert(*callee);
        reached.extend(reachable(db, *callee).iter().copied());
    }
    reached
}

fn nothing_reached<'db>(
    _db: &'db dyn WorkspaceDataBase,
    _id: salsa::Id,
    _node: CallNode<'db>,
) -> FxHashSet<CallNode<'db>> {
    FxHashSet::default()
}

/// Whether `node` may call itself, directly or through others: each of its
/// activations then needs memory of its own.
pub fn is_recursive<'db>(db: &'db dyn WorkspaceDataBase, node: CallNode<'db>) -> bool {
    reachable(db, node).contains(&node)
}

/// The shortest chain of calls from `from` to `to`, both included, in the
/// same order on every run; `None` when `from` never reaches `to`.
pub fn call_chain<'db>(
    db: &'db dyn WorkspaceDataBase,
    from: CallNode<'db>,
    to: CallNode<'db>,
) -> Option<Vec<CallNode<'db>>> {
    let mut came_from: FxHashMap<CallNode<'db>, CallNode<'db>> = FxHashMap::default();
    let mut pending = VecDeque::from([from]);
    while let Some(node) = pending.pop_front() {
        if node == to {
            let mut chain = vec![to];
            let mut at = to;
            while at != from {
                at = came_from[&at];
                chain.push(at);
            }
            chain.reverse();
            return Some(chain);
        }
        let mut next: Vec<_> = callees(db, node).iter().copied().collect();
        next.sort_by_cached_key(|callee| callee.position(db));
        for callee in next {
            if callee != from && !came_from.contains_key(&callee) {
                came_from.insert(callee, node);
                pending.push_back(callee);
            }
        }
    }
    None
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
    db.get_files()
        .iter()
        .any(|file| instances_reached_through_paths(db, *file).contains(&instance))
}
