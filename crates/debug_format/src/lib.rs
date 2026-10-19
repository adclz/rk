//! The on-wire debug-info format shared by the compiler, which produces it,
//! and the tools that consume it: pure serde data, carried in the core
//! module as MessagePack-encoded custom sections.

use serde::{Deserialize, Serialize};

pub mod info;
pub mod test_manifest;
pub mod test_report;
mod tolerant;

pub use info::{
    DebugInfo, Live, Place, Pointee, SourcePos, StackFrame, TypeMismatch, VarLoc, VarValue, decode,
    encode,
};
/// Decode a section's MessagePack as every `from_msgpack` here does: the
/// fields `T` declares, and whatever a newer producer wrote after them
/// skipped. For a reader of an older shape, or a mirror of one in a test.
pub use tolerant::from_slice as read_tolerant;

/// Custom wasm section carrying the MessagePack-encoded [`DebugSymbols`].
pub const DEBUG_SYMBOLS_SECTION: &str = "debug-symbols";

/// On-wire format version; bump on any breaking change. A new field goes
/// at the TAIL of its struct: `rmp_serde` encodes positionally, so
/// `#[serde(default)]` only backfills a field missing from the end, and an
/// older reader skips the fields past those it knows ([`tolerant`]). An enum
/// variant is written by its name: its place in the enum is free, and a new
/// one is what an older reader cannot step over.
pub const DEBUG_SYMBOLS_VERSION: u16 = 10;

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
    /// Every pointer in memory, by path (v9), sorted by `path`: what a
    /// `REF_TO` variable or member holds, and what a VAR_IN_OUT member is
    /// bound to. They are not in `symbols`, a pointer having no [`SymType`].
    #[serde(default)]
    pub pointers: Vec<PointerSym>,
    /// `[base, end)` of the stack the frames of recursive calls are pushed
    /// on (v9), `None` when no function pushes one: an address in it is in
    /// the frame of some call, which only the live frames name.
    #[serde(default)]
    pub stack: Option<(u32, u32)>,
}

/// A pointer in linear memory: 4 bytes holding an address, or 0 for none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointerSym {
    /// Dotted path of the pointer itself, as [`Symbol::path`] spells it.
    pub path: String,
    /// Where the pointer is: an address in linear memory, or, in a
    /// [`FrameLocals`], bytes from the start of the frame.
    pub address: u32,
    /// Whether it lives in the configuration's globals.
    pub global: bool,
    /// What it points at, in the type table of its section. `None` when the
    /// compiler has no type for it.
    pub target: Option<TypeId>,
    /// A VAR_IN_OUT member: its name reads as the variable it is bound to,
    /// where a `REF_TO` is followed by writing `^`.
    pub by_ref: bool,
    /// An `ARRAY[*]` VAR_IN_OUT member (v10), which has no `target`: what
    /// it says of the array it is bound to. Last on purpose, and left out
    /// when unset, so every other pointer encodes as in v9.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conformand: Option<Conformand>,
}

/// What an `ARRAY[*]` says of the array it is bound to: how many dimensions
/// it has, and its element. The bounds are that array's. The instance keeps
/// the address alone, each copy of the body having them as constants, and
/// an address does not say which array starts there: a matrix and its
/// first row start together, and so do an array of STRUCTs and the array
/// its first element begins with. The rank and the element do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conformand {
    /// One per `*`.
    pub rank: u32,
    /// The element, in the type table of the section.
    pub elem: TypeId,
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

/// Which type table a [`TypeRef`] indexes: the module's
/// ([`DebugSymbols::types`]) or the frames' ([`DebugLocals::types`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Table {
    Module,
    Frames,
}

/// A type by its table and id: what a resolved location carries when its
/// entry says more than its [`SymType`] does, an enumeration's variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeRef {
    pub table: Table,
    pub id: TypeId,
}

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
    /// A pointer when `size` is 4: locating through one needs a live
    /// dereference, a read of the address it holds.
    Opaque {
        size: u32,
        /// What it points at (v9); `None` when the compiler has no type
        /// for it.
        #[serde(default)]
        target: Option<TypeId>,
        /// A VAR_IN_OUT member (v9), read through without `^`.
        #[serde(default)]
        by_ref: bool,
        /// An `ARRAY[*]` VAR_IN_OUT member (v10), as
        /// [`PointerSym::conformand`]: left out when unset.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        conformand: Option<Conformand>,
    },
    /// An IEC enumeration: stored as `storage`, displayed as a variant name.
    Enum {
        /// The IEC type name (`TrafficLight`).
        name: String,
        storage: SymType,
        /// Variants in declaration order, with their integer values.
        variants: Vec<(String, i64)>,
    },
}

