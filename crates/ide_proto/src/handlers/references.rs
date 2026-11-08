// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::ops::ControlFlow;

use auto_lsp::{default::db::file::File, lsp_types::Location, tree_sitter};
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{InitExpr, InitExprKind, ParamAssign, ParamAssignKind, PathExpr},
            spec::{Spec, SpecKind},
        },
        hir_node::HirNode,
        interned::{
            identifier::SpanIdent,
            namespace::{NamespacePath, SpanNamespacePath},
        },
        semantic_index::semantic_index,
    },
    hir_ty::{
        index_graphs::absolute_namespace_path,
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
        // A namespace is named by its declarations, its USINGs and the names
        // qualified with it.
        let ns_path = match self {
            HirNode::Namespace(ns) => Some(ns.path(db)),
            HirNode::Using(u) => Some(absolute_namespace_path(
                db,
                u.scope_id(db),
                u.path(db).path(db),
            )),
            HirNode::PathExpr(p) => namespace_of_step(db, *p),
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
        // The return value is referred to by its callable's name: renaming the
        // FUNCTION renames `F := ...` in its body too.
        Type::CallableType(ct) | Type::ReturnValue((ct, _)) => match ct {
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
    if let Some(var) = p.scope_id(db).inference(db).variable_for_path_expr(p) {
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
        // A field it names: its parentheses and brackets name nothing, and
        // a rename from one renamed the type.
        HirNode::InitExpr(i) => init_field_name(db, *i).map(|_| i.infer(db))?,
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
        // A binding names its parameter, `in := x` or `out => y`; a
        // positional argument names nothing, and taking it for the
        // parameter it reaches wrote the new name over the argument: `f(v)`
        // and `f(1.5)` came back as `f(renamed)`.
        HirNode::Param(p) => param_name(db, *p).map(|_| p.infer(db))?,
        // A field named in an initializer, `fuel` of `(fuel := 1)`. The other
        // initializer nodes hold their values, and name nothing themselves.
        HirNode::InitExpr(i) => init_field_name(db, *i).map(|_| i.infer(db))?,
        // Skip Expr, Invocation, VariableAccess — they wrap inner nodes
        // and would produce duplicate matches (PathExpr already covers variable accesses)
        _ => return None,
    };
    normalize_reference_type(ty)
}

/// The parameter a binding names, `in` of `in := x` or `out => y`; a
/// positional argument has none.
fn param_name<'db>(db: &'db dyn WorkspaceDataBase, p: ParamAssign<'db>) -> Option<SpanIdent<'db>> {
    match p.kind(db) {
        ParamAssignKind::FormalInput { param, .. }
        | ParamAssignKind::FormalOutput { param, .. } => Some(param),
        ParamAssignKind::NonFormal { .. } => None,
    }
}

/// The field or member an initializer names, `fuel` of `(fuel := 1)`.
fn init_field_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    i: InitExpr<'db>,
) -> Option<SpanIdent<'db>> {
    match i.kind(db) {
        InitExprKind::StructElement { name, .. } => Some(name),
        _ => None,
    }
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
        HirNode::PathExpr(p) => p.ident(db).ident(db).text(db),
        HirNode::Param(p) => param_name(db, *p)?.ident(db).text(db),
        HirNode::InitExpr(i) => init_field_name(db, *i)?.ident(db).text(db),
        HirNode::Spec(spec) => spec_target(db, *spec)?.ident(db).text(db),
        _ => return None,
    })
}

/// The name a type specification names, `T` of `x : Lib.T`, when it names
/// one.
fn spec_target<'db>(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Option<SpanIdent<'db>> {
    match spec.kind(db) {
        SpecKind::Target(access) => Some(access.path.target),
        _ => None,
    }
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
        // The name alone: the whole binding, `IN := 5`, used to be replaced
        // by the new name, value included.
        HirNode::Param(p) => match param_name(db, *p) {
            Some(name) => name.get_span(db),
            None => node.get_span(db),
        },
        // The name alone: the whole element, `fuel := 1`, is not the field's.
        HirNode::InitExpr(i) => match init_field_name(db, *i) {
            Some(name) => name.get_span(db),
            None => node.get_span(db),
        },
        // The name alone, `T` of `Lib.T`: the whole path was replaced, and
        // the namespace it was qualified with went with it.
        HirNode::Spec(spec) => match spec_target(db, *spec) {
            Some(name) => name.get_span(db),
            None => node.get_span(db),
        },
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

/// The namespace a path step names: `Lib` and `Lib.Sub` of `Lib.Sub.G()`.
fn namespace_of_step<'db>(
    db: &'db dyn WorkspaceDataBase,
    p: PathExpr<'db>,
) -> Option<NamespacePath> {
    let scope = p.scope_id(db);
    if !scope.inference(db).path_expr_is_namespace(p) {
        return None;
    }
    let (access, _) = p.to_namespace_access(db).as_ref()?;
    let mut fragments = match &access.namespace {
        Some(namespace) => namespace.path(db).fragments(db).clone(),
        None => vec![],
    };
    fragments.push(access.target.ident(db));
    Some(absolute_namespace_path(
        db,
        scope,
        NamespacePath::new(db, fragments),
    ))
}

