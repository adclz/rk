//! The debug view of a compiled module: symbol table, line tables and
//! frame-local tables, read from the `debug-*` sections and queried by
//! path, address or wasm program counter. Pure data over the module's
//! bytes; a stripped binary yields an empty [`DebugInfo`].

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{SymType, Symbol};

/// A decoded variable value, as read from (or forced into) the PLC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VarValue {
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    /// A decoded IEC STRING — the `len`-prefixed buffer, as UTF-8.
    String(String),
    /// An enumeration value: its type, the variant it names, and the raw
    /// integer for when it names none.
    Enum {
        type_name: String,
        /// `None` when the stored integer matches no declared variant.
        variant: Option<String>,
        raw: i64,
    },
    /// The variable exists but its value could not be read, with the reason;
    /// a dropped row would read as "no such variable". Kept last: `rmp_serde`
    /// encodes an enum by variant index.
    Unavailable(String),
}

/// A resolved source position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourcePos {
    /// Index into the module's source-file table.
    pub file: u32,
    /// 0-based source line.
    pub line: u32,
    /// 0-based source column.
    pub col: u32,
}

/// One frame of the source-level stack at a stop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackFrame {
    pub defined_index: u32,
    pub function: Option<String>,
    pub source: Option<SourcePos>,
    /// Whose state this frame is running on: the program or FB instance it
    /// was called with (`Run`, `Run.motor`), resolved from the `this` pointer
    /// through the container table. `None` for a FUNCTION.
    #[serde(default)]
    pub instance: Option<String>,
}

/// A value offered for a variable whose type cannot hold it.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeMismatch {
    pub ty: SymType,
    pub value: VarValue,
}

impl std::fmt::Display for TypeMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "value {:?} does not match variable type {:?}",
            self.value, self.ty
        )
    }
}

impl std::error::Error for TypeMismatch {}

/// A symbol path as it is looked up: case-folded (IEC 61131-3 6.1.2),
/// matching the front end's fold.
fn fold_path(path: &str) -> String {
    path.to_lowercase()
}

/// Where one variable's value lives and how to decode it, cached behind a
/// monitoring handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VarLoc {
    /// Absolute address in linear memory.
    pub address: u32,
    /// Width in bytes.
    pub size: u32,
    /// Declared IEC type, for decoding the bytes.
    pub ty: SymType,
    /// Whether this is a configuration `VAR_GLOBAL` (vs program-local state).
    pub global: bool,
}

/// Parsed debug information for a compiled module; empty when the binary
/// carries none.
#[derive(Debug, Clone, Default)]
pub struct DebugInfo {
    symbols: Vec<Symbol>,
    symbol_index: HashMap<String, usize>,
    /// Array descriptors (v4): shape once, elements computed on demand,
    /// consulted when a path misses the leaf table.
    arrays: Vec<crate::ArraySym>,
    array_index: HashMap<String, usize>,
    /// The type table (v5): aggregate layouts walked by
    /// [`locate`](Self::locate) for members of an element never enumerated.
    types: Vec<crate::TypeDesc>,
    /// Aggregate base addresses with what each is an instance OF
    /// (`debug-symbols` v7).
    containers: Vec<crate::ContainerSym>,
    /// `DefinedFuncIndex` → IEC function name (from `debug-functions`), for
    /// naming wasm stack frames.
    function_names: HashMap<u32, String>,
    /// `DefinedFuncIndex` → its line table (from `debug-lines`), sorted by
    /// within-body offset.
    line_tables: HashMap<u32, Vec<crate::LineEntry>>,
    /// `DefinedFuncIndex` → the function body's start offset in the binary,
    /// for converting an absolute `wasm_pc` to a within-body offset.
    body_starts: Vec<u32>,
    /// Source file paths, indexed by `LineEntry::file`.
    source_files: Vec<String>,
    /// Per-function frame-local tables (`debug-locals`), keyed by
    /// `DefinedFuncIndex`.
    frame_locals: HashMap<u32, crate::FuncLocals>,
    /// Why a debug section that is present could not be used: a decode
    /// failure or an unknown version. The realistic cause is a module built
    /// by a newer compiler, whose extra trailing fields positional
    /// MessagePack rejects.
    problems: Vec<String>,
}

