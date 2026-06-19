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
/// v2 adds `SymType::String { capacity }`.
pub const DEBUG_SYMBOLS_VERSION: u16 = 2;

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
    /// Fixed-capacity IEC STRING: a 4-byte little-endian `len` prefix followed by
    /// `capacity` bytes of UTF-8 buffer (total `4 + capacity`).
    String { capacity: u32 },
}

impl SymType {
    /// In-memory slot size in bytes. Every elementary value occupies a 4- or
    /// 8-byte slot in linear memory.
    pub fn size_bytes(self) -> u32 {
        match self {
            SymType::String { capacity } => 4 + capacity,
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
        // A STRING slot is never a scalar 64-bit value, even when 4 + capacity == 8.
        !matches!(self, SymType::String { .. }) && self.size_bytes() == 8
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

// ---------------------------------------------------------------------------
// Functions — naming wasm stack frames
// ---------------------------------------------------------------------------

/// Custom wasm section carrying the MessagePack-encoded [`DebugFunctions`].
pub const DEBUG_FUNCTIONS_SECTION: &str = "debug-functions";

/// On-wire format version for [`DebugFunctions`].
pub const DEBUG_FUNCTIONS_VERSION: u16 = 1;

/// Each defined (non-import) wasm function to its IEC name, keyed by the
/// `DefinedFuncIndex` wasmtime's `FrameHandle` reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugFunctions {
    pub version: u16,
    /// One entry per nameable defined function, sorted by `defined_index`.
    pub functions: Vec<FuncEntry>,
}

/// One defined wasm function and its IEC name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncEntry {
    /// `DefinedFuncIndex` (excludes imports).
    pub defined_index: u32,
    /// The function's IEC name (e.g. `Motor$spin`, `Main$__body__`).
    pub name: String,
}

impl DebugFunctions {
    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("DebugFunctions serialization should not fail")
    }

    /// Deserialize from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}

// ---------------------------------------------------------------------------
// Lines — mapping wasm code offsets to IEC source positions
// ---------------------------------------------------------------------------

/// Custom wasm section carrying the MessagePack-encoded [`DebugLines`].
pub const DEBUG_LINES_SECTION: &str = "debug-lines";

/// On-wire format version for [`DebugLines`]. v2 populates `files` (per-file
/// `LineEntry::file` indices); v1 hardcoded file 0.
pub const DEBUG_LINES_VERSION: u16 = 2;

/// Per-function line tables: within-body offset → IEC source position. A
/// consumer converts an absolute `wasm_pc` to within-body via the body's
/// start, then takes the largest `offset <= within_body`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugLines {
    pub version: u16,
    /// Source file paths, indexed by `LineEntry::file`.
    pub files: Vec<String>,
    /// Per defined function, sorted by `defined_index`.
    pub functions: Vec<FuncLines>,
}

/// The line table for one defined wasm function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncLines {
    /// `DefinedFuncIndex` (excludes imports) — matches `FrameHandle`.
    pub defined_index: u32,
    /// Statement entries, sorted ascending by `offset`.
    pub lines: Vec<LineEntry>,
}

/// One statement's within-body offset and the source position it maps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineEntry {
    /// Within-body byte offset of the statement's first instruction.
    pub offset: u32,
    /// Index into [`DebugLines::files`].
    pub file: u32,
    /// 0-based source line.
    pub line: u32,
    /// 0-based source column.
    pub col: u32,
}

impl DebugLines {
    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("DebugLines serialization should not fail")
    }

    /// Deserialize from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}

// ---------------------------------------------------------------------------
// Locals — labeling a frame's wasm local slots with IEC names/types
// ---------------------------------------------------------------------------

/// Custom wasm section carrying the MessagePack-encoded [`DebugLocals`].
pub const DEBUG_LOCALS_SECTION: &str = "debug-locals";

/// On-wire format version for [`DebugLocals`].
pub const DEBUG_LOCALS_VERSION: u16 = 1;

/// Per-function scalar-local tables: wasm local slot → IEC name and type;
/// memory-resident variables are reached via [`DebugSymbols`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugLocals {
    pub version: u16,
    /// Per defined function, sorted by `defined_index`.
    pub functions: Vec<FuncLocals>,
}

/// The scalar-local table for one defined wasm function.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncLocals {
    /// `DefinedFuncIndex` (excludes imports) — matches `FrameHandle`.
    pub defined_index: u32,
    /// Scalar locals, sorted ascending by `wasm_index`.
    pub locals: Vec<LocalVar>,
}

/// One scalar local: its wasm local slot index + IEC name and type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalVar {
    /// Index for `FrameHandle::local(i)`.
    pub wasm_index: u32,
    pub name: String,
    pub ty: SymType,
}

impl DebugLocals {
    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("DebugLocals serialization should not fail")
    }

    /// Deserialize from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}