/// An array element's IEC-subscripted path from its flat row-major index,
/// `a[1, 2]` as the standard spells it; owned here so emitter and consumers
/// agree. A reader takes the chained `a[1][2]` as well.
pub fn element_path(path: &str, flat: u32, dimensions: &[(i64, i64)]) -> String {
    if dimensions.is_empty() {
        return format!("{path}[{flat}]");
    }
    let mut subs = vec![0i64; dimensions.len()];
    let mut rem = i64::from(flat);
    for d in (0..dimensions.len()).rev() {
        let (lo, hi) = dimensions[d];
        // A bound a hostile table wrote can span the whole of i64: the flat
        // index is then all there is to say.
        let Some(size) = hi.checked_sub(lo).and_then(|n| n.checked_add(1)) else {
            return format!("{path}[{flat}]");
        };
        let size = size.max(1);
        let Some(sub) = lo.checked_add(rem % size) else {
            return format!("{path}[{flat}]");
        };
        subs[d] = sub;
        rem /= size;
    }
    let joined = subs
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
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
    /// enumeration).
    #[serde(default)]
    pub named_type: Option<TypeId>,
    /// Set when the value is bits of the one at `address` rather than bytes
    /// of its own (v8): a part of a wider located address. Last on purpose,
    /// and left out when unset, so every other symbol encodes as in v7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bits: Option<SymBits>,
}

/// Where a value that is part of a wider one sits in it: `width` bits from
/// bit `shift` of the little-endian value at the symbol's `address`, which is
/// `size` bytes. `%IX1.2` beside a `%IW0` is bit 10 of that word's cell. It
/// is read by shifting and masking, and written by replacing those bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymBits {
    pub shift: u16,
    pub width: u16,
}

impl SymBits {
    fn mask(self) -> u64 {
        if self.width >= 64 {
            u64::MAX
        } else {
            (1 << self.width) - 1
        }
    }

    /// The part, taken out of `whole`'s bytes: a value's own bytes,
    /// zero-extended, which [`decode`](crate::decode) reads as any other.
    /// A shift past the value, which a module may say, is a part of
    /// nothing: zero.
    pub fn extract(self, whole: &[u8]) -> [u8; 8] {
        if self.shift >= 64 {
            return [0; 8];
        }
        ((widen(whole) >> self.shift) & self.mask()).to_le_bytes()
    }

    /// `whole` with the part's bits replaced by the low bits of `part`, a
    /// value's own bytes; the rest of `whole` is kept, all of it for a
    /// shift past the value.
    pub fn replace(self, whole: &[u8], part: &[u8]) -> Vec<u8> {
        if self.shift >= 64 {
            return whole[..whole.len().min(8)].to_vec();
        }
        let mask = self.mask() << self.shift;
        let value = (widen(whole) & !mask) | ((widen(part) << self.shift) & mask);
        value.to_le_bytes()[..whole.len().min(8)].to_vec()
    }
}

/// Up to eight little-endian bytes as one value.
fn widen(bytes: &[u8]) -> u64 {
    let mut buf = [0u8; 8];
    let n = bytes.len().min(8);
    buf[..n].copy_from_slice(&bytes[..n]);
    u64::from_le_bytes(buf)
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
            pointers: Vec::new(),
            stack: None,
        }
    }

    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("DebugSymbols serialization should not fail")
    }

    /// Deserialize from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        crate::tolerant::from_slice(bytes)
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
        crate::tolerant::from_slice(bytes)
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
        crate::tolerant::from_slice(bytes)
    }
}

// ---------------------------------------------------------------------------
// Locals — labeling a frame's wasm local slots with IEC names/types
// ---------------------------------------------------------------------------