impl DebugInfo {
    /// Parse the debug sections out of a compiled core module's bytes. Returns
    /// an empty view for a stripped or hand-written module.
    pub fn from_wasm(wasm: &[u8]) -> Self {
        let mut problems = Vec::new();
        let (symbols, arrays, types, containers) = read_debug_symbols(wasm, &mut problems);
        // Present but pre-typing: nested instances that share a base address
        // cannot be told apart, so a frame may be attributed to the outer one.
        if containers.iter().any(|c| c.type_name.is_empty()) {
            problems.push(format!(
                "`{}` names instances without their type; a frame on a block \
                 nested at offset 0 may show the enclosing instance's state. \
                 Rebuild the program with this toolchain.",
                crate::DEBUG_SYMBOLS_SECTION
            ));
        }
        // Keyed by the folded path (IEC identifiers are not case sensitive); the
        // symbols keep their spelling for display.
        let symbol_index = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (fold_path(&s.path), i))
            .collect();
        let array_index = arrays
            .iter()
            .enumerate()
            .map(|(i, a)| (fold_path(&a.path), i))
            .collect();
        let function_names = read_function_names(wasm);
        let (source_files, line_tables) = read_debug_lines(wasm);
        let body_starts = read_body_starts(wasm);
        let frame_locals = read_debug_locals(wasm);
        DebugInfo {
            symbols,
            symbol_index,
            arrays,
            array_index,
            types,
            containers,
            frame_locals,
            function_names,
            line_tables,
            body_starts,
            source_files,
            problems,
        }
    }

    /// A symbols-only view, for exercising path resolution without a module.
    pub fn from_symbols(symbols: Vec<Symbol>) -> Self {
        let symbol_index = symbols
            .iter()
            .enumerate()
            .map(|(i, s)| (fold_path(&s.path), i))
            .collect();
        DebugInfo {
            symbols,
            symbol_index,
            arrays: Vec::new(),
            array_index: HashMap::new(),
            types: Vec::new(),
            containers: Vec::new(),
            frame_locals: HashMap::new(),
            function_names: HashMap::new(),
            line_tables: HashMap::new(),
            body_starts: Vec::new(),
            source_files: Vec::new(),
            problems: Vec::new(),
        }
    }

    /// The instance of `pou` based at `address`. Both are needed: an
    /// aggregate whose first field is an aggregate shares its base, so only
    /// the running code separates them. `None` for a FUNCTION.
    pub fn container_at(&self, address: u32, pou: &str) -> Option<&str> {
        if let Some(c) = self
            .containers
            .iter()
            .find(|c| c.address == address && c.type_name == pou)
        {
            return Some(c.path.as_str());
        }
        // A table written before instances carried their type has empty names;
        // degrade to the outermost name at that address, and `problems` says
        // the build is stale.
        self.containers
            .iter()
            .filter(|c| c.address == address && c.type_name.is_empty())
            .min_by_key(|c| (c.path.matches('.').count(), c.path.len()))
            .map(|c| c.path.as_str())
    }

    pub fn frame_locals(&self, defined_index: u32) -> Option<&crate::FuncLocals> {
        self.frame_locals.get(&defined_index)
    }

    /// Debug sections that are present but unusable, one message each; empty
    /// when the binary is simply stripped.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// All debuggable variables (configuration globals + program-instance
    /// fields), sorted by qualified path.
    pub fn list_symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    /// Metadata for one variable by qualified path (e.g. `Main.motor.speed`).
    pub fn symbol(&self, path: &str) -> Option<&Symbol> {
        self.symbol_index
            .get(&fold_path(path))
            .map(|&i| &self.symbols[i])
    }

    /// The IEC name of a defined wasm function by its `DefinedFuncIndex`, as
    /// wasmtime's `FrameHandle` reports it; `None` for an import, a builtin,
    /// or a stripped module.
    pub fn function_name(&self, defined_index: u32) -> Option<&str> {
        self.function_names.get(&defined_index).map(String::as_str)
    }

    /// How many leaves the emitter enumerated.
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }

    /// Every array descriptor in the module.
    pub fn arrays(&self) -> &[crate::ArraySym] {
        &self.arrays
    }

    /// Source file paths, indexed by [`SourcePos::file`].
    pub fn source_files(&self) -> &[String] {
        &self.source_files
    }

    /// Map a frame's `(DefinedFuncIndex, wasm_pc)` to its IEC source position:
    /// the absolute pc becomes a within-body offset via the body's start,
    /// then the largest recorded offset `<=` it wins.
    pub fn source_position(&self, defined_index: u32, wasm_pc: u32) -> Option<SourcePos> {
        let body_start = *self.body_starts.get(defined_index as usize)?;
        let within = wasm_pc.checked_sub(body_start)?;
        let table = self.line_tables.get(&defined_index)?;
        let upper = table.partition_point(|e| e.offset <= within);
        let e = table.get(upper.checked_sub(1)?)?;
        Some(SourcePos {
            file: e.file,
            line: e.line,
            col: e.col,
        })
    }

    /// Reverse of [`source_position`](Self::source_position): the wasm
    /// location `(defined_index, absolute pc)` to set a breakpoint at for a
    /// source `(file, line)`, lowest-offset statement first; `None` if the
    /// line has no code.
    pub fn line_to_pc(&self, file: u32, line: u32) -> Option<(u32, u32)> {
        let mut best: Option<(u32, u32)> = None;
        for (&defined, table) in &self.line_tables {
            let Some(&body_start) = self.body_starts.get(defined as usize) else {
                continue;
            };
            for e in table {
                if e.file == file && e.line == line {
                    let pc = body_start + e.offset;
                    if best.is_none_or(|(_, b)| pc < b) {
                        best = Some((defined, pc));
                    }
                }
            }
        }
        best
    }

    /// Whether this module carries a line table: absent means built with
    /// `--release`, watchable but not steppable.
    pub fn has_lines(&self) -> bool {
        self.line_tables.values().any(|t| !t.is_empty())
    }

    /// Every source `(file, line)` a breakpoint can land on, deduplicated:
    /// what a remote debugger prefetches at attach, since breakpoint
    /// verification is a synchronous per-line UI lookup.
    pub fn breakable_lines(&self) -> Vec<(u32, u32)> {
        let mut lines: Vec<(u32, u32)> = self
            .line_tables
            .iter()
            .filter(|(defined, _)| self.body_starts.get(**defined as usize).is_some())
            .flat_map(|(_, table)| table.iter().map(|e| (e.file, e.line)))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// Resolve one wasm frame to its IEC name and source position; `pc` is
    /// the frame's absolute module offset, `None` when unavailable.
    pub fn resolve_frame(&self, defined_index: u32, pc: Option<u32>) -> StackFrame {
        StackFrame {
            defined_index,
            function: self.function_name(defined_index).map(str::to_string),
            source: pc.and_then(|p| self.source_position(defined_index, p)),
            // Filled by the caller, which holds the frame and can read the
            // `this` it was called with.
            instance: None,
        }
    }

    /// Resolve a dotted path to where its value lives, once, behind a
    /// monitoring handle; `None` for a path that names nothing readable.
    pub fn resolve(&self, path: &str) -> Option<VarLoc> {
        let (address, size, ty, global) = self.locate(path)?;
        Some(VarLoc {
            address,
            size,
            ty,
            global,
        })
    }

    /// Where a path's value lives: the flat leaf table first, then computed
    /// through an array descriptor (`base + flat * stride`, bounds-checked)
    /// and the element's [`crate::TypeDesc`] for what follows, so
    /// `pts[7423].history[2].y` resolves without being enumerated.
    fn locate(&self, path: &str) -> Option<(u32, u32, SymType, bool)> {
        if let Some(sym) = self.symbol(path) {
            return Some((sym.address, sym.size, sym.ty, sym.global));
        }
        // `stem[...]…` — descriptor route. The stem is the dotted path of the
        // array itself; everything after the first `[` is accessors.
        let bracket = path.find('[')?;
        let (stem, rest) = path.split_at(bracket);
        let arr = &self.arrays[*self.array_index.get(&fold_path(stem))?];
        let toks = tokenize_accessors(rest)?;
        let mut pos = 0;
        let flat = flatten_indices(&arr.dimensions, &toks, &mut pos)?;
        let mut addr = arr.address + u32::try_from(flat).ok()? * arr.elem_size;
        if pos == toks.len() {
            // The element itself: readable only when scalar — a whole
            // aggregate has no single `VarValue`.
            let t = arr.elem_ty?;
            return Some((addr, arr.elem_size, t, arr.global));
        }
        // Members of the element: walk its layout in the type table.
        let mut ty_id = arr.elem_type?;
        loop {
            match self.types.get(ty_id as usize)? {
                crate::TypeDesc::Scalar(t) => {
                    // Reached a leaf; any leftover accessor is a bad path.
                    return (pos == toks.len()).then(|| (addr, t.size_bytes(), *t, arr.global));
                }
                crate::TypeDesc::Struct { fields, .. } => {
                    let Access::Field(name) = toks.get(pos)? else {
                        return None;
                    };
                    let f = fields
                        .iter()
                        .find(|f| fold_path(&f.name) == fold_path(name))?;
                    addr += f.offset;
                    ty_id = f.ty;
                    pos += 1;
                }
                crate::TypeDesc::Array {
                    dimensions,
                    elem_size,
                    elem,
                    ..
                } => {
                    let flat = flatten_indices(dimensions, &toks, &mut pos)?;
                    addr += u32::try_from(flat).ok()? * elem_size;
                    ty_id = *elem;
                }
                // An enum is a leaf, stored as its underlying integer.
                crate::TypeDesc::Enum { storage, .. } => {
                    return (pos == toks.len())
                        .then(|| (addr, storage.size_bytes(), *storage, arr.global));
                }
                // A pointer: locating THROUGH it needs a live dereference.
                crate::TypeDesc::Opaque { .. } => return None,
            }
        }
    }

    /// Decode `bytes` for `sym`, naming the variant when the symbol is an
    /// enumeration.
    pub fn decode_symbol(&self, sym: &Symbol, bytes: &[u8]) -> VarValue {
        let raw = decode(sym.ty, bytes);
        let Some(id) = sym.named_type else {
            return raw;
        };
        let Some(crate::TypeDesc::Enum { name, variants, .. }) = self.types.get(id as usize) else {
            return raw;
        };
        let n = match raw {
            VarValue::I8(v) => i64::from(v),
            VarValue::I16(v) => i64::from(v),
            VarValue::I32(v) => i64::from(v),
            VarValue::I64(v) => v,
            VarValue::U8(v) => i64::from(v),
            VarValue::U16(v) => i64::from(v),
            VarValue::U32(v) => i64::from(v),
            _ => return raw,
        };
        VarValue::Enum {
            type_name: name.clone(),
            variant: variants
                .iter()
                .find(|(_, value)| *value == n)
                .map(|(variant, _)| variant.clone()),
            raw: n,
        }
    }

    /// Snapshot every monitorable variable, in symbol order, reading each
    /// slot through `read(address, size)`; an unreadable slot is skipped.
    pub fn read_all_with<'a>(
        &'a self,
        mut read: impl FnMut(u32, u32) -> Option<Vec<u8>>,
    ) -> Vec<(&'a str, VarValue)> {
        self.symbols
            .iter()
            .filter_map(|s| {
                let bytes = read(s.address, s.size)?;
                Some((s.path.as_str(), self.decode_symbol(s, &bytes)))
            })
            .collect()
    }

    /// Like [`read_all_with`](Self::read_all_with) from a raw linear-memory
    /// slice; an out-of-bounds slot is skipped.
    pub fn read_all_bytes(&self, mem: &[u8]) -> Vec<(String, VarValue, bool, SymType)> {
        let out: Vec<(String, VarValue, bool, SymType)> = self
            .symbols
            .iter()
            .filter_map(|s| {
                let start = s.address as usize;
                let bytes = mem.get(start..start + s.size as usize)?;
                Some((s.path.clone(), self.decode_symbol(s, bytes), s.global, s.ty))
            })
            .collect();
        out
    }

    /// Resolve a variable by path and encode `value` into its bytes (the
    /// debug-session "force"); `None` if the path is unknown or the type
    /// does not match.
    pub fn encode_var(&self, path: &str, value: VarValue) -> Option<(u32, Vec<u8>)> {
        // Resolved as a read is, through the type descriptors, so array
        // elements can be written too.
        let (address, _, ty, _) = self.locate(path)?;
        let bytes = encode(ty, value).ok()?;
        Some((address, bytes))
    }
}

