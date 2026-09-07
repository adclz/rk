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

Both tables make PROTECTED the default. We do not: an item that names no
specifier is PUBLIC here. A hiding default costs an annotation on every method
that is meant to be called, and `inst.Method()` from outside the POU — what
essentially all OOP ST is written as — would otherwise be an error by default.
The specifiers themselves mean exactly what the tables say; only the unwritten
case differs.
*/

use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    CallSite, HasVisibility, HirNodeInfo, Visibility,
    check::errors::{ToIdeDiagnostic, e4_visibility::VisibilityError},
    hir_def::{
        namespace::NamespaceDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
};

/// Whether a member (a method, a class variable) may be named from
/// `calling_scope`: PUBLIC or unspecified from anywhere, PRIVATE from its
/// own POU, INTERNAL from its namespace, PROTECTED from a deriving POU. The
/// question a completion asks before offering the member; [`check_visibility`]
/// reports the answer once the name is written.
pub fn member_visible_from<'db>(
    db: &'db dyn WorkspaceDataBase,
    calling_scope: ScopeId<'db>,
    target: &impl HasVisibility<'db>,
) -> bool {
    let (calling_scope, target_scope) = declaring_pous(db, calling_scope, target.get_scope_id(db));
    let visibility = target.get_visibility(db);
    if visibility.contains(Visibility::PUBLIC) || visibility.is_empty() {
        return true;
    }
    if visibility.contains(Visibility::PRIVATE) {
        return calling_scope == target_scope;
    }
    if visibility.contains(Visibility::INTERNAL) {
        return matches!(
            is_same_namespace(db, calling_scope, target_scope),
            SameNamespaceResult::Same
        );
    }
    !visibility.contains(Visibility::PROTECTED) || is_derived_pou(db, calling_scope, target_scope)
}

/// Methods use their declaring POU as the scope visibility is judged from.
fn declaring_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    calling_scope: ScopeId<'db>,
    target_scope: ScopeId<'db>,
) -> (ScopeId<'db>, ScopeId<'db>) {
    let declaring = |scope: ScopeId<'db>| match get_scope(db, scope).kind {
        ScopeKind::MethodDecl(_) => get_scope(db, scope)
            .parent
            .expect("Method should always have a parent scope"),
        _ => scope,
    };
    (declaring(calling_scope), declaring(target_scope))
}

pub fn check_visibility<'db>(
    db: &'db dyn WorkspaceDataBase,
    call_site: &CallSite<'db>,
    target: impl HasVisibility<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let (calling_scope, target_scope) =
        declaring_pous(db, call_site.get_scope_id(db), target.get_scope_id(db));
    let target_visibility = target.get_visibility(db);

    // PUBLIC items are reachable from anywhere, and so is one that names no
    // specifier — the departure from the tables above, stated there.
    if target_visibility.contains(Visibility::PUBLIC) || target_visibility.is_empty() {
        return;
    }

    // Check PRIVATE visibility - only callable from the same POU (same scope)
    if target_visibility.contains(Visibility::PRIVATE) {
        if calling_scope != target_scope {
            errors.push(
                VisibilityError::Private {
                    call_site: *call_site,
                    target: target.as_call_site(db),
                }
                .to_diagnostic(db, call_site.get_scope_id(db).file(db)),
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
                        call_site: *call_site,
                        target: target.as_call_site(db),
                        result,
                    }
                    .to_diagnostic(db, call_site.get_scope_id(db).file(db)),
                );
            }
        }
        return;
    }

    // Check PROTECTED visibility - callable from same POU or derived POUs
    if target_visibility.contains(Visibility::PROTECTED)
        && !is_derived_pou(db, calling_scope, target_scope)
    {
        errors.push(
            VisibilityError::Protected {
                call_site: *call_site,
                target: target.as_call_site(db),
            }
            .to_diagnostic(db, call_site.get_scope_id(db).file(db)),
        );
    }
}

/// E0405: a `FUNCTION PRIVATE` is reachable from its own namespace — nested
/// namespaces included — on its own side of the library line. Namespaces
/// reopen from any file, so the namespace alone is a convention; the origin
/// half is what keeps a library's helpers out of workspace code that
/// reopens the library's namespace. Our extension: the standard gives a
/// FUNCTION no specifier at all.
pub fn check_function_visibility<'db>(
    db: &'db dyn WorkspaceDataBase,
    call_site: &CallSite<'db>,
    func: crate::hir_def::pous::function::Function<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    if function_visible_from(db, call_site.get_scope_id(db), func) {
        return;
    }
    errors.push(
        VisibilityError::PrivateFunction {
            call_site: *call_site,
            target: func.as_call_site(db),
        }
        .to_diagnostic(db, call_site.get_scope_id(db).file(db)),
    );
}

