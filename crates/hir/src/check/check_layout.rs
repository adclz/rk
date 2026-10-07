// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What a file's declarations take in memory: a type that contains itself
//! (E13xx), storage past what a module addresses (E0322), and the frame of a
//! recursive call larger than the stack (E1430). Each file checks the
//! declarations it makes and the calls it writes.

use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic, e03_type::TypeError, e13_recursion::RecursionError,
        e14_config::ConfigError,
    },
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        namespace::NamespaceDecl,
        pous::{
            pou::Pou,
            variable::{StorageClass, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::{SemanticIndex, get_scope},
    },
    hir_ty::{
        body::{ResolvedCall, ScopeInference},
        calls::{CallNode, is_recursive},
        frame::{self, Shapes},
        infer::Infer,
        layout,
        ty::Type,
    },
};

use db::WorkspaceDataBase;

/// E0322 on the declarations of `scope` whose storage passes what a module
/// addresses: a TYPE, a variable, an instance whose members together do.
pub fn check_storage<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let kind = get_scope(db, scope).kind;
    if let ScopeKind::Pou(pou @ Pou::DataType(dt)) = kind {
        storage(
            db,
            dt.spec(db),
            CallSite::new(scope, pou.get_name_id(db)),
            errors,
        );
    }
    // A VAR_EXTERNAL's storage is its global's, reported there.
    for var in scope.variables(db).into_iter().flatten() {
        if !var.is_external(db) {
            storage(
                db,
                var.spec(db),
                CallSite::new(scope, var.get_name_id(db)),
                errors,
            );
        }
    }
    let (instance, what, site) = match kind {
        ScopeKind::Pou(pou @ (Pou::FunctionBlock(_) | Pou::Class(_))) => (
            layout::instance_layout(db, pou).as_ref(),
            format!("an instance of '{}'", pou.get_name_with_case(db).text(db)),
            CallSite::new(scope, pou.get_name_id(db)),
        ),
        ScopeKind::Program(program) => (
            layout::program_layout(db, program).as_ref(),
            format!("the PROGRAM '{}'", program.get_name_with_case(db).text(db)),
            CallSite::new(scope, program.get_name_id(db)),
        ),
        _ => return,
    };
    // Each member that is too large is reported on its own.
    if let Some(instance) = instance
        && !instance.whole.fits()
        && instance.fields.iter().all(|field| field.layout.fits())
    {
        errors.push(
            TypeError::StorageTooLarge {
                site,
                what,
                size: instance.whole.size,
            }
            .to_diagnostic(db, scope.file(db)),
        );
    }
}

/// E0322 on a declaration whose storage passes what a module addresses, at
/// the innermost part that does: a STRUCT holding an array too large is
/// reported at the array, and a variable of a TYPE too large at the TYPE,
/// not again. A STRUCT too large as a whole is reported at `name`, the
/// declaration's, rather than across its lines.
fn storage<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: Spec<'db>,
    name: CallSite<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let Some((part, size)) = oversized(db, spec) else {
        return;
    };
    let (what, site) = match part.kind(db) {
        SpecKind::Struct(_) if part == spec => ("this STRUCT", name),
        SpecKind::Struct(_) => ("this STRUCT", CallSite::from_scoped(db, &part)),
        SpecKind::SizedString(_) => ("this STRING", CallSite::from_scoped(db, &part)),
        _ => ("this array", CallSite::from_scoped(db, &part)),
    };
    errors.push(
        TypeError::StorageTooLarge {
            site,
            what: what.to_string(),
            size,
        }
        .to_diagnostic(db, name.scope.file(db)),
    );
}

/// The innermost part of `spec` whose storage passes what a module
/// addresses, and its size. A named type is reported where it is declared,
/// so neither it nor what holds it is reported here.
fn oversized<'db>(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Option<(Spec<'db>, u64)> {
    let parts: Vec<Spec<'db>> = match spec.kind(db) {
        SpecKind::Struct(strukt) => strukt.elements(db).iter().map(|e| e.spec(db)).collect(),
        SpecKind::Array(array) => vec![array.of_type(db)],
        SpecKind::SizedString(_) => Vec::new(),
        _ => return None,
    };
    for part in &parts {
        if let Some(found) = oversized(db, *part) {
            return Some(found);
        }
    }
    let fits = |spec| layout::of_spec(db, spec).is_none_or(layout::Layout::fits);
    if !parts.iter().all(|part| fits(*part)) {
        return None;
    }
    let whole = layout::of_spec(db, spec)?;
    (!whole.fits()).then_some((spec, whole.size))
}

