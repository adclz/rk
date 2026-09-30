use db::WorkspaceDataBase;

use crate::{
    hir_def::pous::{interface::Interface, pou::Pou},
    hir_ty::oop::explicit_bases,
};

/// Check if an interface extends another interface (directly or transitively).
pub fn interface_extends<'db>(
    db: &'db dyn WorkspaceDataBase,
    child: Interface<'db>,
    target: Interface<'db>,
) -> bool {
    if child == target {
        return true;
    }
    explicit_bases(db, Pou::Interface(child))
        .interfaces
        .iter()
        .any(|parent| interface_extends(db, *parent, target))
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
    if !matches!(pou, Pou::Class(_) | Pou::FunctionBlock(_)) {
        return false;
    }
    explicit_bases(db, pou)
        .interfaces
        .iter()
        .any(|iface| interface_extends(db, *iface, target))
}
