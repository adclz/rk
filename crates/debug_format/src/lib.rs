//! The on-wire debug-info format shared by the compiler (which produces it) and
//! the runtime / debugger (which consume it). Pure serde data — no compiler
//! internals — so the lean runtime reads it without depending on `mir`/`hir`.
//!
//! Carried in the compiled core module as MessagePack-encoded custom sections
//! (see [`DEBUG_SYMBOLS_SECTION`]). This is the symbol-table half of debug info
//! — variables → addresses, keyed by name — not a source map.

use serde::{Deserialize, Serialize};

/// Custom wasm section carrying the MessagePack-encoded [`DebugSymbols`].
pub const DEBUG_SYMBOLS_SECTION: &str = "debug-symbols";

/// On-wire format version. Bump on any breaking change to the layout below.
pub const DEBUG_SYMBOLS_VERSION: u16 = 1;

/// The complete debug-symbol table for a module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugSymbols {
    pub version: u16,
    /// Every debuggable variable, sorted by `path`.
    pub symbols: Vec<Symbol>,
}

/// One debuggable variable: an elementary-typed leaf at a stable linear-memory
/// address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Symbol {
    /// Fully qualified dotted path (e.g. `Main.motor.speed`); for a config or
    /// resource `VAR_GLOBAL`, just the global's name.
    pub path: String,
    /// Absolute address in the imported linear memory (post band-relocation).
    pub address: u32,
    /// Size in bytes of the value at `address`.
    pub size: u32,
    /// Elementary type — tells a consumer how to decode the bytes.
    pub ty: SymType,
}

/// Elementary type tag. Mirrors the compiler's elementary types but stands on
/// its own so the wire format is independent of compiler internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl SymType {
    /// In-memory slot size in bytes. Every elementary value occupies a 4- or
    /// 8-byte slot in linear memory.
    pub fn size_bytes(self) -> u32 {
        match self {
            SymType::LInt
            | SymType::ULInt
            | SymType::LWord
            | SymType::LReal
            | SymType::LTime
            | SymType::LDate
            | SymType::LTod
            | SymType::LDateTime => 8,
            _ => 4,
        }
    }

    /// Whether this value occupies an 8-byte slot.
    pub fn is_64bit(self) -> bool {
        self.size_bytes() == 8
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
