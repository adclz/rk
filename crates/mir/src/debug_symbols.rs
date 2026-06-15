//! Builder for the debug-symbol table. The on-wire types live in the shared
//! [`debug_format`] crate (so the runtime can read them without depending on the
//! compiler); this module walks MIR to populate them.
//!
//! v1 scope: variables of *elementary* type only — config/resource globals and
//! program-instance fields, with nested structs traversed to build dotted
//! paths. Arrays, strings, enums, subranges and pointers are not yet emitted as
//! leaves (a struct that *contains* them is still traversed for its scalars).

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

/// Recursively emit a [`Symbol`] for every elementary leaf reachable from a
/// root variable at `addr` with type `ty`, naming each by its dotted `path`.
/// Structs are traversed (field offsets added to `addr`); aggregate or opaque
/// types (arrays, strings, enums, subranges, pointers) are skipped in v1.
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
        MirType::Struct(s) => {
            for f in &s.fields {
                let child = format!("{path}.{}", f.name.text(db));
                walk_type(db, &child, addr + f.offset, &f.ty, out);
            }
        }
        // v1: arrays, strings, enums, subranges and pointers are not yet
        // emitted as debuggable leaves.
        _ => {}
    }
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