/// One step of a variable path after its stem: a subscript or a field.
#[derive(Debug, PartialEq)]
enum Access {
    Index(i64),
    Field(String),
}

/// Tokenize a path's accessor tail (`"[7][2].y"` → `Index(7), Index(2),
/// Field("y")`), chained and comma forms; `None` on anything malformed.
fn tokenize_accessors(mut rest: &str) -> Option<Vec<Access>> {
    let mut out = Vec::new();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']')?;
            for part in r[..end].split(',') {
                out.push(Access::Index(part.trim().parse().ok()?));
            }
            rest = &r[end + 1..];
        } else {
            let r = rest.strip_prefix('.')?;
            let end = r.find(['.', '[']).unwrap_or(r.len());
            if end == 0 {
                return None;
            }
            out.push(Access::Field(r[..end].to_string()));
            rest = &r[end..];
        }
    }
    Some(out)
}

/// Consume one subscript per dimension from `toks` at `pos`, bounds-check each
/// against its `(lower, upper)`, and return the row-major flat index.
fn flatten_indices(dimensions: &[(i64, i64)], toks: &[Access], pos: &mut usize) -> Option<i64> {
    let mut flat = 0i64;
    for (lo, hi) in dimensions {
        let Access::Index(sub) = toks.get(*pos)? else {
            return None;
        };
        if sub < lo || sub > hi {
            return None;
        }
        flat = flat * (hi - lo + 1) + (sub - lo);
        *pos += 1;
    }
    Some(flat)
}

