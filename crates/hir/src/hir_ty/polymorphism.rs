use db::WorkspaceDataBase;

use crate::{
    hir_def::{
        expressions::spec::SpecKind,
        pous::{class::Class, function_block::FunctionBlock, interface::Interface, pou::Pou},
    },
    hir_ty::resolver::name::resolve_namespace_access,
};

/// Check if a class directly implements the given interface.
pub fn class_implements<'db>(
    db: &'db dyn WorkspaceDataBase,
    class: Class<'db>,
    target: Interface<'db>,
) -> bool {
    class
        .implements(db)
        .iter()
        .filter_map(|spec| resolve_spec_to_interface(db, spec))
        .any(|iface| iface == target)
}

/// Check if a function block directly implements the given interface.
pub fn fb_implements<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: FunctionBlock<'db>,
    target: Interface<'db>,
) -> bool {
    fb.implements(db)
        .iter()
        .filter_map(|spec| resolve_spec_to_interface(db, spec))
        .any(|iface| iface == target)
}

/// Check if an interface extends another interface (directly or transitively).
pub fn interface_extends<'db>(
    db: &'db dyn WorkspaceDataBase,
    child: Interface<'db>,
    target: Interface<'db>,
) -> bool {
    if child == target {
        return true;
    }

    if let Some(extends) = child.extends(db) {
        for spec in extends {
            if let Some(parent) = resolve_spec_to_interface(db, spec)
                && interface_extends(db, parent, target)
            {
                return true;
            }
        }
    }
    false
}

/// Check if a POU implements the given interface (directly or through inheritance).
///
/// For classes and FBs, checks both direct IMPLEMENTS and transitive
/// interface extension (if the POU implements ITF2 which extends ITF1,
/// it also satisfies ITF1).
pub fn pou_implements_interface<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    target: Interface<'db>,
) -> bool {
    let implemented: Vec<Interface<'db>> = match pou {
        Pou::Class(c) => c
            .implements(db)
            .iter()
            .filter_map(|spec| resolve_spec_to_interface(db, spec))
            .collect(),
        Pou::FunctionBlock(fb) => fb
            .implements(db)
            .iter()
            .filter_map(|spec| resolve_spec_to_interface(db, spec))
            .collect(),
        _ => return false,
    };

    implemented
        .iter()
        .any(|iface| interface_extends(db, *iface, target))
}

/// Check if `derived` is a subclass of `base` (directly or transitively through EXTENDS).
pub fn is_subclass_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    derived: Class<'db>,
    base: Class<'db>,
) -> bool {
    if derived == base {
        return true;
    }
    if let Some(extends_spec) = derived.extends(db)
        && let Some(parent) = resolve_spec_to_class(db, extends_spec)
    {
        return is_subclass_of(db, parent, base);
    }
    false
}

/// Check if `derived` FB extends `base` FB (directly or transitively).
pub fn is_sub_fb_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    derived: FunctionBlock<'db>,
    base: FunctionBlock<'db>,
) -> bool {
    if derived == base {
        return true;
    }
    if let Some(extends_spec) = derived.extends(db)
        && let Some(parent) = resolve_spec_to_fb(db, extends_spec)
    {
        return is_sub_fb_of(db, parent, base);
    }
    false
}

fn resolve_spec_to_interface<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: &crate::hir_def::expressions::spec::Spec<'db>,
) -> Option<Interface<'db>> {
    if let SpecKind::Target(target) = spec.kind(db)
        && let Some(Pou::Interface(iface)) = resolve_namespace_access(db, &target.path).found()
    {
        return Some(iface);
    }
    None
}

fn resolve_spec_to_class<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: &crate::hir_def::expressions::spec::Spec<'db>,
) -> Option<Class<'db>> {
    if let SpecKind::Target(target) = spec.kind(db)
        && let Some(Pou::Class(class)) = resolve_namespace_access(db, &target.path).found()
    {
        return Some(class);
    }
    None
}

fn resolve_spec_to_fb<'db>(
    db: &'db dyn WorkspaceDataBase,
    spec: &crate::hir_def::expressions::spec::Spec<'db>,
) -> Option<FunctionBlock<'db>> {
    if let SpecKind::Target(target) = spec.kind(db)
        && let Some(Pou::FunctionBlock(fb)) = resolve_namespace_access(db, &target.path).found()
    {
        return Some(fb);
    }
    None
}
