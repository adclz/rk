use db::WorkspaceDataBase;

use crate::hir_def::pous::{interface::Interface, pou::Pou};

use super::explicit_bases;

/// Where a POU sits in the inheritance graph: what it extends and what it
/// implements, through all its bases. Built from its bases' own ancestry,
/// which salsa caches, as ty builds a class's MRO from its bases'.
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
    /// `pou` before its bases are added.
    fn alone(pou: Pou<'db>) -> Self {
        Ancestry {
            chain: vec![pou],
            interfaces: match pou {
                Pou::Interface(iface) => vec![iface],
                _ => Vec::new(),
            },
            cyclic: false,
        }
    }

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

    fn inherit(&mut self, base: &Ancestry<'db>, chain: bool) {
        self.cyclic |= base.cyclic;
        if chain {
            for pou in &base.chain {
                if self.chain.contains(pou) {
                    self.cyclic = true;
                    break;
                }
                self.chain.push(*pou);
            }
        }
        for iface in &base.interfaces {
            if !self.interfaces.contains(iface) {
                self.interfaces.push(*iface);
            }
        }
    }
}

/// The ancestry of `pou`: its bases' ancestry, merged. A cyclic `EXTENDS` is
/// a salsa cycle here, which starts from [`cyclic_ancestry`] and settles on
/// the same result whichever POU it was entered from; a valid program never
/// forms one.
#[salsa::tracked(returns(ref), cycle_initial = cyclic_ancestry)]
pub fn ancestry<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Ancestry<'db> {
    let mut ancestry = Ancestry::alone(pou);
    let bases = explicit_bases(db, pou);
    if let Some(base) = bases.extends {
        ancestry.inherit(self::ancestry(db, base), true);
    }
    for iface in &bases.interfaces {
        ancestry.inherit(self::ancestry(db, Pou::Interface(*iface)), false);
    }
    ancestry
}

/// Where a cycle starts: the POU alone, marked cyclic.
fn cyclic_ancestry<'db>(
    _db: &'db dyn WorkspaceDataBase,
    _id: salsa::Id,
    pou: Pou<'db>,
) -> Ancestry<'db> {
    Ancestry {
        cyclic: true,
        ..Ancestry::alone(pou)
    }
}
