use std::ops::ControlFlow;

use auto_lsp::{default::db::file::File, lsp_types::Location, tree_sitter};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::expression::PathExpr, hir_node::HirNode, interned::namespace::NamespacePath,
        semantic_index::semantic_index,
    },
    hir_ty::{
        body::infer_body,
        index_graphs::{absolute_namespace_path, namespace_index},
        infer::Infer,
        ty::{CallableType, Type},
    },
};

use crate::{handlers::ReferencesHandler, walk::WalkHir};

pub struct ReferenceLocation {
    pub file: File,
    pub span: tree_sitter::Range,
}

impl ReferenceLocation {
    pub fn to_location(&self, db: &dyn WorkspaceDataBase) -> Location {
        Location::new(
            self.file.url(db).clone(),
            hir::denormalize(db, self.file, &self.span).unwrap_or_default(),
        )
    }
}

impl<'db> ReferencesHandler<'db> for HirNode<'db> {
    fn references(&'db self, db: &'db dyn WorkspaceDataBase) -> Option<Vec<Location>> {
        self.locations(db)
            .map(|locs| locs.into_iter().map(|loc| loc.to_location(db)).collect())
    }

    fn locations(&self, db: &'db dyn WorkspaceDataBase) -> Option<Vec<ReferenceLocation>> {
        // Namespace references: declarations via namespace_index + USING statements via walk
        let ns_path = match self {
            HirNode::Namespace(ns) => Some(*ns.path(db)),
            HirNode::Using(u) => Some(absolute_namespace_path(db, u.scope_id(db), u.path(db).path)),
            _ => None,
        };
        if let Some(path) = ns_path {
            return find_namespace_references(db, path);
        }

        let target = resolve_cursor_target(db, self)?;
        let name = reference_type_name(db, &target)?;

        let mut locations = vec![];

        for file in db.get_files().iter() {
            // Text pre-filter: skip files that don't contain the symbol name
            let source = file.document(db).as_str();
            // Unicode, matching the fold resolution uses: an ASCII fold here
            // would skip a file whose only mention of `MÄX` is spelled `mäx`.
            if !source.to_lowercase().contains(&name.to_lowercase()) {
                continue;
            }

            find_references_in_file(db, *file, &target, name, &mut locations);
        }

        // Deduplicate — the walk can visit overlapping nodes
        locations.dedup_by(|a, b| a.file.url(db) == b.file.url(db) && a.span == b.span);

        if locations.is_empty() {
            None
        } else {
            Some(locations)
        }
    }
}

/// Get the name text of a reference target for text pre-filtering
fn reference_type_name<'db>(db: &'db dyn WorkspaceDataBase, ty: &Type<'db>) -> Option<&'db str> {
    Some(match ty {
        Type::Function(f) => f.get_name_ident(db).text(db),
        Type::FunctionBlock(fb) => fb.get_name_ident(db).text(db),
        Type::Class(c) => c.get_name_ident(db).text(db),
        Type::Interface(i) => i.get_name_ident(db).text(db),
        Type::DataType(dt) => dt.get_name_ident(db).text(db),
        Type::Variable((var, _)) => var.get_name_ident(db).text(db),
        Type::StructElement(st) => st.get_name_ident(db).text(db),
        Type::MethodDecl(m) => m.get_name_ident(db).text(db),
        _ => return None,
    })
}

/// Normalize a Type to its canonical reference identity.
/// Strips MultibitsPart from variables and unwraps CallableType.
fn normalize_reference_type<'db>(ty: Type<'db>) -> Option<Type<'db>> {
    Some(match ty {
        Type::Function(_)
        | Type::FunctionBlock(_)
        | Type::Class(_)
        | Type::Interface(_)
        | Type::DataType(_)
        | Type::StructElement(_)
        | Type::MethodDecl(_) => ty,
        Type::Variable((var, _)) => Type::Variable((var, None)),
        Type::CallableType(ct) => match ct {
            CallableType::Function(f) => Type::Function(f),
            CallableType::FunctionBlock(fb) => Type::FunctionBlock(fb),
            CallableType::MethodDecl(m) => Type::MethodDecl(m),
        },
        _ => return None,
    })
}

/// What a path refers to. A bare name that denotes a variable is that
/// variable wherever it stands: in callee position (`motor()`) the path is
/// inferred as the block it invokes, and taking that for the reference made
/// a rename of the instance rename the block, everywhere.
fn path_reference_target<'db>(db: &'db dyn WorkspaceDataBase, p: PathExpr<'db>) -> Type<'db> {
    if let Some(var) = infer_body(db, p.scope_id(db)).variable_for_path_expr(p) {
        return Type::Variable((var, None));
    }
    p.infer(db)
}

