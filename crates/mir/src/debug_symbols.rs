//! Debug symbols — the minimal, non-DWARF symbol table the runtime uses to
//! read and write IEC variables by name while the PLC runs.
//!
//! Embedded in the compiled **core** module as the [`DEBUG_SYMBOLS_SECTION`]
//! custom section (MessagePack-encoded). The runtime instantiates that core
//! module with an imported `env.memory`, so every [`Symbol`] address is an
//! absolute offset into the shared linear memory the host can peek/poke
//! directly — no marshalling, identical on wasmtime and WAMR.
//!
//! This is the symbol-table half of debug info (DWARF's `.debug_info`), *not*
//! a source map: it maps **variables → addresses**, keyed by name. Mapping
//! **code offsets → source lines** is a separate, later table.
//!
//! v1 scope: variables of *elementary* type only — config/resource globals and
//! program-instance fields, with nested structs traversed to build dotted
//! paths. Arrays, strings, enums, subranges and pointers are not yet emitted as
//! leaves (a struct that *contains* them is still traversed for its scalars).

use serde::{Deserialize, Serialize};

use crate::types::{MirElementary, MirType};
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;

/// Name of the custom wasm section carrying the MessagePack-encoded
/// [`DebugSymbols`] inside the compiled core module.
pub const DEBUG_SYMBOLS_SECTION: &str = "debug-symbols";

/// On-wire format version. Bump on any breaking change to the layout below.
pub const DEBUG_SYMBOLS_VERSION: u16 = 1;

/// The complete debug-symbol table for a module.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebugSymbols {
    pub version: u16,
    /// Every debuggable variable, sorted by `path` for deterministic output.
    pub symbols: Vec<Symbol>,
}

/// One debuggable variable: an elementary-typed leaf at a stable linear-memory
/// address.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Symbol {
    /// Fully qualified dotted path (e.g. `Main.motor.speed`). For a config or
    /// resource `VAR_GLOBAL` this is just the global's name.
    pub path: String,
    /// Absolute address in the imported linear memory (post band-relocation).
    pub address: u32,
    /// Size in bytes of the value at `address`.
    pub size: u32,
    /// Elementary type of the value — tells the host how to decode the bytes.
    pub ty: SymType,
}

/// Elementary type tag — mirrors [`MirElementary`], kept as its own enum so the
/// on-wire format doesn't shift when MIR internals change.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SymType {
    Bool,
    SInt,
    Int,
    DInt,
    LInt,
    USInt,
    UInt,
    UDInt,
    ULInt,
    Byte,
    Word,
    DWord,
    LWord,
    Real,
    LReal,
    Char,
    Time,
    LTime,
    Date,
    LDate,
    Tod,
    LTod,
    DateAndTime,
    LDateTime,
}

impl From<MirElementary> for SymType {
    fn from(e: MirElementary) -> Self {
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
}

impl DebugSymbols {
    pub fn new() -> Self {
        DebugSymbols {
            version: DEBUG_SYMBOLS_VERSION,
            symbols: Vec::new(),
        }
    }

    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("DebugSymbols serialization should not fail")
    }

    /// Deserialize from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}

impl Default for DebugSymbols {
    fn default() -> Self {
        Self::new()
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
            ty: SymType::from(*e),
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
