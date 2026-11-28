// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;
use std::ops::ControlFlow;

use auto_lsp::lsp_types::{TextEdit, WorkspaceEdit};
use db::WorkspaceDataBase;
use hir::{
    HasName, HasVisibility, HirNodeInfo, Visibility,
    hir_def::{
        hir_node::HirNode,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::ScopeKind,
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        index_graphs::external_var_lookup,
        oop::{MethodRef, ancestry, descendants},
        ty::Type,
    },
};

use crate::handlers::references::ReferenceLocation;
use crate::handlers::{ReferencesHandler, RenameHandler};
use crate::walk::WalkHir;

impl<'db> RenameHandler<'db> for HirNode<'db> {
    fn rename(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        // A library file is not the workspace's to edit, and what matters is
        // where the name is DECLARED, not where this use of it sits. Renaming
        // rewrote every use and left the declaration behind, so the next
        // check reported E0203 on code the reader had not touched.
        if declared_in_a_library(self, db) {
            return None;
        }

        // A namespace is renamed by its last segment: on `App` of `NAMESPACE
        // App.Motors` the cursor is on the parent, which is renamed where
        // it is declared, and nothing is edited from here.
        let dotted = match self {
            HirNode::Namespace(ns) => Some((ns.get_scope_id(db).file(db), ns.name_span(db))),
            HirNode::Using(u) => Some((u.get_scope_id(db).file(db), u.get_span(db))),
            _ => None,
        };
        if let Some((file, span)) = dotted
            && offset < super::references::last_segment(db, file, span).start_byte
        {
            return None;
        }

        // On `Lib` of `x : Lib.T` the cursor is on the namespace, not the type.
        let namespace = match self {
            HirNode::Spec(spec) => super::references::namespace_at(db, *spec, offset),
            _ => None,
        };
        let locations = match namespace {
            Some(path) => super::references::find_namespace_references(db, path)?,
            None => match renamed_together(db, self) {
                Some(locations) if !locations.is_empty() => locations,
                Some(_) => return None,
                None => self.locations(db)?,
            },
        };

        let mut changes: HashMap<_, Vec<TextEdit>> = HashMap::new();
        for loc in locations {
            // A namespace can be spread over files, and a library's are not the
            // workspace's to edit.
            if db.get_library_file(loc.file.url(db)).is_some() {
                return None;
            }
            // An edit replaces a name and nothing else. A located variable
            // declared without one, `AT %QB4 : INT`, has its address where
            // a name would be, and a declaration whose name the parser had
            // to make up has nothing there at all.
            if !is_identifier(loc.file.document(db).as_str(), &loc.span) {
                continue;
            }
            changes
                .entry(loc.file.url(db).clone())
                .or_default()
                .push(TextEdit {
                    range: hir::denormalize(db, loc.file, &loc.span).unwrap_or_default(),
                    new_text: new_name.to_string(),
                });
        }

        if changes.is_empty() {
            return None;
        }
        Some(WorkspaceEdit::new(changes))
    }
}

/// Where a rename from `node` edits when the cursor names a method, or an
/// input, output or in-out of one: the method shares its name with every
/// method it overrides or implements and that overrides or implements it,
/// and its parameters their names with theirs (E1128). Renaming one of them
/// alone broke the others: E1113, E1116, E1119. A VAR_GLOBAL and the
/// VAR_EXTERNALs that import it share theirs too (E0206). `None` for
/// anything else.
fn renamed_together<'db>(
    db: &'db dyn WorkspaceDataBase,
    node: &HirNode<'db>,
) -> Option<Vec<ReferenceLocation>> {
    let targets: Vec<Type<'db>> = match super::references::resolve_cursor_target(db, node)? {
        Type::MethodDecl(method) => method_family(db, method)
            .into_iter()
            .map(Type::MethodDecl)
            .collect(),
        Type::Variable((variable, _))
            if matches!(
                variable.kind(db),
                VariableKind::Global | VariableKind::External
            ) =>
        {
            global_and_externals(db, variable)
        }
        Type::Variable((variable, _)) => {
            let method = method_of_parameter(db, variable)?;
            let name = variable.get_name_ident(db);
            method_family(db, method)
                .into_iter()
                .filter_map(|member| {
                    member
                        .variables(db)
                        .iter()
                        .find(|parameter| parameter.get_name_ident(db) == name)
                        .map(|parameter| Type::Variable((*parameter, None)))
                })
                .collect()
        }
        _ => return None,
    };
    let mut locations: Vec<ReferenceLocation> = targets
        .iter()
        .flat_map(|target| super::references::locations_of(db, target))
        .collect();
    super::references::once_each(&mut locations);
    Some(locations)
}