/// Whether `func` may be named from `calling_scope`: not PRIVATE, or PRIVATE
/// within its boundary. The rule [`check_function_visibility`] reports.
pub fn function_visible_from<'db>(
    db: &'db dyn WorkspaceDataBase,
    calling_scope: ScopeId<'db>,
    func: crate::hir_def::pous::function::Function<'db>,
) -> bool {
    if !func.visibility(db).contains(Visibility::PRIVATE) {
        return true;
    }
    let target_scope = func.get_scope_id(db);
    let target_ns = crate::hir_ty::resolver::name::enclosing_namespace_path(db, target_scope);
    let inside = match (
        &target_ns,
        crate::hir_ty::resolver::name::enclosing_namespace_path(db, calling_scope),
    ) {
        // Declared at global scope: the root holds everything.
        (None, _) => true,
        (Some(target), Some(caller)) => caller
            .caseless(db)
            .fragments(db)
            .starts_with(target.caseless(db).fragments(db)),
        (Some(_), None) => false,
    };
    let is_library =
        |scope: ScopeId<'db>| crate::check::check_duplicates::is_library_file(db, scope.file(db));
    let cross_origin = is_library(calling_scope) != is_library(target_scope);
    inside && !cross_origin
}

/// Whether nothing on `target_scope`'s namespace chain is an INTERNAL
/// namespace closed to `calling_scope`, and the `{test}` rule holds: the
/// question a completion or a suggestion asks before offering a POU, which
/// the checks here answer with a diagnostic once it is written.
pub fn pou_visible_from<'db>(
    db: &'db dyn WorkspaceDataBase,
    calling_scope: ScopeId<'db>,
    pou: crate::hir_def::pous::pou::Pou<'db>,
) -> bool {
    let target_scope = pou.get_scope_id(db);
    if get_scope(db, target_scope).is_test(db) && !get_scope(db, calling_scope).is_test(db) {
        return false;
    }
    if let crate::hir_def::pous::pou::Pou::Function(f) = pou
        && !function_visible_from(db, calling_scope, f)
    {
        return false;
    }
    first_closed_internal(db, calling_scope, target_scope).is_none()
}

/// The first INTERNAL namespace on `target_scope`'s chain that
/// `calling_scope` may not enter, if any.
pub fn first_closed_internal<'db>(
    db: &'db dyn WorkspaceDataBase,
    calling_scope: ScopeId<'db>,
    target_scope: ScopeId<'db>,
) -> Option<NamespaceDecl<'db>> {
    let sema = semantic_index(db, target_scope.file(db));
    sema.scope_iterator(db, target_scope)
        .find_map(|scope_info| match scope_info.kind {
            ScopeKind::Namespace(ns) if ns.internal(db) => {
                internal_namespace_violated(db, calling_scope, ns)
            }
            _ => None,
        })
}

/// E0407: `NAMESPACE INTERNAL N` is reachable only from inside the namespace
/// that encloses it — nested namespaces included — on its own side of the
/// library line (the same boundary as `FUNCTION PRIVATE`). Checked on the
/// TARGET's namespace chain, so a qualified, relative, or USING-imported
/// access all meet one rule. At top level the enclosing namespace is the
/// root, so INTERNAL there means "this library only".
pub fn check_namespace_visibility<'db>(
    db: &'db dyn WorkspaceDataBase,
    call_site: &CallSite<'db>,
    target_scope_id: ScopeId<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let calling_scope = call_site.get_scope_id(db);
    if let Some(violated) = first_closed_internal(db, calling_scope, target_scope_id) {
        errors.push(
            VisibilityError::InternalNamespace {
                call_site: *call_site,
                namespace: violated,
            }
            .to_diagnostic(db, calling_scope.file(db)),
        );
    }
}

/// `Some(ns)` when `calling_scope` may not enter the INTERNAL namespace `ns`:
/// it is outside the enclosing namespace, or on the other side of the
/// library line.
pub fn internal_namespace_violated<'db>(
    db: &'db dyn WorkspaceDataBase,
    calling_scope: ScopeId<'db>,
    ns: NamespaceDecl<'db>,
) -> Option<NamespaceDecl<'db>> {
    let fragments = ns.path(db).caseless(db).fragments(db).clone();
    let enclosing = &fragments[..fragments.len().saturating_sub(1)];
    let inside = match crate::hir_ty::resolver::name::enclosing_namespace_path(db, calling_scope) {
        Some(caller) => caller.caseless(db).fragments(db).starts_with(enclosing),
        None => enclosing.is_empty(),
    };
    let is_library =
        |scope: ScopeId<'db>| crate::check::check_duplicates::is_library_file(db, scope.file(db));
    let cross_origin = is_library(calling_scope) != is_library(ns.scope_id(db));
    (!inside || cross_origin).then_some(ns)
}

/// Check that non-test code does not reference {test}-annotated items.
///
/// Test items can reference anything, but non-test items cannot reference test items.
pub fn check_test_visibility<'db>(
    db: &'db dyn WorkspaceDataBase,
    call_site: &CallSite<'db>,
    target_scope_id: ScopeId<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let target_scope = get_scope(db, target_scope_id);
    if !target_scope.is_test(db) {
        return;
    }
    let caller_scope = get_scope(db, call_site.scope);
    if caller_scope.is_test(db) {
        return;
    }
    errors.push(
        VisibilityError::TestOnly {
            call_site: *call_site,
        }
        .to_diagnostic(db, call_site.get_scope_id(db).file(db)),
    );
}

/// Check if the calling scope is in a POU that derives from the method's POU
fn is_derived_pou<'db>(
    db: &'db dyn WorkspaceDataBase,
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
            child
                .get_scope_id(db)
                .inheritors(db)
                .values()
                .any(|p| *p == parent)
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
    db: &'db dyn WorkspaceDataBase,
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
    db: &'db dyn WorkspaceDataBase,
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
