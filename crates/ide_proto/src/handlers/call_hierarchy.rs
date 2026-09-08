//! Call hierarchy: what calls this, and what does this call.
//!
//! Both directions read [`BodyInferenceResult::resolved_calls`], the plan
//! inference assembled when it checked each call. Re-walking the statement
//! tree to find calls would re-decide which overload a site picked, and the
//! hierarchy would then disagree with the diagnostics about the same call.

use auto_lsp::{
    default::db::file::File,
    lsp_types::{
        CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Range, SymbolKind,
    },
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        hir_node::HirNode, pous::pou::Pou, program::ProgramDecl, scope::ScopeId,
        semantic_index::semantic_index,
    },
    hir_ty::{
        body::infer_body,
        head::inheritance::MethodRef,
        infer::Infer,
        ty::{CallableType, Type},
    },
};
use rustc_hash::FxHashMap;

/// A body the hierarchy can stand on: something with calls in it, something
/// calls can name, or both.
///
/// A PROGRAM is only ever a caller — a TASK schedules it, no body invokes it —
/// so it answers outgoing calls and never appears as an incoming one.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallTarget<'db> {
    Callable(CallableType<'db>),
    Program(ProgramDecl<'db>),
}

impl<'db> CallTarget<'db> {
    fn scope(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            CallTarget::Callable(c) => c.get_scope_id(db),
            CallTarget::Program(p) => p.get_scope_id(db),
        }
    }

    fn name(&self, db: &'db dyn WorkspaceDataBase) -> String {
        match self {
            CallTarget::Callable(CallableType::Function(f)) => f.get_name_ident(db).text(db),
            CallTarget::Callable(CallableType::FunctionBlock(fb)) => fb.get_name_ident(db).text(db),
            CallTarget::Callable(CallableType::MethodDecl(m)) => m.get_name_ident(db).text(db),
            CallTarget::Program(p) => p.get_name_ident(db).text(db),
        }
        .to_string()
    }

    fn kind(&self) -> SymbolKind {
        match self {
            CallTarget::Callable(CallableType::MethodDecl(_)) => SymbolKind::METHOD,
            CallTarget::Program(_) => SymbolKind::MODULE,
            _ => SymbolKind::FUNCTION,
        }
    }

    fn spans(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> (
        File,
        auto_lsp::tree_sitter::Range,
        auto_lsp::tree_sitter::Range,
    ) {
        let (whole, name) = match self {
            CallTarget::Callable(CallableType::Function(f)) => {
                (f.get_span(db), f.get_name_span(db))
            }
            CallTarget::Callable(CallableType::FunctionBlock(fb)) => {
                (fb.get_span(db), fb.get_name_span(db))
            }
            CallTarget::Callable(CallableType::MethodDecl(m)) => {
                (m.get_span(db), m.get_name_span(db))
            }
            CallTarget::Program(p) => (p.get_span(db), p.get_name_span(db)),
        };
        (self.scope(db).file(db), whole, name)
    }

    fn to_item(&self, db: &'db dyn WorkspaceDataBase) -> CallHierarchyItem {
        let (file, whole, name_span) = self.spans(db);
        CallHierarchyItem {
            name: self.name(db),
            kind: self.kind(),
            tags: None,
            detail: None,
            uri: file.url(db).clone(),
            range: hir::denormalize(db, file, &whole).unwrap_or_default(),
            selection_range: hir::denormalize(db, file, &name_span).unwrap_or_default(),
            data: None,
        }
    }
}

/// The callable the cursor is on: its declaration, or a call site naming it.
pub fn call_target_at<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> Option<CallTarget<'db>> {
    let callable = match node {
        HirNode::PouDecl(Pou::Function(f)) => CallableType::Function(*f),
        HirNode::PouDecl(Pou::FunctionBlock(fb)) => CallableType::FunctionBlock(*fb),
        HirNode::MethodRef(m) => CallableType::MethodDecl(*m),
        HirNode::Program(p) => return Some(CallTarget::Program(*p)),
        // On a call site, the hierarchy is the CALLEE's: that is what the
        // reader pointed at.
        HirNode::PathExpr(p) => match p.infer(db) {
            Type::CallableType(c) => c,
            Type::Function(f) => CallableType::Function(f),
            Type::MethodDecl(m) => CallableType::MethodDecl(m),
            _ => return None,
        },
        _ => return None,
    };
    Some(CallTarget::Callable(callable))
}