/// Custom wasm section carrying the MessagePack-encoded [`DebugLocals`].
pub const DEBUG_LOCALS_SECTION: &str = "debug-locals";

/// On-wire format version for [`DebugLocals`]. v2 adds `memory`/`arrays`
/// per function; v3 adds the module-level `types` table; v6 lets a pointer
/// of that table say what an `ARRAY[*]` is bound to ([`Conformand`]).
pub const DEBUG_LOCALS_VERSION: u16 = 6;

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
    /// static addresses. A recursive function's are in the frame of each
    /// call, and not listed.
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
    /// The pointers among the memory locals above (v5), at static addresses,
    /// as [`DebugSymbols::pointers`].
    #[serde(default)]
    pub pointers: Vec<PointerSym>,
    /// The pointers held in wasm locals (v5): a VAR_IN_OUT or VAR_OUTPUT
    /// parameter, a `REF_TO` input or local.
    #[serde(default)]
    pub local_pointers: Vec<LocalPointer>,
    /// The memory locals of a recursive function (v5), which are in the
    /// frame of each call.
    #[serde(default)]
    pub frame: Option<FrameLocals>,
    /// Every aggregate among the memory locals above, by its base (v5), as
    /// [`DebugSymbols::containers`]: what a reference into this function's
    /// locals points at, when it points at a whole STRUCT.
    #[serde(default)]
    pub containers: Vec<ContainerSym>,
}

/// A pointer in a wasm local: an i32 holding an address, or 0 for none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalPointer {
    /// Index for `FrameHandle::local(i)`.
    pub wasm_index: u32,
    pub name: String,
    /// What it points at, in [`DebugLocals::types`].
    pub target: Option<TypeId>,
    /// A VAR_IN_OUT or VAR_OUTPUT parameter: its name reads as the caller's
    /// variable, where a `REF_TO` is followed by writing `^`.
    pub by_ref: bool,
}

/// The memory locals of a recursive function. Each call pushes a frame on
/// the stack and keeps where it starts in a wasm local: a local lives at
/// that address plus its offset, in the frame of the call being looked at.
///
/// They are not in [`FuncLocals::memory`], whose addresses are absolute: a
/// reader that took an offset for an address would show memory that is
/// something else's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameLocals {
    /// The wasm local holding the address the frame starts at.
    pub slot: u32,
    /// As [`FuncLocals::memory`], each `address` counted from the frame's
    /// start.
    pub memory: Vec<Symbol>,
    /// As [`FuncLocals::arrays`], likewise.
    pub arrays: Vec<ArraySym>,
    /// As [`FuncLocals::pointers`], likewise.
    pub pointers: Vec<PointerSym>,
    /// Bytes a call pushes: `[start, start + size)` is this call's, and an
    /// address in it is one of these locals'.
    #[serde(default)]
    pub size: u32,
    /// As [`FuncLocals::containers`], each `address` counted from the
    /// frame's start.
    #[serde(default)]
    pub containers: Vec<ContainerSym>,
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
        crate::tolerant::from_slice(bytes)
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
        crate::tolerant::from_slice(bytes)
    }
}

// ---------------------------------------------------------------------------
// Retain map — per-field RETAIN persistence ranges
// ---------------------------------------------------------------------------

/// Custom wasm section carrying the [`RetainMap`]. Load-bearing for IEC
/// semantics: it must survive release optimization.
pub const RETAIN_MAP_SECTION: &str = "retain-map";

/// On-wire format version for [`RetainMap`]. 3: the ranges are sorted and
/// hashed by their case-folded path, so a case-only rename is the same
/// layout.
pub const RETAIN_MAP_VERSION: u16 = 3;

/// The retained byte ranges of a module, inside the retain band;
/// everything in the band not covered is transient and keeps its
/// `__init` cold-start value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetainMap {
    pub version: u16,
    /// Layout identity: FNV-1a over the sorted `(path, size, type_key)`
    /// sequence, the path case-folded as IEC 61131-3 §6.1.2 matches names,
    /// excluding addresses so the band may re-base between builds.
    /// `type_key` is included because size alone does not identify a value.
    pub layout_hash: u64,
    /// Retained ranges, sorted by case-folded `path` (deterministic; file
    /// payload order).
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
    /// Build from ranges: sorts by folded path and stamps the layout hash.
    pub fn new(mut ranges: Vec<RetainRange>) -> Self {
        ranges.sort_by_cached_key(|r| info::fold_path(&r.path));
        let layout_hash = Self::hash_layout(&ranges);
        RetainMap {
            version: RETAIN_MAP_VERSION,
            layout_hash,
            ranges,
        }
    }

    /// FNV-1a over the sorted `(folded path, size, type_key)` sequence.
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
            eat(info::fold_path(&r.path).as_bytes());
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
        crate::tolerant::from_slice(bytes)
    }
}

