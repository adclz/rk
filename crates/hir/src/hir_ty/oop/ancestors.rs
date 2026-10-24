// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::hir_def::pous::{interface::Interface, pou::Pou};

use super::explicit_bases;

/// Where a POU sits in the inheritance graph: what it extends and what it
/// implements, through all its bases.
#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct Ancestry<'db> {
    /// The POU, then the FB or CLASS it extends, then that one's base,
    /// nearest first. On a cyclic `EXTENDS` (E1301/E1302), cut where it would
    /// repeat.
    pub chain: Vec<Pou<'db>>,
    /// Every INTERFACE the POU implements: its own `IMPLEMENTS`, its bases',
    /// and every INTERFACE those extend. An INTERFACE's are itself and every
    /// INTERFACE it extends.
    pub interfaces: Vec<Interface<'db>>,
    /// Whether a cyclic `EXTENDS` is on the way up.
    pub cyclic: bool,
}

impl<'db> Ancestry<'db> {
    /// The FB or CLASS `SUPER` reaches: the nearest base.
    pub fn base(&self) -> Option<Pou<'db>> {
        self.chain.get(1).copied()
    }

    /// Whether the POU is `pou` or derives from it.
    pub fn is_or_extends(&self, pou: Pou<'db>) -> bool {
        self.chain.contains(&pou)
    }

    /// Whether the POU implements `iface`, or, an INTERFACE, is or extends it.
    pub fn implements(&self, iface: Interface<'db>) -> bool {
        self.interfaces.contains(&iface)
    }
}

/// The ancestry of `pou`, walked up its bases within this one query: every
/// base is read through [`explicit_bases`], and no ancestry asks another's.
/// A cyclic `EXTENDS` is then a POU met twice on the way up, cut there,
/// rather than a salsa cycle: a fixpoint panics when anything under it
/// accumulated a value, which a file the builder could not read does.
#[salsa::tracked(returns(ref))]
pub fn ancestry<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Ancestry<'db> {
    let mut ancestry = Ancestry {
        chain: Vec::new(),
        interfaces: Vec::new(),
        cyclic: false,
    };
    walk(db, pou, true, &mut Vec::new(), &mut ancestry);
    ancestry
}

/// Adds `pou` and what it inherits to `ancestry`, in the order the bases are
/// written: the POU, its base and that one's bases, then the INTERFACEs it
/// implements or extends. `chain` while on the `EXTENDS` chain of FBs and
/// CLASSes. `path` is the walk from the POU asked about: meeting a POU on it
/// again is a cyclic `EXTENDS`, and inside such a cycle the order is the one
/// of this walk. An INTERFACE met before was walked then.
fn walk<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    chain: bool,
    path: &mut Vec<Pou<'db>>,
    ancestry: &mut Ancestry<'db>,
) {
    if path.contains(&pou) {
        ancestry.cyclic = true;
        return;
    }
    if let Pou::Interface(iface) = pou {
        if ancestry.interfaces.contains(&iface) {
            return;
        }
        ancestry.interfaces.push(iface);
    }
    if chain {
        ancestry.chain.push(pou);
    }
    path.push(pou);
    let bases = explicit_bases(db, pou);
    if let Some(base) = bases.extends {
        walk(db, base, chain, path, ancestry);
    }
    for iface in &bases.interfaces {
        walk(db, Pou::Interface(*iface), false, path, ancestry);
    }
    path.pop();
}
