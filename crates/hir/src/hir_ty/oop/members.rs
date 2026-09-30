use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::HirNodeInfo;
use crate::hir_def::{
    interned::identifier::Ident,
    pous::{class::MethodDecl, pou::Pou},
};
use crate::hir_ty::head::inheritance::MethodRef;

use super::ancestry;

/// A method as a POU sees it, and the POU that declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub struct ClassMember<'db> {
    pub owner: Pou<'db>,
    pub method: MethodRef<'db>,
}

/// Every method a POU answers to, found the way a call finds it: the
/// nearest declaration up its `EXTENDS` chain, else the prototype of an
/// INTERFACE it implements. As ty looks a member up along a class's MRO.
#[derive(Debug, Clone, Default, PartialEq, Eq, salsa::Update)]
pub struct ClassMembers<'db> {
    /// By name, the POU's own methods included.
    pub methods: FxHashMap<Ident, ClassMember<'db>>,
    /// For each method the POU declares that a base declares too, the
    /// base's: a concrete method before a prototype.
    pub overridden: FxHashMap<Ident, ClassMember<'db>>,
    /// A name two INTERFACEs declare, neither extending the other (E0110).
    pub duplicates: Vec<(ClassMember<'db>, ClassMember<'db>)>,
}

impl<'db> ClassMembers<'db> {
    /// The methods `pou` inherits and does not declare itself.
    pub fn inherited(&self, pou: Pou<'db>) -> impl Iterator<Item = (&Ident, &ClassMember<'db>)> {
        self.methods
            .iter()
            .filter(move |(_, member)| member.owner != pou)
    }

    /// The method a call of `name` runs: none for a prototype, which has
    /// no body.
    pub fn implementation(&self, name: &Ident) -> Option<MethodDecl<'db>> {
        match self.methods.get(name)?.method {
            MethodRef::Declared(decl) => Some(decl),
            MethodRef::Prototype(_) => None,
        }
    }
}

#[salsa::tracked(returns(ref))]
pub fn class_members<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> ClassMembers<'db> {
    let declared = |owner: Pou<'db>| &owner.get_scope_id(db).def_map(db).declared_methods;
    // Whether INTERFACE `a` extends INTERFACE `b`.
    let extends = |a: Pou<'db>, b: Pou<'db>| match b {
        Pou::Interface(b) => ancestry(db, a).implements(b),
        _ => false,
    };
    let lineage = ancestry(db, pou);
    let mut members = ClassMembers::default();

    // Base first, so the nearest declaration takes the name.
    for owner in lineage.chain.iter().rev() {
        for (name, method) in declared(*owner) {
            let member = ClassMember {
                owner: *owner,
                method: *method,
            };
            if let Some(base) = members.methods.insert(*name, member)
                && *owner == pou
            {
                members.overridden.insert(*name, base);
            }
        }
    }

    // The prototypes of the INTERFACEs it implements (an INTERFACE: those it
    // extends). Of two, the one whose INTERFACE extends the other's is
    // nearer; two unrelated ones are a conflict.
    let mut prototypes: FxHashMap<Ident, ClassMember<'db>> = FxHashMap::default();
    for iface in &lineage.interfaces {
        let owner = Pou::Interface(*iface);
        if owner == pou {
            continue;
        }
        for (name, method) in declared(owner) {
            let member = ClassMember {
                owner,
                method: *method,
            };
            match prototypes.get(name) {
                None => {
                    prototypes.insert(*name, member);
                }
                Some(other) if *other == member => {}
                Some(other) if extends(owner, other.owner) => {
                    prototypes.insert(*name, member);
                }
                Some(other) if extends(other.owner, owner) => {}
                Some(other) => members.duplicates.push((*other, member)),
            }
        }
    }
    for (name, prototype) in prototypes {
        match members.methods.get(&name) {
            None => {
                members.methods.insert(name, prototype);
            }
            // Its own declaration implements or redeclares the prototype.
            Some(own) if own.owner == pou => {
                members.overridden.entry(name).or_insert(prototype);
            }
            // A base's method implements it.
            Some(_) => {}
        }
    }
    members
}
