use std::ops::ControlFlow;

use auto_lsp::{
    core::span::Span,
    default::db::file::File,
    lsp_types::Location,
};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        interned::namespace::NamespacePath,
        semantic_index::semantic_index,
    },
    hir_ty::{
        infer::Infer,
        index_graphs::namespace_index,
        ty::{CallableType, Type},
    },
};

use crate::{hir_node::HirNode, walk::WalkHir};

pub struct ReferenceLocation {
    pub file: File,
    pub span: Span,
}

impl ReferenceLocation {
    pub fn to_location(&self, db: &dyn WorkspaceDataBase) -> Location {
        Location::new(self.file.url(db).clone(), self.span.into())
    }
}

impl<'db> HirNode<'db> {
    pub fn references(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<Vec<ReferenceLocation>> {
        // Namespace references: declarations via namespace_index + USING statements via walk
        let ns_path = match self {
            HirNode::Namespace(ns) => Some(*ns.path(db)),
            HirNode::Using(u) => Some(u.path(db).path),
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
            if !source
                .to_ascii_lowercase()
                .contains(&name.to_ascii_lowercase())
            {
                continue;
            }

            find_references_in_file(db, *file, &target, &mut locations);
        }

        // Deduplicate — the walk can visit overlapping nodes
        locations.dedup_by(|a, b| {
            a.file.url(db) == b.file.url(db) && a.span == b.span
        });

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
        HirNode::PathExpr { curr, .. } => curr.infer(db),
        HirNode::VariableAccess(v) => v.infer(db),
        HirNode::Expr(e) => e.infer(db),
        HirNode::Param(p) => p.infer(db),
        HirNode::InitExpr { curr, .. } => curr.infer(db),
        HirNode::NamespaceAccess(ns) => ns.infer(db),
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
        HirNode::PathExpr { curr, .. } => curr.infer(db),
        HirNode::VariableAccess(v) => v.infer(db),
        HirNode::Param(p) => p.infer(db),
        HirNode::NamespaceAccess(ns) => ns.infer(db),
        // Skip Expr, Invocation, InitExpr — they wrap inner nodes
        // and would produce duplicate matches
        _ => return None,
    };
    normalize_reference_type(ty)
}

/// Get the span for a reference result.
/// For declarations, returns just the name span; for references, returns the node span.
fn reference_span<'db>(db: &'db dyn WorkspaceDataBase, node: &HirNode<'db>) -> auto_lsp::core::span::Span {
    match node {
        HirNode::PouDecl(pou) => pou.get_name_span(db),
        HirNode::VariableDecl(var) => var.get_name_span(db),
        HirNode::MethodRef(m) => m.get_name_span(db),
        HirNode::StructElement(st) => st.get_name_span(db),
        _ => node.get_span(db),
    }
}

fn find_references_in_file<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    target: &Type<'db>,
    locations: &mut Vec<ReferenceLocation>,
) {
    let sema = semantic_index(db, file);

    let _ = sema.walk_hir(db, &mut |node: HirNode<'db>| {
        if let Some(resolved) = resolve_walk_target(db, &node) {
            if resolved == *target {
                let span = reference_span(db, &node);
                let file = node.get_scope_id(db).file(db);
                locations.push(ReferenceLocation { file, span });
            }
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
            if let HirNode::Using(u) = &node {
                if u.path(db).path == path {
                    locations.push(ReferenceLocation {
                        file: u.scope_id(db).file(db),
                        span: u.get_span(db),
                    });
                }
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