/// The VAR_GLOBAL `variable` is or imports, and every VAR_EXTERNAL of the
/// workspace that imports it: a VAR_EXTERNAL names its global by its name.
/// A library imports none (E1008).
fn global_and_externals<'db>(
    db: &'db dyn WorkspaceDataBase,
    variable: VariableDecl<'db>,
) -> Vec<Type<'db>> {
    let name = variable.get_name_ident(db);
    let global = match variable.kind(db) {
        VariableKind::Global => Some(variable),
        _ => external_var_lookup(db, name),
    };
    let mut family: Vec<VariableDecl<'db>> = global.into_iter().chain([variable]).collect();
    for file in db.get_files().iter() {
        if db.get_library_file(file.url(db)).is_some() {
            continue;
        }
        let _ = semantic_index(db, *file).walk_hir(db, &mut |node| {
            if let HirNode::VariableDecl(declared) = node
                && declared.kind(db) == VariableKind::External
                && declared.get_name_ident(db) == name
                && !family.contains(&declared)
            {
                family.push(declared);
            }
            ControlFlow::Continue(())
        });
    }
    family
        .into_iter()
        .map(|declared| Type::Variable((declared, None)))
        .collect()
}

/// The method `variable` is an input, an output or an in-out of. A method's
/// own VARs are its own: two methods of a family keep theirs apart.
fn method_of_parameter<'db>(
    db: &'db dyn WorkspaceDataBase,
    variable: VariableDecl<'db>,
) -> Option<MethodRef<'db>> {
    if !matches!(
        variable.kind(db),
        VariableKind::Input | VariableKind::Output | VariableKind::InOut
    ) {
        return None;
    }
    match get_scope(db, variable.get_scope_id(db)).kind {
        ScopeKind::MethodDecl(method) => Some(MethodRef::Declared(method)),
        ScopeKind::MethodProt(prototype) => Some(MethodRef::Prototype(prototype)),
        _ => None,
    }
}

/// `method` and every method that shares its name with it through
/// overriding and implementing: what it overrides or implements, what
/// overrides or implements it, and so on through every base, INTERFACE and
/// derived block. A PRIVATE method neither overrides nor is overridden, and
/// is alone.
fn method_family<'db>(
    db: &'db dyn WorkspaceDataBase,
    method: MethodRef<'db>,
) -> Vec<MethodRef<'db>> {
    let private = |m: &MethodRef<'db>| m.get_visibility(db).contains(Visibility::PRIVATE);
    let mut family = vec![method];
    if private(&method) {
        return family;
    }
    let name = method.get_name_ident(db);
    let mut next = 0;
    while let Some(&member) = family.get(next) {
        next += 1;
        let Some(owner) = super::implementation::owner_of(db, member) else {
            continue;
        };
        let ancestry = ancestry(db, owner);
        let related = ancestry
            .chain
            .iter()
            .copied()
            .chain(
                ancestry
                    .interfaces
                    .iter()
                    .map(|iface| Pou::Interface(*iface)),
            )
            .chain(descendants(db, owner));
        for pou in related {
            if let Some(other) = pou
                .get_scope_id(db)
                .def_map(db)
                .declared_methods
                .get(&name)
                .copied()
                && !private(&other)
                && !family.contains(&other)
            {
                family.push(other);
            }
        }
    }
    family
}

/// Whether `span` of `source` holds one identifier.
fn is_identifier(source: &str, span: &auto_lsp::tree_sitter::Range) -> bool {
    source
        .get(span.start_byte..span.end_byte)
        .is_some_and(|text| {
            !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_')
        })
}

/// Whether the name this node refers to is declared in a library file.
fn declared_in_a_library<'db>(node: &'db HirNode<'db>, db: &'db dyn WorkspaceDataBase) -> bool {
    use crate::handlers::DefinitionHandler;
    use auto_lsp::lsp_types::GotoDefinitionResponse;

    let declared = match node.definition(db, node.get_span(db).start_byte) {
        Some(GotoDefinitionResponse::Scalar(location)) => location.uri,
        Some(GotoDefinitionResponse::Array(mut locations)) => match locations.pop() {
            Some(location) => location.uri,
            None => return false,
        },
        _ => return false,
    };
    db.get_library_file(&declared).is_some()
}