/// Decode the bytes of a variable of type `ty`.
pub fn decode(ty: SymType, bytes: &[u8]) -> VarValue {
    let i32le = || i32::from_le_bytes(bytes[..4].try_into().unwrap());
    let u32le = || u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let i64le = || i64::from_le_bytes(bytes[..8].try_into().unwrap());
    let u64le = || u64::from_le_bytes(bytes[..8].try_into().unwrap());
    match ty {
        SymType::Bool => VarValue::Bool(i32le() != 0),
        SymType::SInt => VarValue::I8(i32le() as i8),
        SymType::Int => VarValue::I16(i32le() as i16),
        SymType::DInt => VarValue::I32(i32le()),
        SymType::LInt => VarValue::I64(i64le()),
        SymType::USInt | SymType::Byte => VarValue::U8(u32le() as u8),
        // A CHAR is a code point in the slot, not a byte.
        SymType::Char => VarValue::U32(u32le()),
        SymType::UInt | SymType::Word => VarValue::U16(u32le() as u16),
        SymType::UDInt | SymType::DWord => VarValue::U32(u32le()),
        SymType::ULInt | SymType::LWord => VarValue::U64(u64le()),
        SymType::Real => VarValue::F32(f32::from_le_bytes(bytes[..4].try_into().unwrap())),
        SymType::LReal => VarValue::F64(f64::from_le_bytes(bytes[..8].try_into().unwrap())),
        // Date/time encodings are raw 32- or 64-bit integers (ms / ns / days /
        // seconds since epoch); surface the raw integer for now.
        SymType::Time | SymType::Date | SymType::Tod => VarValue::I32(i32le()),
        SymType::LTime
        | SymType::LDate
        | SymType::LTod
        | SymType::DateAndTime
        | SymType::LDateTime => VarValue::I64(i64le()),
        // [len: i32][capacity bytes]: read len, clamp to the buffer, decode UTF-8.
        SymType::String { capacity } => {
            let len = i32::from_le_bytes(bytes[..4].try_into().unwrap()).max(0) as usize;
            let n = len
                .min(capacity as usize)
                .min(bytes.len().saturating_sub(4));
            VarValue::String(String::from_utf8_lossy(&bytes[4..4 + n]).into_owned())
        }
    }
}