/// The frames of recursive calls `scope` decides: its own body's, when it
/// may call itself, and those of the copies its calls create of a body with
/// `ARRAY[*]` parameters. One larger than the stack `stack_size` sets is
/// E1430, and one past what a module addresses E0322.
pub fn check_frames<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let stack = db::config_file::get_config(db)
        .settings
        .as_ref()
        .and_then(|settings| settings.stack_size)
        .map(|size| size.bytes());
    let fits =
        |frame: u64| frame <= u64::from(u32::MAX) && stack.is_none_or(|stack| frame <= stack);
    let file = scope.file(db);
    let too_large = |site, what: String, frame: u64| match stack {
        _ if frame > u64::from(u32::MAX) => Some(
            TypeError::StorageTooLarge {
                site,
                what,
                size: frame,
            }
            .to_diagnostic(db, file),
        ),
        Some(stack) if frame > stack => Some(
            ConfigError::FrameLargerThanStack {
                site,
                what,
                frame,
                stack,
            }
            .to_diagnostic(db, file),
        ),
        _ => None,
    };

    let node = match get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => Some((CallNode::Function(f), f.get_name_id(db))),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => Some((CallNode::Body(fb), fb.get_name_id(db))),
        ScopeKind::MethodDecl(m) => Some((CallNode::Method(m), m.get_name_id(db))),
        _ => None,
    };
    if let Some((node, name)) = node
        && is_recursive(db, node)
    {
        let size = frame::frame(db, node, &Shapes::default());
        if !fits(size) {
            let what = format!("each call of '{}'", node.display_name(db));
            errors.extend(too_large(CallSite::new(scope, name), what, size));
        }
    }

    // A copy is reported where it is created, when the arrays it binds are
    // what make it too large.
    let inference = scope.inference(db);
    let mut calls: Vec<_> = inference.resolved_calls().collect();
    calls.sort_by_key(|(call, _)| call.path(db).get_span(db).start_byte);
    for (call, resolved) in calls {
        let copy = copies(db, inference, resolved)
            .into_iter()
            .find(|(node, size)| !fits(*size) && fits(frame::frame(db, *node, &Shapes::default())));
        if let Some((node, size)) = copy {
            let what = format!(
                "each call of the copy of '{}' for these arrays",
                node.display_name(db)
            );
            errors.extend(too_large(
                CallSite::from_scoped(db, &call.path(db)),
                what,
                size,
            ));
        }
    }
}

/// The copies of recursive bodies a call creates, and the frame of each: the
/// callee's, and those its copy creates by passing its arrays on. A copy a
/// body's own arrays decide is checked where that body is written.
fn copies<'db>(
    db: &'db dyn WorkspaceDataBase,
    inference: ScopeInference<'db>,
    call: &ResolvedCall<'db>,
) -> Vec<(CallNode<'db>, u64)> {
    let mut work: Vec<_> = frame::copy_of(db, inference, call, &Shapes::default())
        .into_iter()
        .collect();
    let mut seen: Vec<(CallNode<'db>, Shapes<'db>)> = Vec::new();
    let mut found = Vec::new();
    while let Some((node, shapes)) = work.pop() {
        if seen.iter().any(|(n, s)| *n == node && *s == shapes) {
            continue;
        }
        if is_recursive(db, node) {
            found.push((node, frame::frame(db, node, &shapes)));
        }
        let inner = node.scope(db).inference(db);
        for inner_call in frame::body_calls(db, node) {
            if frame::copy_of(db, inner, inner_call, &Shapes::default()).is_none() {
                work.extend(frame::copy_of(db, inner, inner_call, &shapes));
            }
        }
        seen.push((node, shapes));
    }
    found
}

