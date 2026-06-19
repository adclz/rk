//! Builder for the debug-symbol table. The on-wire types live in the shared
//! [`debug_format`] crate (so the runtime can read them without depending on the
//! compiler); this module walks MIR to populate them.
//!
//! Scope: config/resource globals and program-instance fields, walked down to
//! their elementary leaves — through nested structs (dotted paths) and arrays
//! (`[i]` / `[i,j]` IEC subscripts). Enums and subranges emit a leaf of their
//! underlying integer; STRING emits a leaf carrying its capacity (the runtime
//! decodes the length prefix). Pointers (REF_TO / VAR_IN_OUT — a raw address)
//! are not yet emitted as leaves.

pub use debug_format::{
    DEBUG_SYMBOLS_SECTION, DEBUG_SYMBOLS_VERSION, DebugSymbols, SymType, Symbol,
};

use crate::types::{MirElementary, MirType};
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;

/// Map a resolved elementary type to its on-wire [`SymType`] tag.
pub fn sym_type_of(e: MirElementary) -> SymType {
    match e {
        MirElementary::Bool => SymType::Bool,
        MirElementary::SInt => SymType::SInt,
        MirElementary::Int => SymType::Int,
        MirElementary::DInt => SymType::DInt,
        MirElementary::LInt => SymType::LInt,
        MirElementary::USInt => SymType::USInt,
        MirElementary::UInt => SymType::UInt,
        MirElementary::UDInt => SymType::UDInt,
        MirElementary::ULInt => SymType::ULInt,
        MirElementary::Byte => SymType::Byte,
        MirElementary::Word => SymType::Word,
        MirElementary::DWord => SymType::DWord,
        MirElementary::LWord => SymType::LWord,
        MirElementary::Real => SymType::Real,
        MirElementary::LReal => SymType::LReal,
        MirElementary::Char => SymType::Char,
        MirElementary::Time => SymType::Time,
        MirElementary::LTime => SymType::LTime,
        MirElementary::Date => SymType::Date,
        MirElementary::LDate => SymType::LDate,
        MirElementary::Tod => SymType::Tod,
        MirElementary::LTod => SymType::LTod,
        MirElementary::DateAndTime => SymType::DateAndTime,
        MirElementary::LDateTime => SymType::LDateTime,
    }
}

/// Max array elements enumerated as individual leaves, bounding symbol-table
/// growth on large arrays. An array larger than this contributes no leaves.
const MAX_ARRAY_LEAVES: u32 = 1024;

/// Recursively emit a [`Symbol`] for every elementary leaf reachable from a
/// root variable at `addr` with type `ty`, naming each by its dotted/subscripted
/// `path`. Structs add `.field` and their offset; arrays add `[i]`/`[i,j]` and
/// `i * element_size`; enums/subranges emit one leaf of their underlying
/// integer; STRING emits a leaf carrying its capacity. Pointers are not emitted
/// (see module docs). The match is exhaustive so a new [`MirType`] forces a
/// deliberate debug-symbols decision.
pub fn walk_type(
    db: &dyn WorkspaceDataBase,
    path: &str,
    addr: u32,
    ty: &MirType,
    out: &mut Vec<Symbol>,
) {
    match ty {
        MirType::Elementary(e) => out.push(Symbol {
            path: path.to_string(),
            address: addr,
            size: e.size_bytes(),
            ty: sym_type_of(*e),
        }),
        // Enum / Subrange are stored as their underlying integer; emit one leaf
        // of that type. (Symbolic variant / bound display is a richer wire format.)
        MirType::Enum(e) => out.push(Symbol {
            path: path.to_string(),
            address: addr,
            size: e.storage.size_bytes(),
            ty: sym_type_of(e.storage),
        }),
        MirType::Subrange(s) => out.push(Symbol {
            path: path.to_string(),
            address: addr,
            size: s.base.size_bytes(),
            ty: sym_type_of(s.base),
        }),
        MirType::Struct(s) => {
            for f in &s.fields {
                let child = format!("{path}.{}", f.name.text(db));
                walk_type(db, &child, addr + f.offset, &f.ty, out);
            }
        }
        // Flat row-major elements (`lower_func`: `offset += idx * element_size`),
        // each named by its IEC subscript(s); recurses, so an array of
        // structs/arrays expands to scalar leaves. Capped to bound the table —
        // note nested aggregates can still multiply below the cap.
        MirType::Array(a) => {
            if a.total_elements > MAX_ARRAY_LEAVES {
                return;
            }
            for k in 0..a.total_elements {
                let child = array_index_path(path, k, &a.dimensions);
                walk_type(db, &child, addr + k * a.element_size, &a.element_type, out);
            }
        }
        // STRING → one leaf carrying capacity; the runtime reads the 4-byte len
        // prefix then that many UTF-8 bytes.
        MirType::String { capacity } => out.push(Symbol {
            path: path.to_string(),
            address: addr,
            size: 4 + *capacity,
            ty: SymType::String {
                capacity: *capacity,
            },
        }),
        // Pointers are raw addresses; not emitted yet.
        MirType::Pointer(_) | MirType::Void => {}
    }
}

/// Build an array element's IEC-subscripted path from the flat row-major index
/// `flat` and the array's `dimensions` (`(lower, upper)` per dim). IEC subscript
/// of a dimension = `lower + index`; the rightmost dimension varies fastest, so
/// e.g. flat 0 of `ARRAY[1..2, 1..3]` is `[1,1]`, flat 3 is `[2,1]`.
fn array_index_path(path: &str, flat: u32, dimensions: &[(i64, i64)]) -> String {
    if dimensions.is_empty() {
        return format!("{path}[{flat}]");
    }
    let mut subs = vec![0i64; dimensions.len()];
    let mut rem = flat as i64;
    for d in (0..dimensions.len()).rev() {
        let (lo, hi) = dimensions[d];
        let size = (hi - lo + 1).max(1);
        subs[d] = lo + rem % size;
        rem /= size;
    }
    let joined = subs
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!("{path}[{joined}]")
}

/// Emit symbols for one root variable named `root` whose storage starts at
/// `base` and has type `ty` — the entry point [`walk_type`] recurses from.
pub fn collect_root(
    db: &dyn WorkspaceDataBase,
    root: &str,
    base: u32,
    ty: &MirType,
    out: &mut Vec<Symbol>,
) {
    walk_type(db, root, base, ty, out);
}

/// A leaf symbol's dotted path from a root segment and a field name.
pub fn join_path(db: &dyn WorkspaceDataBase, root: &str, field: Ident) -> String {
    format!("{root}.{}", field.text(db))
}
