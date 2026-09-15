//! The on-wire debug-info format shared by the compiler, which produces it,
//! and the tools that consume it: pure serde data, carried in the core
//! module as MessagePack-encoded custom sections.

use serde::{Deserialize, Serialize};

pub mod info;
pub mod test_manifest;
pub mod test_report;

pub use info::{DebugInfo, SourcePos, StackFrame, TypeMismatch, VarLoc, VarValue, decode, encode};

/// Custom wasm section carrying the MessagePack-encoded [`DebugSymbols`].
pub const DEBUG_SYMBOLS_SECTION: &str = "debug-symbols";

/// On-wire format version; bump on any breaking change. A new field goes
/// at the TAIL of its struct: `rmp_serde` encodes positionally, so
/// `#[serde(default)]` only backfills a field missing from the end.
pub const DEBUG_SYMBOLS_VERSION: u16 = 7;

/// The complete debug-symbol table for a module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugSymbols {
    pub version: u16,
    /// Every debuggable variable, sorted by `path`.
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub arrays: Vec<ArraySym>,
    /// The type table (v5): layouts referenced by `TypeId`, what makes an
    /// aggregate array element addressable on demand (`pts[7000].y`). Only
    /// referenced types appear.
    #[serde(default)]
    pub types: Vec<TypeDesc>,
    /// Every aggregate's base address, by path (v7): the address a frame is
    /// called with, which is how a debugger learns whose state a frame runs
    /// on.
    #[serde(default)]
    pub containers: Vec<ContainerSym>,
}

/// One aggregate's name and base address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContainerSym {
    /// Dotted path, as [`Symbol::path`] spells it (`P1`, `P1.motor`).
    pub path: String,
    /// Base address in linear memory.
    pub address: u32,
    /// Whether it lives in the configuration's globals.
    pub global: bool,
    /// The POU or struct this is an instance of; an address alone is
    /// ambiguous when an aggregate's first field is itself an aggregate.
    #[serde(default)]
    pub type_name: String,
}

/// Index into a `types` table.
pub type TypeId = u32;

/// The layout of one type, enough to locate any member of a value of that
/// type from its base address. Deduplicated: one entry per distinct layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeDesc {
    /// A scalar leaf — decode with the [`SymType`].
    Scalar(SymType),
    /// A struct / FB instance body: named fields at byte offsets.
    Struct {
        /// The IEC type name (`Pt`, `Motor`) — for display, never for lookup.
        name: String,
        /// Total size in bytes, padding included.
        size: u32,
        /// Fields in declaration order.
        fields: Vec<FieldDesc>,
    },
    /// A nested array (an array INSIDE a struct element reached through the
    /// type walk — top-level arrays get an [`ArraySym`] instead).
    Array {
        /// `(lower, upper)` per dimension, rightmost varying fastest.
        dimensions: Vec<(i64, i64)>,
        total_elements: u32,
        elem_size: u32,
        /// Element type.
        elem: TypeId,
    },
    /// Present but not walkable: a pointer. Locating through one needs a live
    /// dereference.
    Opaque { size: u32 },
    /// An IEC enumeration: stored as `storage`, displayed as a variant name.
    /// Last on purpose (variant index encoding).
    Enum {
        /// The IEC type name (`TrafficLight`).
        name: String,
        storage: SymType,
        /// Variants in declaration order, with their integer values.
        variants: Vec<(String, i64)>,
    },
}

/// An array element's IEC-subscripted path from its flat row-major index,
/// chained (`a[1][2]`); owned here so emitter and consumers agree.
pub fn element_path(path: &str, flat: u32, dimensions: &[(i64, i64)]) -> String {
    if dimensions.is_empty() {
        return format!("{path}[{flat}]");
    }
    let mut subs = vec![0i64; dimensions.len()];
    let mut rem = i64::from(flat);
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
        .join("][");
    format!("{path}[{joined}]")
}