// ---------------------------------------------------------------------------
// Located map — which address is which cell
// ---------------------------------------------------------------------------

/// Custom wasm section carrying the [`LocatedMap`]. Load-bearing: the band
/// exports say where the three areas are, and this says which variable sits
/// where inside them. Without it a host can copy an image in and out but
/// cannot bind a single channel.
pub const LOCATED_MAP_SECTION: &str = "located-map";

/// On-wire format version for [`LocatedMap`].
pub const LOCATED_MAP_VERSION: u16 = 2;

/// The area an address names. Each is one contiguous band, exported as
/// `input_base`/`input_size` and its two siblings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LocatedArea {
    /// `%I` — the host writes it before a scan; the program only reads it.
    Input,
    /// `%Q` — the program writes it; the host reads it after a scan.
    Output,
    /// `%M` — the marker area, which the program owns outright.
    Marker,
}

impl LocatedArea {
    /// The prefix as written.
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Input => "%I",
            Self::Output => "%Q",
            Self::Marker => "%M",
        }
    }
}

/// Every located (`AT %…`) variable of a module, with the address it is bound
/// to and where that lands in linear memory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocatedMap {
    pub version: u16,
    /// Layout identity: FNV-1a over the sorted `(address, name, size)`
    /// sequence, excluding linear-memory addresses so a band may re-base
    /// between builds without invalidating a host's bindings. A host compares
    /// it to know the set of addresses it bound is still this module's.
    pub layout_hash: u64,
    /// The located variables, sorted by `address` (deterministic; file
    /// payload order).
    pub entries: Vec<LocatedVar>,
}

/// One located variable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocatedVar {
    /// The address as written: `%IX0.0`, `%MW1.7.9`. This is the key a host
    /// binds a channel to; rk gives its numeric levels no meaning of their
    /// own, so two addresses name the same cell only if they are equal.
    pub address: String,
    /// The name the program calls it, which is what the debug symbols list
    /// it under.
    pub name: String,
    pub area: LocatedArea,
    /// The numeric levels of the address, in order — `%MW1.7.9` is
    /// `[1, 7, 9]`. Carried decoded so a host need not parse `address`.
    pub path: Vec<u32>,
    /// What the size letter names, in bits: 1 (`X`), 8 (`B`), 16 (`W`),
    /// 32 (`D`), 64 (`L`).
    pub width: u16,
    /// Absolute address in linear memory, inside this entry's band: where
    /// the cell is, the owner's for a part.
    pub addr: u32,
    /// Bytes the cell occupies there. This is its DECLARED type's size, not
    /// `width / 8`: `sensor AT %IX0.0 : BOOL` names one bit and occupies four
    /// bytes. For a part it is the owner's cell.
    pub size: u32,
    /// The declared type, when it is a scalar the host can decode on its own;
    /// `None` for an aggregate, which the debug symbols describe field by
    /// field instead. For a part, the type its own bits read as.
    pub ty: Option<SymType>,
    /// Set when the address is part of a wider one the module also names:
    /// `%IX0.3` beside a `%IW0` is bit 3 of that word's cell, not a cell of
    /// its own. A host reads it as `(cell >> shift) & mask(width)` and writes
    /// it by replacing those bits.
    pub part_of: Option<LocatedPart>,
}

/// Where a part sits in the address that owns its storage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocatedPart {
    /// The owner's address, as its own entry lists it.
    pub owner: String,
    /// The part's low bit in the owner's value. Each size counts in its own
    /// units and the image is little-endian, so `%IB1` is bits 8 to 15 of
    /// `%IW0` and `%IX1.2` is bit 10.
    pub shift: u16,
}

