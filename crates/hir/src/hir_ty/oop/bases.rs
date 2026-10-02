// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::hir_def::{
    expressions::spec::{Spec, SpecKind},
    pous::{interface::Interface, pou::Pou},
};
use crate::hir_ty::resolver::name::resolve_namespace_access;

/// Where a POU's header names a base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum BaseRole {
    /// An FB's or CLASS's `EXTENDS`: an FB extends an FB or a CLASS, a
    /// CLASS only a CLASS.
    Extends,
    /// An FB's or CLASS's `IMPLEMENTS`, or an INTERFACE's `EXTENDS`, which
    /// names an INTERFACE.
    Interface,
}

/// A base a POU's header names, and the POU the name resolves to.
#[derive(Debug, Clone, Copy)]
pub struct WrittenBase<'db> {
    /// The POU whose header names it.
    pub pou: Pou<'db>,
    pub role: BaseRole,
    pub spec: Spec<'db>,
    /// `None` when the name is no POU, which is reported where it is written.
    pub target: Option<Pou<'db>>,
}

impl WrittenBase<'_> {
    /// Whether the target is of the kind its place in the header takes.
    pub fn fits(&self) -> bool {
        match (self.role, self.target) {
            (BaseRole::Extends, Some(target)) => can_extend(self.pou, target),
            (BaseRole::Interface, Some(target)) => matches!(target, Pou::Interface(_)),
            (_, None) => false,
        }
    }
}

/// Whether `pou` may name `base` in its `EXTENDS`: an FB extends an FB or a
/// CLASS, a CLASS only a CLASS, which has no body to inherit an FB's into
/// (IEC 61131-3, tables 40 and 48).
pub fn can_extend(pou: Pou, base: Pou) -> bool {
    matches!(
        (pou, base),
        (Pou::FunctionBlock(_), Pou::FunctionBlock(_) | Pou::Class(_))
            | (Pou::Class(_), Pou::Class(_))
    )
}

/// Every base `pou`'s header names, in the order written: an FB's or CLASS's
/// `EXTENDS`, then its `IMPLEMENTS`; an INTERFACE's `EXTENDS`. This is for
/// diagnostics, which need where each base is written; the bases a POU
/// inherits from are [`explicit_bases`].
pub fn written_bases<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Vec<WrittenBase<'db>> {
    let (extends, interfaces): (Option<&Spec<'db>>, &[Spec<'db>]) = match pou {
        Pou::FunctionBlock(fb) => (fb.extends(db), fb.implements(db)),
        Pou::Class(class) => (class.extends(db), class.implements(db)),
        Pou::Interface(iface) => (None, iface.extends(db).map_or(&[], Vec::as_slice)),
        _ => return Vec::new(),
    };
    let resolve = |spec: &Spec<'db>| match spec.kind(db) {
        SpecKind::Target(target) => resolve_namespace_access(db, &target.path).found(),
        _ => None,
    };
    extends
        .into_iter()
        .map(|spec| (BaseRole::Extends, spec))
        .chain(interfaces.iter().map(|spec| (BaseRole::Interface, spec)))
        .map(|(role, spec)| WrittenBase {
            pou,
            role,
            spec: *spec,
            target: resolve(spec),
        })
        .collect()
}

/// The bases `pou` inherits from: the FB or CLASS it `EXTENDS`, and the
/// INTERFACEs it implements (an INTERFACE: those it extends), in the order
/// written. A base of the wrong kind is none (E1130).
#[derive(Debug, Clone, Default, PartialEq, Eq, salsa::Update)]
pub struct ExplicitBases<'db> {
    pub extends: Option<Pou<'db>>,
    pub interfaces: Vec<Interface<'db>>,
}

#[salsa::tracked(returns(ref))]
pub fn explicit_bases<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> ExplicitBases<'db> {
    let mut bases = ExplicitBases::default();
    for base in written_bases(db, pou) {
        match base.target {
            Some(Pou::Interface(iface)) if base.fits() => bases.interfaces.push(iface),
            Some(target) if base.fits() => bases.extends = Some(target),
            _ => {}
        }
    }
    bases
}