/// DFS-based cycle detection on type dependency graph
///
/// Time complexity: O(V + E)
pub struct TypeDependencyGraph<'db> {
    db: &'db dyn WorkspaceDataBase,

    /// POU -> directly referenced POUs
    edges: FxHashMap<Pou<'db>, FxHashSet<Pou<'db>>>,

    /// (from, to) -> callsites
    callsites: FxHashMap<(Pou<'db>, Pou<'db>), Vec<CallSite<'db>>>,
}

impl<'db> TypeDependencyGraph<'db> {
    pub fn new(db: &'db dyn WorkspaceDataBase, semantic_index: &SemanticIndex<'db>) -> Self {
        let mut edges = FxHashMap::default();
        let callsites = FxHashMap::default();

        // Seed graph with file-local POUs only
        for &pou in semantic_index.global_pous.iter() {
            edges.entry(pou).or_insert_with(FxHashSet::default);
        }

        // The namespace list is FLAT (nested included), so each namespace
        // seeds its own POUs exactly once — no recursion into children.
        for ns in semantic_index.namespaces.iter() {
            for &pou in ns.pous(db).iter() {
                edges.entry(pou).or_default();
            }
        }

        Self {
            db,
            edges,
            callsites,
        }
    }

    /// Lazily ensure that `pou` has its dependencies extracted
    fn ensure_edges(&mut self, pou: Pou<'db>) {
        // Already expanded
        if !self.edges.get(&pou).is_none_or(FxHashSet::is_empty) {
            return;
        }

        let mut deps = FxHashSet::default();
        Self::extract_type_dependencies(self.db, pou, &mut deps, &mut self.callsites);

        // Insert outgoing edges
        self.edges.insert(pou, deps.clone());

        // Ensure referenced POUs exist as nodes
        for dep in deps {
            self.edges.entry(dep).or_default();
        }
    }

    fn extract_type_dependencies(
        db: &'db dyn WorkspaceDataBase,
        pou: Pou<'db>,
        deps: &mut FxHashSet<Pou<'db>>,
        callsites: &mut FxHashMap<(Pou<'db>, Pou<'db>), Vec<CallSite<'db>>>,
    ) {
        let def_map = pou.get_scope_id(db).def_map(db);

        // The members an instance holds by value. A VAR_IN_OUT member is the
        // address of the caller's instance, like a REF_TO, and VAR_TEMP and
        // VAR_EXTERNAL are no part of the instance at all.
        let contained = def_map.global_variables.values().filter(|var| {
            var.storage_class(db) == StorageClass::InstanceMember
                && var.kind(db) != VariableKind::InOut
        });
        for var in contained {
            let typ = var.spec(db).infer(db);
            let callsite = CallSite::from_scoped(db, &var.spec(db));
            Self::extract_pou_from_type(db, pou, typ, deps, callsites, callsite);
        }

        // check inheritance
        for base in crate::hir_ty::oop::written_bases(db, pou) {
            if let Some(target) = base.target {
                let typ = Type::new_pou(db, target);
                let callsite = CallSite::from_scoped(db, &base.spec);
                Self::extract_pou_from_type(db, pou, typ, deps, callsites, callsite);
            }
        }

        if let Pou::DataType(dt) = pou {
            let typ = dt.spec(db).infer(db);
            let callsite = CallSite::from_scoped(db, &dt.spec(db));
            Self::extract_pou_from_type(db, pou, typ, deps, callsites, callsite);
        }
    }