impl LocatedMap {
    /// Build from entries: sorts by address and stamps the layout hash.
    pub fn new(mut entries: Vec<LocatedVar>) -> Self {
        entries.sort_by(|a, b| a.address.cmp(&b.address));
        let layout_hash = Self::hash_layout(&entries);
        LocatedMap {
            version: LOCATED_MAP_VERSION,
            layout_hash,
            entries,
        }
    }

    /// FNV-1a over the sorted `(address, folded name, size)` sequence.
    fn hash_layout(entries: &[LocatedVar]) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        let mut h = FNV_OFFSET;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h ^= b as u64;
                h = h.wrapping_mul(FNV_PRIME);
            }
        };
        for e in entries {
            eat(e.address.as_bytes());
            eat(&[0]); // separator
            eat(info::fold_path(&e.name).as_bytes());
            eat(&[0]);
            eat(&e.size.to_le_bytes());
            // An address that becomes part of a wider one is read another
            // way, so a binding made against the old layout is stale.
            if let Some(part) = &e.part_of {
                eat(part.owner.as_bytes());
                eat(&[0]);
                eat(&part.shift.to_le_bytes());
            }
        }
        h
    }

    /// The entries of one area, in address order.
    pub fn area(&self, area: LocatedArea) -> impl Iterator<Item = &LocatedVar> {
        self.entries.iter().filter(move |e| e.area == area)
    }

    /// Serialize to MessagePack bytes.
    pub fn to_msgpack(&self) -> Vec<u8> {
        rmp_serde::to_vec(self).expect("LocatedMap serialization should not fail")
    }

    /// Deserialize from MessagePack bytes.
    pub fn from_msgpack(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
        crate::tolerant::from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The located map round-trips, and its hash covers the addresses rather
    /// than where they landed: re-basing a band must not invalidate a host's
    /// bindings, adding an address must.
    #[test]
    fn located_map_round_trips_and_hashes_its_addresses() {
        let var = |address: &str, name: &str, area, addr| LocatedVar {
            address: address.into(),
            name: name.into(),
            area,
            path: vec![0, 0],
            width: 1,
            addr,
            size: 4,
            ty: Some(SymType::Bool),
            part_of: None,
        };
        let map = LocatedMap::new(vec![
            var("%QX0.0", "lamp", LocatedArea::Output, 128),
            var("%IX0.0", "start", LocatedArea::Input, 64),
        ]);
        assert_eq!(
            map.entries.iter().map(|e| &e.address).collect::<Vec<_>>(),
            ["%IX0.0", "%QX0.0"],
            "sorted by address, whatever order they arrived in"
        );
        assert_eq!(LocatedMap::from_msgpack(&map.to_msgpack()).unwrap(), map);

        let rebased = LocatedMap::new(vec![
            var("%IX0.0", "start", LocatedArea::Input, 4096),
            var("%QX0.0", "lamp", LocatedArea::Output, 8192),
        ]);
        assert_eq!(map.layout_hash, rebased.layout_hash);

        let mut grown = map.entries.clone();
        grown.push(var("%IX0.1", "stop", LocatedArea::Input, 68));
        assert_ne!(map.layout_hash, LocatedMap::new(grown).layout_hash);
    }

    /// An address that becomes part of a wider one is read another way, so
    /// the same entry with an owner, or another shift in it, is another
    /// layout.
    #[test]
    fn an_owner_is_part_of_the_layout() {
        let word = |part_of| LocatedVar {
            address: "%IW0".into(),
            name: "dial".into(),
            area: LocatedArea::Input,
            path: vec![0],
            width: 16,
            addr: 64,
            size: 4,
            ty: Some(SymType::Int),
            part_of,
        };
        let owned = |shift| {
            Some(LocatedPart {
                owner: "%ID0".into(),
                shift,
            })
        };
        let alone = LocatedMap::new(vec![word(None)]).layout_hash;
        let low = LocatedMap::new(vec![word(owned(0))]).layout_hash;
        let high = LocatedMap::new(vec![word(owned(16))]).layout_hash;
        assert_ne!(alone, low, "an owner");
        assert_ne!(low, high, "another place in it");
    }

    /// A symbol without bits encodes exactly as a v7 one did; a part's
    /// round-trips, and reads and writes only its own bits.
    #[test]
    fn a_part_symbol_round_trips_and_others_encode_as_before() {
        #[derive(Serialize)]
        struct SymbolV7 {
            path: String,
            address: u32,
            size: u32,
            ty: SymType,
            global: bool,
            named_type: Option<TypeId>,
        }
        let plain = Symbol {
            path: "status".into(),
            address: 64,
            size: 4,
            ty: SymType::Word,
            global: true,
            named_type: None,
            bits: None,
        };
        let v7 = SymbolV7 {
            path: "status".into(),
            address: 64,
            size: 4,
            ty: SymType::Word,
            global: true,
            named_type: None,
        };
        assert_eq!(
            rmp_serde::to_vec(&plain).unwrap(),
            rmp_serde::to_vec(&v7).unwrap()
        );

        let bits = SymBits { shift: 8, width: 8 };
        let part = Symbol {
            path: "level".into(),
            ty: SymType::SInt,
            bits: Some(bits),
            ..plain
        };
        let back: Symbol = rmp_serde::from_slice(&rmp_serde::to_vec(&part).unwrap()).unwrap();
        assert_eq!(back, part);

        let cell = 0x0000_8001u32.to_le_bytes();
        assert_eq!(
            decode(SymType::SInt, &bits.extract(&cell)),
            VarValue::I8(-128)
        );
        let forced = bits.replace(&cell, &encode(SymType::SInt, VarValue::I8(5)).unwrap());
        assert_eq!(forced, 0x0000_0501u32.to_le_bytes(), "the low byte kept");

        // A module may say any shift: past the value the part is nothing,
        // read as zero and written as no change, where a shift by 64 or
        // more overflowed.
        for past in [
            SymBits {
                shift: 64,
                width: 8,
            },
            SymBits {
                shift: u16::MAX,
                width: 1,
            },
        ] {
            assert_eq!(past.extract(&cell), [0; 8]);
            assert_eq!(past.replace(&cell, &[5]), cell.to_vec());
        }
    }

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
            pointers: vec![],
            stack: Some((4096, 8192)),
        };
        let back = DebugSymbols::from_msgpack(&table.to_msgpack()).unwrap();
        assert_eq!(back, table);
    }

    /// A reader of the formats before this one reads a table of this one:
    /// what it knows, with the pointers and the frames it does not skipped.
    /// What a runtime on a device does with a newer program.
    #[test]
    fn an_older_reader_reads_the_newer_tables() {
        /// `debug-symbols` as v8 declared it, and `debug-locals` as v4 did.
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct V8Symbols {
            version: u16,
            symbols: Vec<Symbol>,
            #[serde(default)]
            arrays: Vec<ArraySym>,
            #[serde(default)]
            types: Vec<V8Type>,
            #[serde(default)]
            containers: Vec<ContainerSym>,
        }
        #[derive(Deserialize, Debug)]
        #[allow(dead_code)]
        enum V8Type {
            Scalar(SymType),
            Struct {
                name: String,
                size: u32,
                fields: Vec<FieldDesc>,
            },
            Array {
                dimensions: Vec<(i64, i64)>,
                total_elements: u32,
                elem_size: u32,
                elem: TypeId,
            },
            Opaque {
                size: u32,
            },
            Enum {
                name: String,
                storage: SymType,
                variants: Vec<(String, i64)>,
            },
        }
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct V4Locals {
            version: u16,
            functions: Vec<V4Func>,
            #[serde(default)]
            types: Vec<V8Type>,
        }
        #[derive(Deserialize)]
        #[allow(dead_code)]
        struct V4Func {
            defined_index: u32,
            locals: Vec<LocalVar>,
            #[serde(default)]
            memory: Vec<Symbol>,
            #[serde(default)]
            arrays: Vec<ArraySym>,
            #[serde(default)]
            this_slot: Option<u32>,
        }

        let leaf = Symbol {
            path: "P1.n".into(),
            address: 8,
            size: 4,
            ty: SymType::Int,
            global: false,
            named_type: None,
            bits: None,
        };
        let mut table = DebugSymbols::new();
        table.symbols = vec![leaf.clone()];
        let of_ints = Conformand { rank: 2, elem: 0 };
        table.types = vec![
            TypeDesc::Scalar(SymType::Int),
            TypeDesc::Opaque {
                size: 4,
                target: Some(0),
                by_ref: true,
                conformand: None,
            },
            TypeDesc::Opaque {
                size: 4,
                target: None,
                by_ref: true,
                conformand: Some(of_ints),
            },
        ];
        table.pointers = vec![
            PointerSym {
                path: "P1.m.speed".into(),
                address: 12,
                global: false,
                target: Some(0),
                by_ref: true,
                conformand: None,
            },
            PointerSym {
                path: "P1.w.a".into(),
                address: 16,
                global: false,
                target: None,
                by_ref: true,
                conformand: Some(of_ints),
            },
        ];
        let back = DebugSymbols::from_msgpack(&table.to_msgpack()).expect("its own reader");
        assert_eq!(back, table, "what an ARRAY[*] says comes back");
        let read: V8Symbols = crate::tolerant::from_slice(&table.to_msgpack())
            .expect("the v8 reader reads a v10 table");
        assert_eq!(read.version, DEBUG_SYMBOLS_VERSION);
        assert_eq!(read.symbols, vec![leaf.clone()]);
        assert!(
            matches!(read.types[1], V8Type::Opaque { size: 4 }),
            "the pointer's entry, its target skipped: {:?}",
            read.types
        );

        let locals = DebugLocals {
            version: DEBUG_LOCALS_VERSION,
            functions: vec![FuncLocals {
                defined_index: 3,
                locals: vec![LocalVar {
                    wasm_index: 0,
                    name: "n".into(),
                    ty: SymType::Int,
                }],
                memory: Vec::new(),
                arrays: Vec::new(),
                this_slot: Some(1),
                pointers: Vec::new(),
                local_pointers: vec![LocalPointer {
                    wasm_index: 4,
                    name: "v".into(),
                    target: Some(0),
                    by_ref: true,
                }],
                frame: Some(FrameLocals {
                    slot: 2,
                    memory: vec![leaf],
                    arrays: Vec::new(),
                    pointers: Vec::new(),
                    size: 16,
                    containers: Vec::new(),
                }),
                containers: Vec::new(),
            }],
            types: vec![TypeDesc::Scalar(SymType::Int)],
        };
        let read: V4Locals = crate::tolerant::from_slice(&locals.to_msgpack())
            .expect("the v4 reader reads a v6 table");
        assert_eq!(read.version, DEBUG_LOCALS_VERSION);
        assert_eq!(read.functions[0].this_slot, Some(1));
        assert_eq!(read.functions[0].locals[0].name, "n");
        assert!(
            read.functions[0].memory.is_empty(),
            "a frame's locals are not taken for static ones"
        );
    }

    /// What an `ARRAY[*]` says is the last field of a pointer and is left
    /// out when there is nothing to say: every other pointer, and every
    /// other pointer's type, is the bytes v9 wrote.
    #[test]
    fn a_pointer_that_is_no_conformand_encodes_as_before() {
        #[derive(Serialize)]
        struct PointerV9 {
            path: String,
            address: u32,
            global: bool,
            target: Option<TypeId>,
            by_ref: bool,
        }
        #[derive(Serialize)]
        enum TypeV9 {
            Opaque {
                size: u32,
                target: Option<TypeId>,
                by_ref: bool,
            },
        }
        let pointer = PointerSym {
            path: "P1.m.speed".into(),
            address: 12,
            global: false,
            target: Some(0),
            by_ref: true,
            conformand: None,
        };
        let v9 = PointerV9 {
            path: "P1.m.speed".into(),
            address: 12,
            global: false,
            target: Some(0),
            by_ref: true,
        };
        assert_eq!(
            rmp_serde::to_vec(&pointer).unwrap(),
            rmp_serde::to_vec(&v9).unwrap()
        );
        let ty = TypeDesc::Opaque {
            size: 4,
            target: Some(0),
            by_ref: true,
            conformand: None,
        };
        let v9 = TypeV9::Opaque {
            size: 4,
            target: Some(0),
            by_ref: true,
        };
        assert_eq!(
            rmp_serde::to_vec(&ty).unwrap(),
            rmp_serde::to_vec(&v9).unwrap()
        );
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
                bits: None,
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