/// Encode `value` for a variable of type `ty`, as the bytes to write.
pub fn encode(ty: SymType, value: VarValue) -> Result<Vec<u8>, TypeMismatch> {
    let bytes = match (ty, value) {
        (SymType::Bool, VarValue::Bool(b)) => (b as i32).to_le_bytes().to_vec(),
        (SymType::SInt, VarValue::I8(v)) => (v as i32).to_le_bytes().to_vec(),
        (SymType::Int, VarValue::I16(v)) => (v as i32).to_le_bytes().to_vec(),
        (SymType::DInt, VarValue::I32(v)) => v.to_le_bytes().to_vec(),
        (SymType::LInt, VarValue::I64(v)) => v.to_le_bytes().to_vec(),
        (SymType::USInt | SymType::Byte | SymType::Char, VarValue::U8(v)) => {
            (v as u32).to_le_bytes().to_vec()
        }
        (SymType::Char, VarValue::U32(v)) => v.to_le_bytes().to_vec(),
        (SymType::UInt | SymType::Word, VarValue::U16(v)) => (v as u32).to_le_bytes().to_vec(),
        (SymType::UDInt | SymType::DWord, VarValue::U32(v)) => v.to_le_bytes().to_vec(),
        (SymType::ULInt | SymType::LWord, VarValue::U64(v)) => v.to_le_bytes().to_vec(),
        (SymType::Real, VarValue::F32(v)) => v.to_le_bytes().to_vec(),
        (SymType::LReal, VarValue::F64(v)) => v.to_le_bytes().to_vec(),
        (SymType::Time | SymType::Date | SymType::Tod, VarValue::I32(v)) => {
            v.to_le_bytes().to_vec()
        }
        (
            SymType::LTime
            | SymType::LDate
            | SymType::LTod
            | SymType::DateAndTime
            | SymType::LDateTime,
            VarValue::I64(v),
        ) => v.to_le_bytes().to_vec(),
        // [len: i32][capacity bytes]: write the whole slot (len, the bytes, then
        // zero padding) so no stale tail from a previous value is left behind.
        (SymType::String { capacity }, VarValue::String(s)) => {
            let cap = capacity as usize;
            let src = s.as_bytes();
            let n = src.len().min(cap);
            let mut buf = vec![0u8; 4 + cap];
            buf[..4].copy_from_slice(&(n as i32).to_le_bytes());
            buf[4..4 + n].copy_from_slice(&src[..n]);
            buf
        }
        (ty, value) => return Err(TypeMismatch { ty, value }),
    };
    Ok(bytes)
}