/// Extract a normalized reference target type from a HirNode at cursor position.
/// Accepts all node types since `descendant_at` returns the deepest node.
fn resolve_cursor_target<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> Option<Type<'db>> {
    let ty = match node {
        // Declarations
        HirNode::PouDecl(pou) => Type::new_pou(db, *pou),
        HirNode::VariableDecl(var) => Type::Variable((*var, None)),
        HirNode::MethodRef(m) => Type::MethodDecl(*m),
        HirNode::StructElement(st) => Type::StructElement(*st),
        // References
        HirNode::Spec(spec) => spec.infer(db),
        HirNode::PathExpr(p) => path_reference_target(db, *p),
        HirNode::VariableAccess(v) => v.infer(db),
        HirNode::Expr(e) => e.infer(db),
        HirNode::Param(p) => p.infer(db),
        HirNode::InitExpr(i) => i.infer(db),
        HirNode::Invocation(i) => i.infer(db),
        _ => return None,
    };
    normalize_reference_type(ty)
}

/// Resolve a HirNode to its reference target during a walk.
/// Only checks leaf-level reference nodes to avoid duplicates from wrapper nodes
/// (e.g. Expr wrapping VariableAccess, Invocation wrapping PathExpr).
fn resolve_walk_target<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> Option<Type<'db>> {
    let ty = match node {
        // Declarations
        HirNode::PouDecl(pou) => Type::new_pou(db, *pou),
        HirNode::VariableDecl(var) => Type::Variable((*var, None)),
        HirNode::MethodRef(m) => Type::MethodDecl(*m),
        HirNode::StructElement(st) => Type::StructElement(*st),
        // Leaf-level reference nodes only
        HirNode::Spec(spec) => spec.infer(db),
        HirNode::PathExpr(p) => path_reference_target(db, *p),
        HirNode::Param(p) => p.infer(db),
        // Skip Expr, Invocation, InitExpr, VariableAccess — they wrap inner nodes
        // and would produce duplicate matches (PathExpr already covers variable accesses)
        _ => return None,
    };
    normalize_reference_type(ty)
}

/// Extract the identifier text from a HirNode for reference matching.
/// Returns None for nodes where ident comparison is not applicable.
fn node_reference_ident<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> Option<&'db str> {
    Some(match node {
        HirNode::PouDecl(pou) => pou.get_name_ident(db).text(db),
        HirNode::VariableDecl(var) => var.get_name_ident(db).text(db),
        HirNode::MethodRef(m) => m.get_name_ident(db).text(db),
        HirNode::StructElement(st) => st.get_name_ident(db).text(db),
        HirNode::PathExpr(p) => p.ident(db).text(db),
        _ => return None,
    })
}

/// Get the span for a reference result.
/// For declarations, returns just the name span; for path expressions, returns
/// just the ident span (e.g. `fuel` in `my_var.fuel`); otherwise the full node span.
fn reference_span<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> auto_lsp::tree_sitter::Range {
    match node {
        HirNode::PouDecl(pou) => pou.get_name_span(db),
        HirNode::VariableDecl(var) => var.get_name_span(db),
        HirNode::MethodRef(m) => m.get_name_span(db),
        HirNode::StructElement(st) => st.get_name_span(db),
        HirNode::PathExpr(p) => p.ident(db).get_span(db),
        _ => node.get_span(db),
    }
}

fn find_references_in_file<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    target: &Type<'db>,
    target_name: &str,
    locations: &mut Vec<ReferenceLocation>,
) {
    let sema = semantic_index(db, file);

    let _ = sema.walk_hir(db, &mut |node: HirNode<'db>| {
        if let Some(resolved) = resolve_walk_target(db, &node)
            && resolved == *target
        {
            // Verify the node's ident matches the target name to avoid
            // false positives from path fragments that resolve to the
            // same type through adjustments (deref, field chains, etc.)
            if let Some(ident) = node_reference_ident(db, &node)
                && ident.to_lowercase() != target_name.to_lowercase()
            {
                return ControlFlow::Continue(());
            }
            let span = reference_span(db, &node);
            let file = node.get_scope_id(db).file(db);
            locations.push(ReferenceLocation { file, span });
        }

        ControlFlow::Continue(())
    });
}

fn find_namespace_references<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
) -> Option<Vec<ReferenceLocation>> {
    let mut locations = vec![];

    // All namespace declarations with this path
    for ns in namespace_index(db, path).iter() {
        locations.push(ReferenceLocation {
            file: ns.scope_id(db).file(db),
            span: ns.name_span(db),
        });
    }

    // All USING statements with this path
    for file in db.get_files().iter() {
        let sema = semantic_index(db, *file);
        let _ = sema.walk_hir(db, &mut |node: HirNode<'db>| {
            // A USING's path is relative to where it is written.
            if let HirNode::Using(u) = &node
                && absolute_namespace_path(db, u.scope_id(db), u.path(db).path).caseless(db)
                    == path.caseless(db)
            {
                locations.push(ReferenceLocation {
                    file: u.scope_id(db).file(db),
                    span: u.get_span(db),
                });
            }
            ControlFlow::Continue(())
        });
    }

    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}