    fn extract_pou_from_type(
        db: &'db dyn WorkspaceDataBase,
        from: Pou<'db>,
        typ: Type<'db>,
        deps: &mut FxHashSet<Pou<'db>>,
        callsites: &mut FxHashMap<(Pou<'db>, Pou<'db>), Vec<CallSite<'db>>>,
        callsite: CallSite<'db>,
    ) {
        match typ {
            Type::Struct(s) => {
                for field in s.elements(db) {
                    let field_ty = field.spec(db).infer(db);
                    Self::extract_pou_from_type(
                        db,
                        from,
                        field_ty,
                        deps,
                        callsites,
                        CallSite::from_scoped(db, &field.spec(db)),
                    );
                }
            }
            Type::Array(a) => {
                let elem = a.of_type(db).infer(db);
                Self::extract_pou_from_type(
                    db,
                    from,
                    elem,
                    deps,
                    callsites,
                    CallSite::from_scoped(db, &a.of_type(db)),
                );
            }
            Type::DataType(dt) => {
                let to = Pou::DataType(dt);
                deps.insert(to);
                callsites.entry((from, to)).or_default().push(callsite);
            }
            Type::Function(func) => {
                let to = Pou::Function(func);
                deps.insert(to);
                callsites.entry((from, to)).or_default().push(callsite);
            }
            Type::FunctionBlock(fb) => {
                let to = Pou::FunctionBlock(fb);
                deps.insert(to);
                callsites.entry((from, to)).or_default().push(callsite);
            }
            Type::Class(class) => {
                let to = Pou::Class(class);
                deps.insert(to);
                callsites.entry((from, to)).or_default().push(callsite);
            }
            Type::Interface(interface) => {
                let to = Pou::Interface(interface);
                deps.insert(to);
                callsites.entry((from, to)).or_default().push(callsite);
            }
            _ => {}
        }
    }

    pub fn check_recursions(
        &mut self,
        semantic_index: &SemanticIndex<'db>,
        errors: &mut Vec<IdeDiagnostic>,
    ) {
        let mut visited = FxHashSet::default();
        let mut stack = Vec::new();
        let mut stack_set = FxHashSet::default();

        for &root in semantic_index.global_pous.iter() {
            if !visited.contains(&root) {
                self.dfs(root, root, &mut visited, &mut stack, &mut stack_set, errors);
            }
        }

        self.namespace_recursion(
            &semantic_index.namespaces,
            &mut visited,
            &mut stack,
            &mut stack_set,
            errors,
        );
    }

    fn namespace_recursion(
        &mut self,
        namespaces: &[NamespaceDecl<'db>],
        visited: &mut FxHashSet<Pou<'db>>,
        stack: &mut Vec<Pou<'db>>,
        stack_set: &mut FxHashSet<Pou<'db>>,
        errors: &mut Vec<IdeDiagnostic>,
    ) {
        for &ns in namespaces {
            self.namespace_recursion(ns.namespaces(self.db), visited, stack, stack_set, errors);

            for &pou in ns.pous(self.db) {
                if !visited.contains(&pou) {
                    self.dfs(pou, pou, visited, stack, stack_set, errors);
                }
            }
        }
    }

    fn dfs(
        &mut self,
        root: Pou<'db>, // root POU of current file
        node: Pou<'db>,
        visited: &mut FxHashSet<Pou<'db>>,
        stack: &mut Vec<Pou<'db>>,
        stack_set: &mut FxHashSet<Pou<'db>>,
        errors: &mut Vec<IdeDiagnostic>,
    ) {
        self.ensure_edges(node);

        visited.insert(node);
        stack.push(node);
        stack_set.insert(node);

        for dep in self.edges.get(&node).cloned().unwrap_or_default() {
            if dep == node {
                // direct recursion anchored to file-local root
                errors.push(
                    RecursionError::DirectRecursion {
                        pou: node,
                        callsite: self
                            .callsites
                            .get(&(node, node))
                            .and_then(|v| v.first())
                            .copied(),
                    }
                    .to_diagnostic(self.db, node.get_scope_id(self.db).file(self.db)),
                );
                continue;
            }

            if stack_set.contains(&dep) {
                let start = stack.iter().position(|p| *p == dep).unwrap();
                let cycle = &stack[start..];

                errors.push(
                    RecursionError::MutualRecursion {
                        pou: root, // always the file-local root
                        pous: cycle.to_vec(),
                        callsite: self
                            .callsites
                            .get(&(node, dep))
                            .cloned()
                            .unwrap_or_default(),
                    }
                    .to_diagnostic(self.db, root.get_scope_id(self.db).file(self.db)),
                );
                continue;
            }

            if !visited.contains(&dep) {
                self.dfs(root, dep, visited, stack, stack_set, errors);
            }
        }

        stack.pop();
        stack_set.remove(&node);
    }
}