/// Parse the `debug-symbols` custom section out of a compiled core module.
/// Returns an empty list when the section is absent or malformed.
fn read_debug_symbols(
    wasm: &[u8],
    problems: &mut Vec<String>,
) -> (
    Vec<Symbol>,
    Vec<crate::ArraySym>,
    Vec<crate::TypeDesc>,
    Vec<crate::ContainerSym>,
) {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == crate::DEBUG_SYMBOLS_SECTION
        {
            match crate::DebugSymbols::from_msgpack(reader.data()) {
                Ok(table) => {
                    if table.version > crate::DEBUG_SYMBOLS_VERSION {
                        problems.push(format!(
                            "`{}` is version {} but this build reads up to {}; rebuild with a \
                             matching toolchain, or update the runtime",
                            crate::DEBUG_SYMBOLS_SECTION,
                            table.version,
                            crate::DEBUG_SYMBOLS_VERSION
                        ));
                    }
                    return (table.symbols, table.arrays, table.types, table.containers);
                }
                // The encoding is positional, so a newer producer's record fails
                // on length before its version can be read.
                Err(e) => problems.push(format!(
                    "`{}` is present but unreadable ({e}); most likely built by a newer \
                     compiler than this runtime understands",
                    crate::DEBUG_SYMBOLS_SECTION
                )),
            }
            break;
        }
    }
    (Vec::new(), Vec::new(), Vec::new(), Vec::new())
}

