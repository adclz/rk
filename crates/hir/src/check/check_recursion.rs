use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    CallSite, HirNodeInfo,
    check::errors::e9_recursion::RecursionError,
    hir_def::{namespace::NamespaceDecl, pous::pou::Pou, semantic_index::SemanticIndex},
    hir_ty::ty::Type,
};

use db::WorkspaceDataBase;

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
        for &pou in &semantic_index.global_pous {
            edges.entry(pou).or_insert_with(FxHashSet::default);
        }

        Self::namespace_edges(db, &mut edges, &semantic_index.global_namespaces);

        Self {
            db,
            edges,
            callsites,
        }
    }

    fn namespace_edges(
        db: &'db dyn WorkspaceDataBase,
        edges: &mut FxHashMap<Pou<'db>, FxHashSet<Pou<'db>>>,
        namespaces: &[NamespaceDecl<'db>],
    ) {
        for &ns in namespaces {
            Self::namespace_edges(db, edges, ns.namespaces(db));

            for &pou in ns.pous(db).iter() {
                edges.entry(pou).or_default();
            }
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

        // check variables
        for var in def_map.global_variables.values() {
            let typ = Type::new_spec(db, var.spec(db));
            let callsite = CallSite::from_spec(db, var.spec(db));
            Self::extract_pou_from_type(db, pou, typ, deps, callsites, callsite);
        }

        // check inheritance
        for (callsite, p) in pou.get_scope_id(db).inheritors(db) {
            let typ = Type::new_pou(db, *p);
            Self::extract_pou_from_type(db, pou, typ, deps, callsites, *callsite);
        }

        if let Pou::DataType(dt) = pou {
            let typ = Type::new_spec(db, dt.spec(db));
            let callsite = CallSite::from_spec(db, dt.spec(db));
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
                    let field_ty = Type::new_spec(db, field.spec(db));
                    Self::extract_pou_from_type(
                        db,
                        from,
                        field_ty,
                        deps,
                        callsites,
                        CallSite::from_struct_element(db, field),
                    );
                }
            }
            Type::Array(a) => {
                let elem = Type::new_spec(db, a.of_type(db));
                Self::extract_pou_from_type(
                    db,
                    from,
                    elem,
                    deps,
                    callsites,
                    CallSite::from_spec(db, a.of_type(db)),
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

    pub fn find_recursion(
        &mut self,
        semantic_index: &SemanticIndex<'db>,
    ) -> Vec<RecursionError<'db>> {
        let mut errors = Vec::new();
        let mut visited = FxHashSet::default();
        let mut stack = Vec::new();
        let mut stack_set = FxHashSet::default();

        for &root in &semantic_index.global_pous {
            if !visited.contains(&root) {
                self.dfs(
                    root,
                    root,
                    &mut visited,
                    &mut stack,
                    &mut stack_set,
                    &mut errors,
                );
            }
        }

        self.namespace_recursion(
            &semantic_index.global_namespaces,
            &mut visited,
            &mut stack,
            &mut stack_set,
            &mut errors,
        );

        errors
    }

    fn namespace_recursion(
        &mut self,
        namespaces: &[NamespaceDecl<'db>],
        visited: &mut FxHashSet<Pou<'db>>,
        stack: &mut Vec<Pou<'db>>,
        stack_set: &mut FxHashSet<Pou<'db>>,
        errors: &mut Vec<RecursionError<'db>>,
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
        errors: &mut Vec<RecursionError<'db>>,
    ) {
        self.ensure_edges(node);

        visited.insert(node);
        stack.push(node);
        stack_set.insert(node);

        for dep in self.edges.get(&node).cloned().unwrap_or_default() {
            if dep == node {
                // direct recursion anchored to file-local root
                errors.push(RecursionError::DirectRecursion {
                    pou: node,
                    callsite: self
                        .callsites
                        .get(&(node, node))
                        .and_then(|v| v.first())
                        .copied(),
                });
                continue;
            }

            if stack_set.contains(&dep) {
                let start = stack.iter().position(|p| *p == dep).unwrap();
                let cycle = &stack[start..];

                errors.push(RecursionError::MutualRecursion {
                    pou: root, // always the file-local root
                    pous: cycle.to_vec(),
                    callsite: self
                        .callsites
                        .get(&(node, dep))
                        .cloned()
                        .unwrap_or_default(),
                });
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