/// One struct field: name, byte offset from the struct's base, type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDesc {
    pub name: String,
    pub offset: u32,
    pub ty: TypeId,
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
    /// How to decode one scalar element; `None` when the element is an
    /// aggregate (see `elem_type`).
    pub elem_ty: Option<SymType>,
    /// `true` for a configuration `VAR_GLOBAL`.
    pub global: bool,
    /// The element's layout in the carrying table's `types` (v5), for
    /// aggregate elements; `None` for scalars or a pre-v5 producer. Last on
    /// purpose: positional encoding.
    #[serde(default)]
    pub elem_type: Option<TypeId>,
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
    /// `true` for a configuration `VAR_GLOBAL`, `false` for a program-instance
    /// field — lets a debugger group variables into Globals vs Locals.
    pub global: bool,
    /// The declared type when it has a name the value cannot carry (an
    /// enumeration). Last on purpose.
    #[serde(default)]
    pub named_type: Option<TypeId>,
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
    /// The IEC 61131-3 type name, as declared.
    pub fn rk_name(self) -> &'static str {
        match self {
            SymType::Bool => "BOOL",
            SymType::SInt => "SINT",
            SymType::Int => "INT",
            SymType::DInt => "DINT",
            SymType::LInt => "LINT",
            SymType::USInt => "USINT",
            SymType::UInt => "UINT",
            SymType::UDInt => "UDINT",
            SymType::ULInt => "ULINT",
            SymType::Byte => "BYTE",
            SymType::Word => "WORD",
            SymType::DWord => "DWORD",
            SymType::LWord => "LWORD",
            SymType::Real => "REAL",
            SymType::LReal => "LREAL",
            SymType::Char => "CHAR",
            SymType::Time => "TIME",
            SymType::LTime => "LTIME",
            SymType::Date => "DATE",
            SymType::LDate => "LDATE",
            SymType::Tod => "TIME_OF_DAY",
            SymType::LTod => "LTIME_OF_DAY",
            SymType::DateAndTime => "DATE_AND_TIME",
            SymType::LDateTime => "LDATE_AND_TIME",
            SymType::String { .. } => "STRING",
        }
    }

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
            types: Vec::new(),
            containers: Vec::new(),
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

/// On-wire format version for [`DebugLocals`]. v2 adds `memory`/`arrays`
/// per function; v3 adds the module-level `types` table.
pub const DEBUG_LOCALS_VERSION: u16 = 4;

/// Per-function scalar-local tables: wasm local slot → IEC name and type;
/// memory-resident variables are reached via [`DebugSymbols`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebugLocals {
    pub version: u16,
    /// Per defined function, sorted by `defined_index`.
    pub functions: Vec<FuncLocals>,
    /// The type table the frames' `arrays` reference, module-level. Trailing.
    #[serde(default)]
    pub types: Vec<TypeDesc>,
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
    /// The wasm local holding this frame's instance pointer (v4); `None` for
    /// a FUNCTION. Not in `locals`, since a `this` is machinery, not a
    /// declared variable; its value is what [`DebugSymbols::containers`]
    /// turns back into a name.
    #[serde(default)]
    pub this_slot: Option<u32>,
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
// Schedule — what the runtime executes, and when
// ---------------------------------------------------------------------------

/// Custom wasm section carrying the [`ScheduleManifest`]. Load-bearing:
/// release optimization must keep it.
pub const SCHEDULE_SECTION: &str = "rk.schedule";

/// On-wire format version for [`ScheduleManifest`].
pub const SCHEDULE_VERSION: u16 = 1;

/// What runs, and when: policy as data, so a task keeps its name,
/// priority and RESOURCE. MessagePack encodes positionally: new fields go
/// at the tail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleManifest {
    pub version: u16,
    /// The base tick: the greatest common divisor of every task interval, in
    /// nanoseconds. Each task's period is a whole number of these.
    pub common_ticktime_ns: u64,
    /// Dispatch order — most urgent first. A consumer runs the due tasks in
    /// slice order and is correct without re-sorting.
    pub tasks: Vec<TaskEntry>,
}

/// One TASK: when it runs, and what it runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskEntry {
    pub name: String,
    /// The RESOURCE that declares it — the group a deployment binds to an
    /// execution unit.
    pub resource: String,
    /// Base ticks between runs: the task is due when `tick % period_ticks == 0`.
    pub period_ticks: u64,
    /// IEC priority, lower is more urgent. `None` when PRIORITY was omitted.
    pub priority: Option<u32>,
    /// Program instances, in declaration order.
    pub programs: Vec<ProgramEntry>,
}

/// One PROGRAM instance bound to a task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgramEntry {
    /// The instance name, as written in the configuration.
    pub instance: String,
    /// Exported function to call: the program type's body.
    pub export: String,
    /// Address of this instance's state, passed as the body's `this`; final,
    /// written after any retain relocation.
    pub instance_addr: u32,
}

impl ScheduleManifest {
    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("ScheduleManifest serialization should not fail")
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The v5 additions round-trip.
    #[test]
    fn symbols_with_types_round_trip() {
        let table = DebugSymbols {
            version: DEBUG_SYMBOLS_VERSION,
            symbols: vec![],
            arrays: vec![ArraySym {
                path: "P.pts".into(),
                address: 64,
                dimensions: vec![(0, 9)],
                total_elements: 10,
                elem_size: 8,
                elem_ty: None,
                global: false,
                elem_type: Some(1),
            }],
            types: vec![
                TypeDesc::Scalar(SymType::DInt),
                TypeDesc::Struct {
                    name: "Pt".into(),
                    size: 8,
                    fields: vec![
                        FieldDesc {
                            name: "x".into(),
                            offset: 0,
                            ty: 0,
                        },
                        FieldDesc {
                            name: "y".into(),
                            offset: 4,
                            ty: 0,
                        },
                    ],
                },
            ],
            containers: vec![ContainerSym {
                path: "P".into(),
                address: 64,
                global: false,
                type_name: "Prog".into(),
            }],
        };
        let back = DebugSymbols::from_msgpack(&table.to_msgpack()).unwrap();
        assert_eq!(back, table);
    }