/// Parse the `debug-lines` section: the file list and per-function line
/// tables; empty when absent or malformed.
fn read_debug_lines(wasm: &[u8]) -> (Vec<String>, HashMap<u32, Vec<crate::LineEntry>>) {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == crate::DEBUG_LINES_SECTION
            && let Ok(table) = crate::DebugLines::from_msgpack(reader.data())
        {
            let map = table
                .functions
                .into_iter()
                .map(|f| (f.defined_index, f.lines))
                .collect();
            return (table.files, map);
        }
    }
    (Vec::new(), HashMap::new())
}

/// Parse the `debug-locals` custom section: per-function frame-local tables
/// keyed by `DefinedFuncIndex`. Empty when absent or malformed.
fn read_debug_locals(wasm: &[u8]) -> HashMap<u32, crate::FuncLocals> {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == crate::DEBUG_LOCALS_SECTION
            && let Ok(table) = crate::DebugLocals::from_msgpack(reader.data())
        {
            return table
                .functions
                .into_iter()
                .map(|f| (f.defined_index, f))
                .collect();
        }
    }
    HashMap::new()
}

/// The start offset of each defined function's body, by
/// `DefinedFuncIndex`, for turning an absolute `wasm_pc` into a
/// within-body offset.
fn read_body_starts(wasm: &[u8]) -> Vec<u32> {
    let mut starts = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CodeSectionEntry(body)) = payload {
            starts.push(body.range().start as u32);
        }
    }
    starts
}

