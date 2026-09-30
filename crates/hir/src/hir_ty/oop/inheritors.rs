//! The reverse of [`explicit_bases`]: the POUs that name a POU as a base.
//! ty keeps no such relation, since Python only asks a class for its
//! subclasses at run time; rk needs it for go-to-implementation, and for the
//! methods a call through a base may reach.

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use rustc_hash::{FxHashMap, FxHashSet};

use super::explicit_bases;
use crate::hir_def::pous::pou::Pou;
use crate::hir_ty::index_graphs::{all_files, file_global_pous, file_namespaces};

/// A file's POUs keyed by each base they name: the FUNCTION_BLOCK or CLASS
/// they extend, the INTERFACEs they implement or extend. Per file, like
/// `file_namespace_map`, so it invalidates with the file it describes and a
/// lookup is one probe per file.
#[salsa::tracked(returns(ref))]
pub fn file_inheritors<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
) -> FxHashMap<Pou<'db>, Vec<Pou<'db>>> {
    let namespaced = file_namespaces(db, file).iter().flat_map(|ns| ns.pous(db));
    let mut map: FxHashMap<Pou<'db>, Vec<Pou<'db>>> = FxHashMap::default();
    for pou in file_global_pous(db, file).iter().chain(namespaced) {
        let bases = explicit_bases(db, *pou);
        let interfaces = bases.interfaces.iter().map(|iface| Pou::Interface(*iface));
        for base in bases.extends.into_iter().chain(interfaces) {
            map.entry(base).or_default().push(*pou);
        }
    }
    map
}

/// The POUs that name `pou` as a base: the FBs and CLASSes extending it, or
/// the POUs implementing or extending an INTERFACE.
pub fn inheritors<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Vec<Pou<'db>> {
    all_files(db)
        .filter_map(|file| file_inheritors(db, file).get(&pou))
        .flatten()
        .copied()
        .collect()
}

/// Every POU that inherits from `pou`, directly or through others: an FB's
/// or CLASS's derived POUs, an INTERFACE's implementers and the POUs deriving
/// from them. A cycle of bases does not bring `pou` back.
pub fn descendants<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Vec<Pou<'db>> {
    let mut seen = FxHashSet::from_iter([pou]);
    let mut found = Vec::new();
    let mut pending = vec![pou];
    while let Some(base) = pending.pop() {
        for inheritor in inheritors(db, base) {
            if seen.insert(inheritor) {
                found.push(inheritor);
                pending.push(inheritor);
            }
        }
    }
    found
}
