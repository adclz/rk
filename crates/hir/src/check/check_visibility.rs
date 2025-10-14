use auto_lsp::default::db::BaseDatabase;

use crate::{
    HirNodeInfo,
    builder::invocation,
    check::errors::{
        analysis_error::AnalysisError, inheritance::MethodError, visibility::VisibilityError,
    },
    hir_def::{
        expressions::invocation::Invocation,
        namespace::NamespaceDecl,
        pous::{class::MethodDecl, pou::Pou},
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
        visibility::Visibility,
    },
    hir_ty::{
        invocation_resolver::ResolvedInvocation,
        ty::{Ty, TyKind, ty_for_pou},
    },
};

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
pub fn check_call_visibility<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    call_site: &'db impl HirNodeInfo<'db>,
    errors: &mut Vec<AnalysisError<'db>>,
) {
    let calling_scope = call_site.get_scope_id(db);
    let method_visibility = match ty.visibility(db) {
        Some(vis) => vis,
        None => return,
    };
    let method_scope = ty.get_scope_id(db);

    // PUBLIC methods can be called from anywhere
    if method_visibility.contains(Visibility::PUBLIC) {
        return;
    }

    // Check PRIVATE visibility - only callable from the same POU (same scope)
    if method_visibility.contains(Visibility::PRIVATE) {
        if calling_scope != method_scope {
            errors.push(
                VisibilityError::PrivateMethod {
                    method: ty,
                    call_site: call_site.get_span(db),
                }
                .into(),
            );
        }
        return;
    }

    // Check INTERNAL visibility - only callable from the same namespace
    if method_visibility.contains(Visibility::INTERNAL) {
        let result = is_same_namespace(db, calling_scope, method_scope);
        match result {
            SameNamespaceResult::Same => {} // Ok
            _ => {
                errors.push(
                    VisibilityError::InternalMethod {
                        method: ty,
                        result,
                        call_site: call_site.get_span(db),
                    }
                    .into(),
                );
            }
        }
        return;
    }

    // Check PROTECTED visibility (default) - callable from same POU or derived POUs
    if method_visibility.contains(Visibility::PROTECTED) || method_visibility.is_empty() {
        if !is_derived_pou(db, calling_scope, method_scope) {
            errors.push(
                VisibilityError::ProtectedMethod {
                    method: ty,
                    call_site: call_site.get_span(db),
                }
                .into(),
            );
        }
        return;
    }
}

/// Check if the calling scope is in a POU that derives from the method's POU
fn is_derived_pou<'db>(
    db: &'db dyn BaseDatabase,
    calling_scope: FileScopeId<'db>,
    method_scope: FileScopeId<'db>,
) -> bool {
    let sema_calling_scope = semantic_index(db, calling_scope.file(db));
    let sema_method_scope = semantic_index(db, method_scope.file(db));

    let sema_calling_scope = sema_calling_scope.get_scope(db, calling_scope);
    let sema_method_scope = sema_method_scope.get_scope(db, method_scope);

    match (sema_calling_scope.kind, sema_method_scope.kind) {
        (ScopeKind::Pou(child), ScopeKind::Pou(parent)) => {
            // In case of THIS
            if child == parent {
                return true;
            }
            // In case of SUPER
            ty_for_pou(db, child)
                .inheritors(db)
                .iter()
                .any(|inheritor| {
                    *inheritor == ty_for_pou(db, parent)
                })
        }
        _ => return false, // One or both are not POUs
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
    scope1: FileScopeId<'db>,
    scope2: FileScopeId<'db>,
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
    scope: FileScopeId<'db>,
) -> Option<crate::hir_def::namespace::NamespaceDecl<'db>> {
    let sema = semantic_index(db, scope.file(db));

    for scope_info in sema.scope_iterator(db, scope) {
        if let ScopeKind::Namespace(ns) = scope_info.kind {
            return Some(ns);
        }
    }

    None
}