/// Parse the `debug-functions` custom section: `DefinedFuncIndex → IEC name`.
/// Empty when the section is absent or malformed.
fn read_function_names(wasm: &[u8]) -> HashMap<u32, String> {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == crate::DEBUG_FUNCTIONS_SECTION
            && let Ok(table) = crate::DebugFunctions::from_msgpack(reader.data())
        {
            return table
                .functions
                .into_iter()
                .map(|e| (e.defined_index, e.name))
                .collect();
        }
    }
    HashMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The accessor tokenizer accepts both subscript spellings and rejects
    /// malformed tails outright.
    #[test]
    fn accessor_paths_tokenize_and_malformed_ones_do_not() {
        assert_eq!(
            tokenize_accessors("[7][2].y"),
            Some(vec![
                Access::Index(7),
                Access::Index(2),
                Access::Field("y".into())
            ])
        );
        assert_eq!(
            tokenize_accessors("[1,2].hist[0]"),
            Some(vec![
                Access::Index(1),
                Access::Index(2),
                Access::Field("hist".into()),
                Access::Index(0)
            ])
        );
        assert_eq!(tokenize_accessors("[-3]"), Some(vec![Access::Index(-3)]));
        for bad in ["[x]", "[1", "]", ".", "..y", "[1]junk", "y"] {
            assert_eq!(tokenize_accessors(bad), None, "{bad:?} must not tokenize");
        }
    }

    /// Row-major flattening consumes one subscript per dimension and bounds-
    /// checks each against its own (lower, upper) — per dimension, not flat.
    #[test]
    fn flatten_is_row_major_and_bounds_checked_per_dimension() {
        let dims = [(1i64, 3i64), (1, 3)];
        let toks = tokenize_accessors("[2][3]").unwrap();
        let mut pos = 0;
        assert_eq!(
            flatten_indices(&dims, &toks, &mut pos),
            Some(5),
            "(2-1)*3 + (3-1)"
        );
        assert_eq!(pos, 2);
        // In-range flat offset but out-of-range dimension: refused.
        let toks = tokenize_accessors("[1][9]").unwrap();
        let mut pos = 0;
        assert_eq!(flatten_indices(&dims, &toks, &mut pos), None);
        // A field where a subscript belongs: refused.
        let toks = tokenize_accessors("[1].y").unwrap();
        let mut pos = 0;
        assert_eq!(flatten_indices(&dims, &toks, &mut pos), None);
    }
}

#[cfg(test)]
mod section_health_tests {
    use super::*;

    /// A minimal wasm module carrying one custom section, hand-encoded so the
    /// crate gains no dependency for a test.
    fn module_with_section(name: &str, data: &[u8]) -> Vec<u8> {
        fn leb128(mut v: u32, out: &mut Vec<u8>) {
            loop {
                let byte = (v & 0x7f) as u8;
                v >>= 7;
                if v == 0 {
                    out.push(byte);
                    return;
                }
                out.push(byte | 0x80);
            }
        }
        let mut body = Vec::new();
        leb128(name.len() as u32, &mut body);
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(data);

        let mut m = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        m.push(0); // custom section id
        leb128(body.len() as u32, &mut m);
        m.extend_from_slice(&body);
        m
    }

    /// A STRIPPED binary is silent — no symbols and nothing to report.
    #[test]
    fn a_stripped_binary_reports_no_problem() {
        let info = DebugInfo::from_wasm(&module_with_section("something-else", b"x"));
        assert!(info.list_symbols().is_empty());
        assert!(
            info.problems().is_empty(),
            "absence of debug info is not a problem to report"
        );
    }

    /// A binary whose debug section is present but unreadable must say so.
    #[test]
    fn an_unreadable_section_is_reported_not_swallowed() {
        let info = DebugInfo::from_wasm(&module_with_section(
            crate::DEBUG_SYMBOLS_SECTION,
            b"\xc1 not messagepack",
        ));
        assert!(info.list_symbols().is_empty());
        let problems = info.problems();
        assert_eq!(problems.len(), 1, "got: {problems:?}");
        assert!(
            problems[0].contains("present but unreadable"),
            "got: {problems:?}"
        );
    }

    /// A section from a newer producer is flagged rather than half-trusted.
    #[test]
    fn a_future_version_is_flagged() {
        let mut table = crate::DebugSymbols::new();
        table.version = crate::DEBUG_SYMBOLS_VERSION + 1;
        let info = DebugInfo::from_wasm(&module_with_section(
            crate::DEBUG_SYMBOLS_SECTION,
            &table.to_msgpack(),
        ));
        let problems = info.problems();
        assert_eq!(problems.len(), 1, "got: {problems:?}");
        assert!(
            problems[0].contains("this build reads up to"),
            "got: {problems:?}"
        );
    }
}