pub fn prepare<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> Option<Vec<CallHierarchyItem>> {
    let target = call_target_at(db, node)?;
    Some(vec![target.to_item(db)])
}

/// Every body in a file that can hold a call.
///
/// `SemanticIndex::namespaces` is FLAT, so recursing into a namespace's own
/// children would visit a nested one twice - the shape that made every lint
/// fire once per ancestor.
fn bodies_in<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> Vec<CallTarget<'db>> {
    let sema = semantic_index(db, file);
    let mut out = Vec::new();

    let push_pou = |pou: &Pou<'db>, out: &mut Vec<CallTarget<'db>>| {
        match pou {
            Pou::Function(f) => out.push(CallTarget::Callable(CallableType::Function(*f))),
            Pou::FunctionBlock(fb) => {
                out.push(CallTarget::Callable(CallableType::FunctionBlock(*fb)))
            }
            _ => {}
        }
        // A method carries its own body and its own scope.
        if let Some(methods) = pou.get_scope_id(db).method_declarations(db) {
            for m in methods {
                out.push(CallTarget::Callable(CallableType::MethodDecl(
                    MethodRef::Declared(*m),
                )));
            }
        }
    };

    for pou in sema.global_pous.iter() {
        push_pou(pou, &mut out);
    }
    for program in sema.programs.iter() {
        out.push(CallTarget::Program(*program));
    }
    for ns in sema.namespaces.iter() {
        for pou in ns.pous(db).iter() {
            push_pou(pou, &mut out);
        }
    }
    out
}

/// The call sites in `caller`'s body, grouped by what they call.
fn calls_from<'db>(
    db: &'db dyn WorkspaceDataBase,
    caller: CallTarget<'db>,
) -> FxHashMap<CallableType<'db>, Vec<Range>> {
    let scope = caller.scope(db);
    let file = scope.file(db);
    let results = infer_body(db, scope);
    let mut by_callee: FxHashMap<CallableType<'db>, Vec<Range>> = FxHashMap::default();

    for (call, resolved) in results.resolved_calls.iter() {
        let span = call.path(db).get_span(db);
        if let Some(range) = hir::denormalize(db, file, &span) {
            by_callee.entry(resolved.callable).or_default().push(range);
        }
    }
    by_callee
}

pub fn outgoing<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> Option<Vec<CallHierarchyOutgoingCall>> {
    let caller = call_target_at(db, node)?;
    let mut out: Vec<CallHierarchyOutgoingCall> = calls_from(db, caller)
        .into_iter()
        .map(|(callee, from_ranges)| CallHierarchyOutgoingCall {
            to: CallTarget::Callable(callee).to_item(db),
            from_ranges,
        })
        .collect();
    out.sort_by(|a, b| a.to.name.cmp(&b.to.name));
    Some(out)
}

pub fn incoming<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> Option<Vec<CallHierarchyIncomingCall>> {
    let target = match call_target_at(db, node)? {
        CallTarget::Callable(c) => c,
        // Nothing invokes a PROGRAM: a TASK schedules it.
        CallTarget::Program(_) => return Some(Vec::new()),
    };
    let name = CallTarget::Callable(target).name(db).to_lowercase();

    let mut out = Vec::new();
    for file in db.get_files().iter() {
        // A file that never spells the name cannot call it. The same
        // pre-filter `references` uses, and Unicode-aware for the same
        // reason: an ASCII fold would skip a file whose only mention of
        // `MÄX` is spelled `mäx`.
        if !file.document(db).as_str().to_lowercase().contains(&name) {
            continue;
        }
        for caller in bodies_in(db, *file) {
            if let Some(from_ranges) = calls_from(db, caller).remove(&target)
                && !from_ranges.is_empty()
            {
                out.push(CallHierarchyIncomingCall {
                    from: caller.to_item(db),
                    from_ranges,
                });
            }
        }
    }
    out.sort_by(|a, b| {
        (a.from.uri.as_str(), a.from.name.clone()).cmp(&(b.from.uri.as_str(), b.from.name.clone()))
    });
    Some(out)
}
