
/*
Methods and specifiers

5 METHOD...END_METHOD Method definition
5a PUBLIC specifier Method may be called from anywhere
5b PRIVATE specifier Method may only be called from inside the defining POU
5c INTERNAL specifier Method may only be called from inside the same namespace
5d PROTECTED specifier Method may only be called from inside the defining POU
and its derivations (default)
5e FINAL specifier Method shall not be overridden


Variable access specifiers

11a PUBLIC specifier The variable may be accessed from anywhere.
11b PRIVATE specifier The variable may only be accessed from inside the defining POU.
11c INTERNAL specifier The variable may only be accessed from inside the same
namespace.
11d PROTECTED specifier The variable may only be accessed from inside the defining POU
and its derivations (default).
*/

use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{CallSite, HasVisibility, HirNodeInfo, Visibility, check::errors::{analysis_error::ToIdeDiagnostic, visibility::VisibilityError}, hir_def::{namespace::NamespaceDecl, scope::{ScopeId, ScopeKind}, semantic_index::{get_scope, semantic_index}}};

pub fn check_visibility<'db>(
    db: &'db dyn BaseDatabase,
    call_site: &CallSite<'db>,
    target: impl HasVisibility<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    // Methods use their declaring POU as scope for visibility checks
    let calling_scope_id = call_site.get_scope_id(db);
    let calling_scope = match get_scope(db, calling_scope_id).kind {
        ScopeKind::MethodDecl(m) => get_scope(db, calling_scope_id)
            .parent
            .expect("Method should always have a parent scope"),
        _ => calling_scope_id,
    };
    let target_scope_id = target.get_scope_id(db);
    let target_scope = match get_scope(db, target_scope_id).kind {
        ScopeKind::MethodDecl(m) => get_scope(db, target_scope_id)
            .parent
            .expect("Method should always have a parent scope"),
        _ => target_scope_id,
    };
    let target_visibility = target.get_visibility(db);

    // PUBLIC methods can be called from anywhere
    if target_visibility.contains(Visibility::PUBLIC) {
        return;
    }

    // Check PRIVATE visibility - only callable from the same POU (same scope)
    if target_visibility.contains(Visibility::PRIVATE) {
        if calling_scope != target_scope {
            errors.push(
                VisibilityError::Private {
                    call_site: call_site.clone(),
                    target: target.as_call_site(db),
                }
                .to_diagnostic(db),
            );
        }
        return;
    }

    // Check INTERNAL visibility - only callable from the same namespace
    if target_visibility.contains(Visibility::INTERNAL) {
        let result = is_same_namespace(db, calling_scope, target_scope);
        match result {
            SameNamespaceResult::Same => {} // Ok
            _ => {
                errors.push(
                    VisibilityError::Internal {
                        call_site: call_site.clone(),
                        target: target.as_call_site(db),
                        result,
                    }
                    .to_diagnostic(db),
                );
            }
        }
        return;
    }

    // Check PROTECTED visibility (default) - callable from same POU or derived POUs
    if (target_visibility.contains(Visibility::PROTECTED) || target_visibility.is_empty())
        && !is_derived_pou(db, calling_scope, target_scope)
    {
        errors.push(
            VisibilityError::Protected {
                call_site: call_site.clone(),
                target: target.as_call_site(db),
            }
            .to_diagnostic(db),
        );
    }
}

/// Check if the calling scope is in a POU that derives from the method's POU
fn is_derived_pou<'db>(
    db: &'db dyn BaseDatabase,
    calling_scope: ScopeId<'db>,
    method_scope: ScopeId<'db>,
) -> bool {
    let sema_calling_scope = semantic_index(db, calling_scope.file(db));
    let sema_method_scope = semantic_index(db, method_scope.file(db));

    let sema_calling_scope = get_scope(db, calling_scope);
    let sema_method_scope = get_scope(db, method_scope);

    match (sema_calling_scope.kind, sema_method_scope.kind) {
        (ScopeKind::Pou(child), ScopeKind::Pou(parent)) => {
            // In case of THIS
            if child == parent {
                return true;
            }
            // In case of SUPER
            child.get_scope_id(db).inheritors(db).contains(&parent)
        }
        _ => false, // One or both are not POUs
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SameNamespaceResult<'db> {
    Same,
    DifferentNamespaces((NamespaceDecl<'db>, NamespaceDecl<'db>)),
    GlobalAndNamespace(NamespaceDecl<'db>),
    NamespaceAndGlobal(NamespaceDecl<'db>),
}

/// Check if two scopes belong to the same namespace
fn is_same_namespace<'db>(
    db: &'db dyn BaseDatabase,
    scope1: ScopeId<'db>,
    scope2: ScopeId<'db>,
) -> SameNamespaceResult<'db> {
    let ns1 = find_containing_namespace(db, scope1);
    let ns2 = find_containing_namespace(db, scope2);

    match (ns1, ns2) {
        // Both scopes are in namespaces
        (Some(n1), Some(n2)) => match n1 == n2 {
            true => SameNamespaceResult::Same,
            false => SameNamespaceResult::DifferentNamespaces((n1, n2)),
        },
        // Both are in global scope
        (None, None) => SameNamespaceResult::Same,
        // Namespace <-> Global
        (Some(n1), None) => SameNamespaceResult::NamespaceAndGlobal(n1),
        // Global <-> Namespace
        (None, Some(n2)) => SameNamespaceResult::GlobalAndNamespace(n2),
    }
}

/// Find the namespace that contains the given scope using the scope iterator
fn find_containing_namespace<'db>(
    db: &'db dyn BaseDatabase,
    scope: ScopeId<'db>,
) -> Option<crate::hir_def::namespace::NamespaceDecl<'db>> {
    let sema = semantic_index(db, scope.file(db));

    for scope_info in sema.scope_iterator(db, scope) {
        if let ScopeKind::Namespace(ns) = scope_info.kind {
            return Some(ns);
        }
    }

    None
}