    /// A v6 module and a v3 locals table must still decode: the encoding is
    /// positional.
    #[test]
    fn a_v6_symbols_and_v3_locals_payload_still_decode() {
        #[derive(Serialize)]
        struct DebugSymbolsV6 {
            version: u16,
            symbols: Vec<Symbol>,
            arrays: Vec<ArraySym>,
            types: Vec<TypeDesc>,
        }
        let v6 = DebugSymbolsV6 {
            version: 6,
            symbols: vec![Symbol {
                path: "P1.n".into(),
                address: 64,
                size: 4,
                ty: SymType::DInt,
                global: false,
                named_type: None,
            }],
            arrays: vec![],
            types: vec![],
        };
        let decoded = DebugSymbols::from_msgpack(&rmp_serde::to_vec(&v6).unwrap())
            .expect("a v6 payload still decodes");
        assert_eq!(decoded.version, 6);
        assert_eq!(decoded.symbols.len(), 1);
        assert!(
            decoded.containers.is_empty(),
            "and simply has no aggregates named"
        );

        #[derive(Serialize)]
        struct FuncLocalsV3 {
            defined_index: u32,
            locals: Vec<LocalVar>,
            memory: Vec<Symbol>,
            arrays: Vec<ArraySym>,
        }
        #[derive(Serialize)]
        struct DebugLocalsV3 {
            version: u16,
            functions: Vec<FuncLocalsV3>,
            types: Vec<TypeDesc>,
        }
        let v3 = DebugLocalsV3 {
            version: 3,
            functions: vec![FuncLocalsV3 {
                defined_index: 0,
                locals: vec![],
                memory: vec![],
                arrays: vec![],
            }],
            types: vec![],
        };
        let decoded = DebugLocals::from_msgpack(&rmp_serde::to_vec(&v3).unwrap())
            .expect("a v3 locals payload still decodes");
        assert_eq!(decoded.version, 3);
        assert_eq!(
            decoded.functions[0].this_slot, None,
            "and no frame claims an instance"
        );
    }

    /// A module built by a pre-v5 compiler must still be readable: a `.wasm`
    /// artifact outlives the toolchain. This constructs a real v4 payload
    /// from mirror structs, since a v5→v5 round-trip proves nothing.
    #[test]
    fn a_v4_payload_still_decodes_today() {
        /// A v4 `Symbol`: no `named_type`. Mirrored rather than reused, so
        /// this really encodes what a v4 producer wrote.
        #[derive(Serialize)]
        struct SymbolV4 {
            path: String,
            address: u32,
            size: u32,
            ty: SymType,
            global: bool,
        }
        #[derive(Serialize)]
        struct ArraySymV4 {
            path: String,
            address: u32,
            dimensions: Vec<(i64, i64)>,
            total_elements: u32,
            elem_size: u32,
            elem_ty: Option<SymType>,
            global: bool,
        }
        #[derive(Serialize)]
        struct DebugSymbolsV4 {
            version: u16,
            symbols: Vec<SymbolV4>,
            arrays: Vec<ArraySymV4>,
        }

        let v4 = DebugSymbolsV4 {
            version: 4,
            symbols: vec![SymbolV4 {
                path: "P.n".into(),
                address: 32,
                size: 4,
                ty: SymType::DInt,
                global: false,
            }],
            arrays: vec![ArraySymV4 {
                path: "P.a".into(),
                address: 64,
                dimensions: vec![(0, 9)],
                total_elements: 10,
                elem_size: 4,
                elem_ty: Some(SymType::DInt),
                global: true,
            }],
        };
        let bytes = rmp_serde::to_vec(&v4).expect("v4 encodes");

        let read = DebugSymbols::from_msgpack(&bytes).expect("a v4 payload must decode as v5");
        assert_eq!(read.version, 4, "the payload's own version is preserved");
        assert_eq!(read.symbols[0].path, "P.n");
        // Every pre-existing field keeps its value — a shifted field would
        // silently misread `global` as something else.
        let a = &read.arrays[0];
        assert_eq!(a.elem_ty, Some(SymType::DInt));
        assert!(a.global, "global must not be shifted into another field");
        // The v5 additions come back empty, which is exactly right: a v4
        // producer described no aggregate element layouts.
        assert_eq!(a.elem_type, None);
        assert!(read.types.is_empty());
    }
}