/// The namespace named by the segment of `x : Lib.T` the cursor is on, when it
/// is on one: past them, the cursor is on the type.
pub(crate) fn namespace_at<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: Spec<'db>,
    offset: usize,
) -> Option<NamespacePath> {
    let SpecKind::Target(access) = spec.kind(db) else {
        return None;
    };
    let written = access.path.namespace.as_ref()?;
    let fragments = written.path(db).fragments(db);
    (0..fragments.len()).find_map(|index| {
        let span = written.fragment_span(db, index);
        (span.start_byte <= offset && offset <= span.end_byte).then(|| {
            absolute_namespace_path(
                db,
                written.scope_id,
                NamespacePath::new(db, fragments[..=index].to_vec()),
            )
        })
    })
}

/// Each segment of a written path that names `path`: `App` and `Motors` of
/// `USING App.Motors` each name a namespace, relative to where it is written.
fn segments_naming<'db>(
    db: &'db dyn WorkspaceDataBase,
    written: &SpanNamespacePath<'db>,
    path: NamespacePath,
    locations: &mut Vec<ReferenceLocation>,
) {
    let fragments = written.path(db).fragments(db);
    for index in 0..fragments.len() {
        let prefix = NamespacePath::new(db, fragments[..=index].to_vec());
        if absolute_namespace_path(db, written.scope_id, prefix) == path {
            locations.push(ReferenceLocation {
                file: written.scope_id.file(db),
                span: written.fragment_span(db, index),
            });
        }
    }
}

/// Where `path` is written: what declares it, or a namespace in it with a dotted
/// name (`App` of `NAMESPACE App.Motors`), the USINGs, and the types and values
/// qualified with it.
pub(crate) fn find_namespace_references<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
) -> Option<Vec<ReferenceLocation>> {
    let mut locations = vec![];
    let target = path.fragments(db);

    for file in db.get_files().iter() {
        let sema = semantic_index(db, *file);

        // The written name ends the declaration's path, and what comes before
        // is the enclosing namespaces: `Motors` nested in `NAMESPACE App` writes
        // one segment of two, and does not name `App`.
        for ns in sema.namespaces.iter() {
            let declared = ns.path(db);
            let declared = declared.fragments(db);
            let span = ns.name_span(db);
            let written = file
                .document(db)
                .as_str()
                .get(span.start_byte..span.end_byte)
                .map_or(1, |text| text.split('.').count());
            let enclosing = declared.len().saturating_sub(written);
            if target.len() > enclosing
                && target.len() <= declared.len()
                && declared[..target.len()] == target[..]
            {
                locations.push(ReferenceLocation {
                    file: *file,
                    span: nth_segment(db, *file, span, target.len() - enclosing - 1),
                });
            }
        }

        let _ = sema.walk_hir(db, &mut |node: HirNode<'db>| {
            match &node {
                HirNode::Using(u) => segments_naming(db, &u.path(db), path, &mut locations),
                HirNode::Spec(spec) => {
                    if let SpecKind::Target(access) = spec.kind(db)
                        && let Some(written) = &access.path.namespace
                    {
                        segments_naming(db, written, path, &mut locations);
                    }
                }
                HirNode::PathExpr(p) if namespace_of_step(db, *p) == Some(path) => {
                    locations.push(ReferenceLocation {
                        file: p.scope_id(db).file(db),
                        span: p.ident(db).get_span(db),
                    });
                }
                _ => {}
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

/// The segment at `index` of the dotted name at `span`: `App` of `App.Motors` at
/// 0. A rename edit replaces one identifier.
fn nth_segment(
    db: &dyn WorkspaceDataBase,
    file: File,
    span: tree_sitter::Range,
    index: usize,
) -> tree_sitter::Range {
    let source = file.document(db).as_str();
    let Some(text) = source.get(span.start_byte..span.end_byte) else {
        return span;
    };
    let mut start = 0;
    for (at, part) in text.split('.').enumerate() {
        if at == index {
            let begin = start + (part.len() - part.trim_start().len());
            let end = begin + part.trim().len();
            let point_at = |bytes: &str| {
                let mut point = span.start_point;
                for byte in bytes.bytes() {
                    if byte == b'\n' {
                        point.row += 1;
                        point.column = 0;
                    } else {
                        point.column += 1;
                    }
                }
                point
            };
            return tree_sitter::Range {
                start_byte: span.start_byte + begin,
                end_byte: span.start_byte + end,
                start_point: point_at(&text[..begin]),
                end_point: point_at(&text[..end]),
            };
        }
        start += part.len() + 1;
    }
    span
}

/// The last segment of the dotted name at `span`: `Motors` of `App.Motors`,
/// the whole of a name with one segment. A rename edit replaces one
/// identifier; replacing the whole name wrote the new one over the parents
/// too, `NAMESPACE App.Motors` becoming `NAMESPACE Drives`.
pub(crate) fn last_segment(
    db: &dyn WorkspaceDataBase,
    file: File,
    span: tree_sitter::Range,
) -> tree_sitter::Range {
    let source = file.document(db).as_str();
    let Some(text) = source.get(span.start_byte..span.end_byte) else {
        return span;
    };
    let after_dot = text.rfind('.').map_or(0, |i| i + 1);
    let skipped = text[after_dot..].len() - text[after_dot..].trim_start().len();
    let start = after_dot + skipped;
    if start == 0 {
        return span;
    }
    // The point moves over what was skipped, line by line.
    let mut point = span.start_point;
    for byte in text[..start].bytes() {
        if byte == b'\n' {
            point.row += 1;
            point.column = 0;
        } else {
            point.column += 1;
        }
    }
    tree_sitter::Range {
        start_byte: span.start_byte + start,
        start_point: point,
        ..span
    }
}
