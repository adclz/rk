use std::ops::ControlFlow;

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::Location;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::semantic_index::semantic_index,
    hir_ty::{
        infer::Infer,
        ty::{CallableType, Type},
    },
};

use crate::{hir_node::HirNode, walk::WalkHir};

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

/// Whether a HirNode represents a declaration (as opposed to a reference/use)
fn is_declaration_node(node: &HirNode) -> bool {
    matches!(
        node,
        HirNode::PouDecl(_)
            | HirNode::VariableDecl(_)
            | HirNode::MethodRef(_)
            | HirNode::StructElement(_)
    )
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

impl<'db> HirNode<'db> {
    pub fn references(
        &self,
        db: &'db dyn WorkspaceDataBase,
        include_declaration: bool,
    ) -> Option<Vec<Location>> {
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

            find_references_in_file(db, *file, &target, include_declaration, &mut locations);
        }

        // Deduplicate by (url, range) — the walk can visit overlapping nodes
        locations.dedup_by(|a, b| a.uri == b.uri && a.range == b.range);

        if locations.is_empty() {
            None
        } else {
            Some(locations)
        }
    }
}

fn find_references_in_file<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    target: &Type<'db>,
    include_declaration: bool,
    locations: &mut Vec<Location>,
) {
    let sema = semantic_index(db, file);

    let _ = sema.walk_hir(db, &mut |node: HirNode<'db>| {
        if is_declaration_node(&node) && !include_declaration {
            return ControlFlow::Continue(());
        }

        if let Some(resolved) = resolve_walk_target(db, &node) {
            if resolved == *target {
                let span = reference_span(db, &node);
                let url = node.get_scope_id(db).file(db).url(db).clone();
                locations.push(Location::new(url, span.into()));
            }
        }

        ControlFlow::Continue(())
    });
}
