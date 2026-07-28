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
/// v2 adds `SymType::String { capacity }`; v3 adds `Symbol.global`.
pub const DEBUG_SYMBOLS_VERSION: u16 = 4;

/// The complete debug-symbol table for a module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugSymbols {
    pub version: u16,
    /// Every debuggable variable, sorted by `path`.
    pub symbols: Vec<Symbol>,
    /// One descriptor per ARRAY-typed variable (any size), sorted by `path`.
    ///
    /// This is the layer every debug format has and ours lacked: DWARF's
    /// `DW_TAG_array_type`, a toolchain's symbol configuration — the SHAPE is
    /// described once and elements are computed on demand, instead of being
    /// enumerated. `symbols` still carries eagerly-expanded leaves for small
    /// arrays as a convenience for the pushed snapshot; past the leaf budget a
    /// consumer resolves `a[i]` through the descriptor: bounds-check against
    /// `dimensions`, row-major flatten, `address + flat * elem_size`. Before
    /// this existed, an array past the cap contributed NOTHING — invisible to
    /// the monitor and the debugger, with no marker saying so.
    #[serde(default)]
    pub arrays: Vec<ArraySym>,
}

/// The shape of one array-typed variable — enough to locate and decode any
/// element without it having been enumerated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArraySym {
    /// Fully qualified dotted path of the array itself (e.g. `Main.samples`).
    pub path: String,
    /// Absolute address of element 0 in linear memory.
    pub address: u32,
    /// `(lower, upper)` bounds per dimension, declaration order; the rightmost
    /// dimension varies fastest (row-major, matching the layout).
    pub dimensions: Vec<(i64, i64)>,
    /// Product of all dimension sizes.
    pub total_elements: u32,
    /// Byte stride between consecutive elements.
    pub elem_size: u32,
    /// How to decode ONE element, when the element is scalar (elementary, an
    /// enum/subrange's underlying integer, or a STRING). `None` when the
    /// element is itself an aggregate — locating THOSE members needs a type
    /// table, which this format does not carry yet.
    pub elem_ty: Option<SymType>,
    /// `true` for a config/resource `VAR_GLOBAL`.
    pub global: bool,
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
    /// `true` for a config/resource `VAR_GLOBAL`, `false` for a program-instance
    /// field — lets a debugger group variables into Globals vs Locals.
    pub global: bool,
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
    String {
        capacity: u32,
    },
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
            arrays: Vec::new(),
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
    /// The function's IEC name (e.g. `Motor#spin` for a method, `Main$__body__`
    /// for a POU body). Stored verbatim — never parsed by the debug layer.
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
pub const DEBUG_LOCALS_VERSION: u16 = 2;

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
    /// Memory-resident locals expanded to leaves like
    /// [`DebugSymbols::symbols`], with frame-relative paths and absolute
    /// static addresses (IEC forbids recursion).
    #[serde(default)]
    pub memory: Vec<Symbol>,
    /// Array descriptors for the frame's memory locals — same on-demand
    /// element resolution as [`DebugSymbols::arrays`].
    #[serde(default)]
    pub arrays: Vec<ArraySym>,
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

// ---------------------------------------------------------------------------
// Retain map — per-field RETAIN persistence ranges
// ---------------------------------------------------------------------------

/// Custom wasm section carrying the [`RetainMap`]. Load-bearing for IEC
/// semantics: it must survive release optimization.
pub const RETAIN_MAP_SECTION: &str = "retain-map";

/// On-wire format version for [`RetainMap`].
pub const RETAIN_MAP_VERSION: u16 = 2;

/// The retained byte ranges of a module, inside the retain band;
/// everything in the band not covered is transient and keeps its
/// `__init` cold-start value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetainMap {
    pub version: u16,
    /// Layout identity: FNV-1a over the sorted `(path, size, type_key)`
    /// sequence, excluding addresses so the band may re-base between builds.
    /// `type_key` is included because size alone does not identify a value.
    pub layout_hash: u64,
    /// Retained ranges, sorted by `path` (deterministic; file payload order).
    pub ranges: Vec<RetainRange>,
}

/// One retained byte range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetainRange {
    /// Fully qualified dotted path of the retained variable (or the retained
    /// sub-run of one, when by-ref pointer holes split it).
    pub path: String,
    /// Absolute address in linear memory (post band-relocation).
    pub addr: u32,
    /// Size in bytes.
    pub size: u32,
    /// Structural identity of the retained value's type (see
    /// [`RetainMap::layout_hash`]); the runtime only compares it.
    pub type_key: u32,
}

impl RetainMap {
    /// Build from ranges: sorts by path and stamps the layout hash.
    pub fn new(mut ranges: Vec<RetainRange>) -> Self {
        ranges.sort_by(|a, b| a.path.cmp(&b.path));
        let layout_hash = Self::hash_layout(&ranges);
        RetainMap {
            version: RETAIN_MAP_VERSION,
            layout_hash,
            ranges,
        }
    }

    /// FNV-1a over the sorted `(path, size, type_key)` sequence.
    fn hash_layout(ranges: &[RetainRange]) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        let mut h = FNV_OFFSET;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(FNV_PRIME);
            }
        };
        for r in ranges {
            eat(r.path.as_bytes());
            eat(&[0]); // separator
            eat(&r.size.to_le_bytes());
            eat(&r.type_key.to_le_bytes());
        }
        h
    }

    /// Total retained payload size in bytes (the v2 retain file's data length).
    pub fn payload_size(&self) -> u32 {
        self.ranges.iter().map(|r| r.size).sum()
    }

    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("RetainMap serialization should not fail")
    }

    /// Deserialize from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        rmp_serde::from_slice(bytes)
    }
}
