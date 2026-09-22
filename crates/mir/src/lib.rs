pub mod debug_symbols;
pub mod expr;
pub mod function;
pub mod located;
pub mod memory;
pub mod retain_map;
pub mod schedule;
pub mod stmt;
/// The `{test}` manifest lives in `debug_format`, the schema crate hosts read.
pub use debug_format::test_manifest;
pub mod types;

pub mod lower;

use hir::hir_def::interned::identifier::Ident;
use rustc_hash::FxHashMap;

use crate::debug_symbols::DebugSymbols;
use crate::function::{MirExternFunction, MirFunction};
use crate::memory::MirMemoryLayout;
use crate::test_manifest::TestManifest;
use crate::types::MirType;

/// A fully lowered MIR module, ready for backend consumption.
#[derive(Debug, Clone)]
pub struct MirModule {
    /// All functions (including monomorphized copies and lowered methods).
    pub functions: Vec<MirFunction>,

    /// Extern function declarations (imports).
    pub extern_functions: Vec<MirExternFunction>,

    /// String literals interned during lowering.
    pub string_literals: Vec<MirStringLiteral>,

    /// Instance type layouts (FB and Class types used in the module).
    pub instance_types: Vec<MirInstanceType>,

    /// Function name → function index (imports first, then locals).
    pub function_indices: FxHashMap<Ident, u32>,

    /// Type name → type index.
    pub type_indices: FxHashMap<Ident, u32>,

    /// Memory layout — fully resolved.
    pub memory_layout: MirMemoryLayout,

    /// String data section entries: (offset, bytes).
    pub string_data: Vec<(u32, Vec<u8>)>,

    /// Test manifest: metadata about test functions.
    pub test_manifest: TestManifest,

    /// Start of the contiguous RETAIN band the host snapshots and restores;
    /// `retain_size == 0` without `RETAIN` variables.
    pub retain_base: u32,
    pub retain_size: u32,

    /// Start and byte length of the contiguous GLOBALS band: every configuration
    /// `VAR_GLOBAL` as one host-visible region. RETAIN globals sit in its
    /// overlap with the retain band.
    pub globals_base: u32,
    pub globals_size: u32,

    /// The three located (`AT %…`) bands: `%I` written by the host before a
    /// scan, `%Q` read back after it, `%M` the program's own marker area.
    /// Each is contiguous, so a host copies one range per direction; all
    /// three are zero-sized when the workspace declares no located variable.
    pub input_base: u32,
    pub input_size: u32,
    pub output_base: u32,
    pub output_size: u32,
    pub marker_base: u32,
    pub marker_size: u32,

    /// Resolved task schedule of the module's CONFIGURATION; `None` without one.
    pub schedule: Option<schedule::MirSchedule>,

    /// The schedule as the module carries it (the `rk.schedule` section),
    /// built once instance addresses are final.
    pub schedule_manifest: Option<debug_format::ScheduleManifest>,

    /// Debug-symbol table: every debuggable variable at its absolute address,
    /// emitted as the `debug-symbols` section.
    pub debug_symbols: DebugSymbols,

    /// Per-field RETAIN map: which byte ranges of the retain band persist
    /// (see `retain_map`), emitted as the `retain-map` section.
    pub retain_map: debug_format::RetainMap,

    /// Which located (`AT %…`) variable sits where inside the three bands,
    /// emitted as the `located-map` section. Empty when the workspace
    /// declares none, and the section is then not emitted at all.
    pub located_map: debug_format::LocatedMap,

    /// Source file URLs, indexed by `MirSourceLocation::file_id` and emitted
    /// as `DebugLines::files`.
    pub source_files: Vec<String>,
}

/// An interned string literal.
#[derive(Debug, Clone)]
pub struct MirStringLiteral {
    pub id: u32,
    pub bytes: Vec<u8>,
}

/// Instance type layout for a FUNCTION_BLOCK or CLASS.
/// Forms a recursive tree via `nested_instance` on fields.
#[derive(Debug, Clone)]
pub struct MirInstanceType {
    pub name: Ident,
    pub fields: Vec<MirInstanceField>,
    pub size: u32,
    pub align: u32,
}

/// A field within an instance type (FB/Class).
#[derive(Debug, Clone)]
pub struct MirInstanceField {
    pub name: Ident,
    pub ty: MirType,
    /// Byte offset within the instance.
    pub offset: u32,
    /// The instance type name when this field is itself an FB or class.
    pub nested_instance: Option<Ident>,
    /// Pre-computed initialization for this field.
    pub init: Option<MirFieldInit>,
}

/// Flat initialization data for an instance field.
#[derive(Debug, Clone)]
pub struct MirFieldInit {
    /// `(relative offset, value)` stores to run at initialization, nested FBs
    /// flattened.
    pub stores: Vec<MirInitStore>,
}

/// A single initialization store operation.
#[derive(Debug, Clone)]
pub struct MirInitStore {
    /// Relative offset from the field's base address.
    pub offset: u32,
    pub value: expr::MirConstant,
}

impl MirModule {
    /// Look up an instance type by name.
    pub fn find_instance_type(&self, name: Ident) -> Option<&MirInstanceType> {
        self.instance_types.iter().find(|it| it.name == name)
    }
}
