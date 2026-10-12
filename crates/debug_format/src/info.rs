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
    /// a dropped row would read as "no such variable".
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
pub(crate) fn fold_path(path: &str) -> String {
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
    /// Set for a part of a wider value: the `size` bytes at `address` are
    /// that value's, and this says which of their bits are this one's.
    /// [`decode`](Self::decode) and [`encode`](Self::encode) account for it.
    pub bits: Option<crate::SymBits>,
    /// The enumeration this is a value of, when it is one:
    /// [`DebugInfo::decode_at`] names the variant, where
    /// [`decode`](Self::decode) gives the integer.
    pub named: Option<crate::TypeRef>,
}

impl VarLoc {
    /// The value, from the `size` bytes read at `address`.
    pub fn decode(&self, bytes: &[u8]) -> VarValue {
        match self.bits {
            Some(bits) => decode(self.ty, &bits.extract(bytes)),
            None => decode(self.ty, bytes),
        }
    }

    /// The bytes to write at `address` to set the value. `current` is what
    /// is there now: a part is written by replacing its bits in it.
    pub fn encode(&self, current: &[u8], value: VarValue) -> Result<Vec<u8>, TypeMismatch> {
        let bytes = encode(self.ty, value)?;
        Ok(match self.bits {
            Some(bits) => bits.replace(current, &bytes),
            None => bytes,
        })
    }
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
    /// `DefinedFuncIndex` → the function body's `[start, end)` offsets in the
    /// binary, for converting an absolute `wasm_pc` to a within-body offset,
    /// and for the function a bare offset is in.
    body_ranges: Vec<(u32, u32)>,
    /// How many functions the module imports: they take the first function
    /// indices, ahead of the defined ones every table here is keyed by.
    imported_functions: u32,
    /// Source file paths, indexed by `LineEntry::file`.
    source_files: Vec<String>,
    /// Per-function frame-local tables (`debug-locals`), keyed by
    /// `DefinedFuncIndex`.
    frame_locals: HashMap<u32, crate::FuncLocals>,
    /// The type table of `debug-locals`: what a frame's symbols and arrays
    /// reference. Its ids are its own, not those of `types`.
    local_types: Vec<crate::TypeDesc>,
    /// Every pointer in the module's memory (`debug-symbols` v9), sorted by
    /// path.
    pointers: Vec<crate::PointerSym>,
    pointer_index: HashMap<String, usize>,
    /// The symbols by address, then in `symbols` order, for
    /// [`variable_at`](Self::variable_at).
    by_address: Vec<(u32, usize)>,
    /// `[base, end)` of the stack of recursive calls' frames.
    stack: Option<(u32, u32)>,
    /// Why a debug section that is present could not be used, or is used
    /// in part: a decode failure, or a version newer than this build knows.
    /// A newer compiler's added fields are not a failure, they are skipped.
    problems: Vec<String>,
}

impl DebugInfo {
    /// Parse the debug sections out of a compiled core module's bytes. Returns
    /// an empty view for a stripped or hand-written module.
    pub fn from_wasm(wasm: &[u8]) -> Self {
        let mut problems = Vec::new();
        let table = read_section::<crate::DebugSymbols>(wasm, &mut problems).unwrap_or_default();
        let (symbols, arrays, types, containers, pointers, stack) = (
            table.symbols,
            table.arrays,
            table.types,
            table.containers,
            table.pointers,
            table.stack,
        );
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
        let function_names = read_section::<crate::DebugFunctions>(wasm, &mut problems)
            .map(|table| {
                table
                    .functions
                    .into_iter()
                    .map(|e| (e.defined_index, e.name))
                    .collect()
            })
            .unwrap_or_default();
        let (source_files, line_tables) = read_section::<crate::DebugLines>(wasm, &mut problems)
            .map(|table| {
                let lines = table
                    .functions
                    .into_iter()
                    .map(|f| (f.defined_index, f.lines))
                    .collect();
                (table.files, lines)
            })
            .unwrap_or_default();
        let pointer_index = pointers
            .iter()
            .enumerate()
            .map(|(i, p)| (fold_path(&p.path), i))
            .collect();
        let by_address = by_address(&symbols);
        let body_ranges = read_body_ranges(wasm);
        let imported_functions = count_imported_functions(wasm);
        let (frame_locals, local_types) = read_section::<crate::DebugLocals>(wasm, &mut problems)
            .map(|table| {
                let frames = table
                    .functions
                    .into_iter()
                    .map(|f| (f.defined_index, f))
                    .collect();
                (frames, table.types)
            })
            .unwrap_or_default();
        DebugInfo {
            symbols,
            symbol_index,
            arrays,
            array_index,
            types,
            containers,
            frame_locals,
            local_types,
            pointers,
            pointer_index,
            by_address,
            stack,
            function_names,
            line_tables,
            body_ranges,
            imported_functions,
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
        let by_address = by_address(&symbols);
        DebugInfo {
            symbols,
            symbol_index,
            arrays: Vec::new(),
            array_index: HashMap::new(),
            types: Vec::new(),
            containers: Vec::new(),
            frame_locals: HashMap::new(),
            local_types: Vec::new(),
            pointers: Vec::new(),
            pointer_index: HashMap::new(),
            by_address,
            stack: None,
            function_names: HashMap::new(),
            line_tables: HashMap::new(),
            body_ranges: Vec::new(),
            imported_functions: 0,
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
        let (body_start, _) = *self.body_ranges.get(defined_index as usize)?;
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
            let Some(&(body_start, _)) = self.body_ranges.get(defined as usize) else {
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
            .filter(|(defined, _)| self.body_ranges.get(**defined as usize).is_some())
            .flat_map(|(_, table)| table.iter().map(|e| (e.file, e.line)))
            .collect();
        lines.sort_unstable();
        lines.dedup();
        lines
    }

    /// The `DefinedFuncIndex` of a function index as a trap's backtrace or a
    /// browser's stack reports it (`wasm-function[7]`), which counts the
    /// imported functions first. `None` for an import: it has no body here.
    pub fn defined_index(&self, func_index: u32) -> Option<u32> {
        func_index.checked_sub(self.imported_functions)
    }

    /// The defined function whose body holds the module offset `pc`: what a
    /// bare code address resolves to, when nothing names its function.
    pub fn function_at(&self, pc: u32) -> Option<u32> {
        let at = self
            .body_ranges
            .partition_point(|&(start, _)| start <= pc)
            .checked_sub(1)?;
        let (start, end) = self.body_ranges[at];
        (pc >= start && pc < end).then_some(at as u32)
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
        if let Some(sym) = self.symbol(path) {
            return leaf_loc(sym, 0, crate::Table::Module);
        }
        let (address, size, ty, global, named) = self.locate(path)?;
        Some(VarLoc {
            address,
            size,
            ty,
            global,
            bits: None,
            named: named.map(|id| crate::TypeRef {
                table: crate::Table::Module,
                id,
            }),
        })
    }

    /// Where a path's value lives: the flat leaf table first, then computed
    /// through an array descriptor (`base + flat * stride`, bounds-checked)
    /// and the element's [`crate::TypeDesc`] for what follows, so
    /// `pts[7423].history[2].y` resolves without being enumerated.
    fn locate(&self, path: &str) -> Option<Located> {
        if let Some(sym) = self.symbol(path) {
            return Some((sym.address, sym.size, sym.ty, sym.global, sym.named_type));
        }
        // `stem[...]…` — descriptor route. The stem is the dotted path of the
        // array itself; everything after the first `[` is accessors.
        let bracket = path.find('[')?;
        let (stem, rest) = path.split_at(bracket);
        let arr = &self.arrays[*self.array_index.get(&fold_path(stem))?];
        locate_in_array(arr, rest, &self.types)
    }

    /// Resolve a path among the memory locals of one function's frame:
    /// `r.x`, `recs[12].a`. As [`resolve`](Self::resolve), against the
    /// frame's own leaves and arrays and the type table of `debug-locals`.
    /// `None` for a wasm local, which has no address, and for a recursive
    /// function, whose memory locals are in the frame of each call.
    pub fn resolve_local(&self, defined_index: u32, path: &str) -> Option<VarLoc> {
        let frame = self.frame_locals.get(&defined_index)?;
        let folded = fold_path(path);
        if let Some(sym) = frame.memory.iter().find(|s| fold_path(&s.path) == folded) {
            return leaf_loc(sym, 0, crate::Table::Frames);
        }
        let bracket = path.find('[')?;
        let (stem, rest) = path.split_at(bracket);
        let stem = fold_path(stem);
        let arr = frame.arrays.iter().find(|a| fold_path(&a.path) == stem)?;
        let (address, size, ty, global, named) = locate_in_array(arr, rest, &self.local_types)?;
        Some(VarLoc {
            address,
            size,
            ty,
            global,
            bits: None,
            named: named.map(|id| crate::TypeRef {
                table: crate::Table::Frames,
                id,
            }),
        })
    }

    /// Resolve a path among the memory locals of a recursive function, in
    /// the frame of one call: as [`resolve_local`](Self::resolve_local), with
    /// each address counted from `frame_base`, the value of the wasm local
    /// [`FrameLocals::slot`](crate::FrameLocals::slot) names in that call.
    pub fn resolve_in_frame(
        &self,
        defined_index: u32,
        path: &str,
        frame_base: u32,
    ) -> Option<VarLoc> {
        let frame = self.frame_locals.get(&defined_index)?.frame.as_ref()?;
        let folded = fold_path(path);
        if let Some(sym) = frame.memory.iter().find(|s| fold_path(&s.path) == folded) {
            return leaf_loc(sym, frame_base, crate::Table::Frames);
        }
        let bracket = path.find('[')?;
        let (stem, rest) = path.split_at(bracket);
        let stem = fold_path(stem);
        let arr = frame.arrays.iter().find(|a| fold_path(&a.path) == stem)?;
        let (address, size, ty, _, named) = locate_in_array(arr, rest, &self.local_types)?;
        Some(VarLoc {
            address: frame_base.checked_add(address)?,
            size,
            ty,
            global: false,
            bits: None,
            named: named.map(|id| crate::TypeRef {
                table: crate::Table::Frames,
                id,
            }),
        })
    }

    /// Decode the bytes read at `loc`, naming the variant when `loc` is an
    /// enumeration's: [`VarLoc::decode`] gives the integer it is stored as.
    pub fn decode_at(&self, loc: &VarLoc, bytes: &[u8]) -> VarValue {
        let raw = loc.decode(bytes);
        match loc.named {
            Some(crate::TypeRef { table, id }) => name_variant(self.table(table), id, raw),
            None => raw,
        }
    }

    fn table(&self, table: crate::Table) -> &[crate::TypeDesc] {
        match table {
            crate::Table::Module => &self.types,
            crate::Table::Frames => &self.local_types,
        }
    }

    // ----- Following pointers ---------------------------------------------

    /// The pointer `path` names in the module's memory: a `REF_TO` variable
    /// or member, or a VAR_IN_OUT member of an instance.
    pub fn pointer(&self, path: &str) -> Option<&crate::PointerSym> {
        self.pointer_index
            .get(&fold_path(path))
            .map(|&i| &self.pointers[i])
    }

    /// What `path` names once the pointers on it are followed: `head^.value`
    /// through a `REF_TO`, `motor.speed` through a VAR_IN_OUT member, as far
    /// as `live` reads the module. `None` for a path that names nothing, or
    /// that goes through a pointer without `^`.
    pub fn follow(&self, path: &str, live: &mut dyn Live) -> Option<Place> {
        let (cursor, rest) = self.module_stem(path)?;
        let toks = tokenize_accessors(rest)?;
        self.walk(&self.types, crate::Table::Module, cursor, &toks, live)
    }

    /// [`follow`](Self::follow) among the locals of one function's frame:
    /// its memory locals, the pointers in its wasm locals, which `live`
    /// reads, and, for a recursive function, the locals in the frame of the
    /// call looked at, which starts at `frame_base`.
    pub fn follow_local(
        &self,
        defined_index: u32,
        path: &str,
        frame_base: Option<u32>,
        live: &mut dyn Live,
    ) -> Option<Place> {
        let (cursor, rest) = self.frame_stem(defined_index, path, frame_base)?;
        let toks = tokenize_accessors(rest)?;
        self.walk(&self.local_types, crate::Table::Frames, cursor, &toks, live)
    }

    /// The variable at `address`, when a table names one: a leaf, an element
    /// of an array, or an aggregate by its base. An address on the
    /// [`stack`](Self::stack) is a recursive call's local, which
    /// [`frame_variable_at`](Self::frame_variable_at) names from the live
    /// frames.
    pub fn variable_at(&self, address: u32) -> Option<String> {
        self.variable_at_as(address, None)
    }

    /// `[base, end)` of the stack the frames of recursive calls are pushed
    /// on; `None` when no function pushes one.
    pub fn stack(&self) -> Option<(u32, u32)> {
        self.stack
    }

    /// The local at `address` in one of `frames`, each a live frame as
    /// `(defined index, frame start)`: which of them, by position, and the
    /// local's path in it. For a pointer into a call's locals, which no
    /// static table names: a recursive call's, in its frame, or any call's
    /// at a static address. `target` is the name of the STRUCT the pointer
    /// points at, when it points at one: an aggregate and its first leaf
    /// share a base, and the target says which is meant.
    pub fn frame_variable_at(
        &self,
        address: u32,
        frames: &[(u32, u32)],
        target: Option<&str>,
    ) -> Option<(usize, String)> {
        frames
            .iter()
            .enumerate()
            .find_map(|(at, &(defined_index, base))| {
                let locals = self.frame_locals.get(&defined_index)?;
                let tables = Named {
                    memory: &locals.memory,
                    arrays: &locals.arrays,
                    pointers: &locals.pointers,
                    containers: &locals.containers,
                };
                if let Some(path) = tables.name_at(address, target) {
                    return Some((at, path));
                }
                let frame = locals.frame.as_ref()?;
                let offset = address.checked_sub(base)?;
                if offset >= frame.size {
                    return None;
                }
                let tables = Named {
                    memory: &frame.memory,
                    arrays: &frame.arrays,
                    pointers: &frame.pointers,
                    containers: &frame.containers,
                };
                tables.name_at(offset, target).map(|path| (at, path))
            })
    }

    /// Every pointer in the module's memory, sorted by path.
    pub fn list_pointers(&self) -> &[crate::PointerSym] {
        &self.pointers
    }

    /// Every function's locals table, with its defined index.
    pub fn frame_tables(&self) -> impl Iterator<Item = (u32, &crate::FuncLocals)> {
        self.frame_locals
            .iter()
            .map(|(index, table)| (*index, table))
    }

    /// An entry of the module's type table, by id.
    pub fn module_type(&self, id: crate::TypeId) -> Option<&crate::TypeDesc> {
        self.types.get(id as usize)
    }

    /// An entry of the frames' type table (`debug-locals`), by id.
    pub fn local_type(&self, id: crate::TypeId) -> Option<&crate::TypeDesc> {
        self.local_types.get(id as usize)
    }

    /// [`variable_at`](Self::variable_at) for what a pointer points at: an
    /// aggregate and its first leaf share a base, and the pointer's target,
    /// in `types`, says which is meant.
    fn variable_at_as(
        &self,
        address: u32,
        target: Option<(&[crate::TypeDesc], crate::TypeId)>,
    ) -> Option<String> {
        let target = target.and_then(|(types, id)| types.get(id as usize));
        if let Some(crate::TypeDesc::Struct { name, .. }) = target
            && let Some(c) = self
                .containers
                .iter()
                .find(|c| c.address == address && c.type_name == *name)
        {
            return Some(c.path.clone());
        }
        let leaf = || {
            let at = self.by_address.partition_point(|&(a, _)| a < address);
            match self.by_address.get(at) {
                Some(&(a, i)) if a == address => Some(self.symbols[i].path.clone()),
                _ => None,
            }
        };
        let aggregate = matches!(
            target,
            Some(crate::TypeDesc::Struct { .. } | crate::TypeDesc::Array { .. })
        );
        if !aggregate && let Some(path) = leaf() {
            return Some(path);
        }
        if let Some(c) = self.containers.iter().find(|c| c.address == address) {
            return Some(c.path.clone());
        }
        self.element_at(address).or_else(leaf)
    }

    /// The element of a described array that starts at `address`.
    fn element_at(&self, address: u32) -> Option<String> {
        for arr in self.arrays.iter().filter(|a| a.elem_size > 0) {
            let bytes = arr.total_elements.checked_mul(arr.elem_size)?;
            if address >= arr.address
                && address < arr.address.checked_add(bytes)?
                && (address - arr.address).is_multiple_of(arr.elem_size)
            {
                let flat = (address - arr.address) / arr.elem_size;
                return Some(crate::element_path(&arr.path, flat, &arr.dimensions));
            }
        }
        None
    }

    /// The start of a walk over the module's tables: the longest prefix of
    /// `path` a table names, and what follows it.
    fn module_stem<'a>(&'a self, path: &'a str) -> Option<(Cursor<'a>, &'a str)> {
        for (stem, rest) in stems(path) {
            let folded = fold_path(stem);
            if let Some(&i) = self.symbol_index.get(&folded) {
                return Some((
                    Cursor::Value(leaf_loc(&self.symbols[i], 0, crate::Table::Module)?),
                    rest,
                ));
            }
            if let Some(&i) = self.pointer_index.get(&folded) {
                return Some((pointer_cursor(&self.pointers[i], 0)?, rest));
            }
            if let Some(&i) = self.array_index.get(&folded) {
                return Some((array_cursor(&self.arrays[i], 0)?, rest));
            }
        }
        None
    }

    /// As [`module_stem`](Self::module_stem), over one frame's tables.
    fn frame_stem<'a>(
        &'a self,
        defined_index: u32,
        path: &'a str,
        frame_base: Option<u32>,
    ) -> Option<(Cursor<'a>, &'a str)> {
        let locals = self.frame_locals.get(&defined_index)?;
        for (stem, rest) in stems(path) {
            let folded = fold_path(stem);
            let named = |p: &str| fold_path(p) == folded;
            if let Some(s) = locals.memory.iter().find(|s| named(&s.path)) {
                return Some((Cursor::Value(leaf_loc(s, 0, crate::Table::Frames)?), rest));
            }
            if let Some(p) = locals.pointers.iter().find(|p| named(&p.path)) {
                return Some((pointer_cursor(p, 0)?, rest));
            }
            if let Some(a) = locals.arrays.iter().find(|a| named(&a.path)) {
                return Some((array_cursor(a, 0)?, rest));
            }
            if let Some(p) = locals.local_pointers.iter().find(|p| named(&p.name)) {
                // A parameter's `ARRAY[*]` has the type of its copy.
                let cursor = Cursor::Pointer {
                    at: PointerAt::Local(p.wasm_index),
                    target: p.target,
                    by_ref: p.by_ref,
                    conformand: None,
                    global: false,
                    path: p.name.clone(),
                };
                return Some((cursor, rest));
            }
            if let (Some(base), Some(frame)) = (frame_base, &locals.frame) {
                if let Some(s) = frame.memory.iter().find(|s| named(&s.path)) {
                    return Some((
                        Cursor::Value(leaf_loc(s, base, crate::Table::Frames)?),
                        rest,
                    ));
                }
                if let Some(p) = frame.pointers.iter().find(|p| named(&p.path)) {
                    return Some((pointer_cursor(p, base)?, rest));
                }
                if let Some(a) = frame.arrays.iter().find(|a| named(&a.path)) {
                    return Some((array_cursor(a, base)?, rest));
                }
            }
        }
        None
    }

    /// Walk `toks` from `cursor`, in the scope whose type table is `types`.
    fn walk<'a>(
        &'a self,
        types: &[crate::TypeDesc],
        table: crate::Table,
        mut cursor: Cursor<'a>,
        toks: &[Access],
        live: &mut dyn Live,
    ) -> Option<Place> {
        let mut pos = 0;
        while pos < toks.len() {
            cursor = match (cursor, &toks[pos]) {
                (
                    Cursor::Pointer {
                        at,
                        target,
                        by_ref,
                        conformand,
                        global,
                        path,
                    },
                    Access::Deref,
                ) => {
                    pos += 1;
                    let held = Held {
                        at,
                        target,
                        by_ref,
                        conformand,
                        global,
                        path: &path,
                    };
                    match self.deref(types, table, held, live)? {
                        Ok(next) => next,
                        Err(stop) => return Some(stop),
                    }
                }
                // A VAR_IN_OUT is read as the variable it is bound to: a
                // member of it is that variable's member.
                (
                    Cursor::Pointer {
                        at,
                        target,
                        by_ref: true,
                        conformand,
                        global,
                        path,
                    },
                    _,
                ) => {
                    let held = Held {
                        at,
                        target,
                        by_ref: true,
                        conformand,
                        global,
                        path: &path,
                    };
                    match self.deref(types, table, held, live)? {
                        Ok(next) => next,
                        Err(stop) => return Some(stop),
                    }
                }
                (
                    Cursor::Typed {
                        address,
                        ty,
                        global,
                        path,
                    },
                    Access::Field(name),
                ) => {
                    let crate::TypeDesc::Struct { fields, .. } = types.get(ty as usize)? else {
                        return None;
                    };
                    let f = fields
                        .iter()
                        .find(|f| fold_path(&f.name) == fold_path(name))?;
                    pos += 1;
                    typed(
                        types,
                        table,
                        address.checked_add(f.offset)?,
                        f.ty,
                        global,
                        format!("{path}.{}", f.name),
                    )?
                }
                (
                    Cursor::Typed {
                        address,
                        ty,
                        global,
                        path,
                    },
                    Access::Index(_),
                ) => {
                    let crate::TypeDesc::Array {
                        dimensions,
                        elem_size,
                        elem,
                        ..
                    } = types.get(ty as usize)?
                    else {
                        return None;
                    };
                    let flat = flatten_indices(dimensions, toks, &mut pos)?;
                    let flat = u32::try_from(flat).ok()?;
                    typed(
                        types,
                        table,
                        address.checked_add(flat.checked_mul(*elem_size)?)?,
                        *elem,
                        global,
                        crate::element_path(&path, flat, dimensions),
                    )?
                }
                (
                    Cursor::Array {
                        arr,
                        address,
                        global,
                        path,
                    },
                    Access::Index(_),
                ) => {
                    let flat = flatten_indices(&arr.dimensions, toks, &mut pos)?;
                    let flat = u32::try_from(flat).ok()?;
                    let address = address.checked_add(flat.checked_mul(arr.elem_size)?)?;
                    let path = crate::element_path(&path, flat, &arr.dimensions);
                    match (arr.elem_type, arr.elem_ty) {
                        (Some(ty), _) => typed(types, table, address, ty, global, path)?,
                        (None, Some(ty)) => Cursor::Value(VarLoc {
                            address,
                            size: ty.size_bytes(),
                            ty,
                            global,
                            bits: None,
                            named: None,
                        }),
                        (None, None) => return None,
                    }
                }
                // A value has no members, a STRUCT no elements, a `REF_TO`
                // nothing before `^`.
                _ => return None,
            };
        }
        self.finish(types, table, cursor, live)
    }

    /// Where a walk ends.
    fn finish(
        &self,
        types: &[crate::TypeDesc],
        table: crate::Table,
        cursor: Cursor<'_>,
        live: &mut dyn Live,
    ) -> Option<Place> {
        match cursor {
            Cursor::Value(loc) => Some(Place::Value(loc)),
            Cursor::Typed { address, ty, .. } => Some(Place::Aggregate { address, ty }),
            Cursor::Array {
                arr, address, path, ..
            } => Some(Place::Array(crate::ArraySym {
                path,
                address,
                ..arr.into_owned()
            })),
            Cursor::Pointer {
                at,
                target,
                by_ref: true,
                conformand,
                global,
                path,
            } => {
                let held = Held {
                    at,
                    target,
                    by_ref: true,
                    conformand,
                    global,
                    path: &path,
                };
                match self.deref(types, table, held, live)? {
                    Ok(next) => self.finish(types, table, next, live),
                    Err(stop) => Some(stop),
                }
            }
            Cursor::Pointer {
                at,
                target,
                by_ref: false,
                path,
                ..
            } => {
                let address = held(at, live)?;
                if address == 0 {
                    return Some(Place::Null { pointer: path });
                }
                let variable = self.variable_at_as(address, target.map(|t| (types, t)));
                let in_frame = match variable {
                    Some(_) => None,
                    None => {
                        let frames = live.frames();
                        self.frame_variable_at(address, &frames, struct_name(types, target))
                    }
                };
                Some(Place::Pointer(Pointee {
                    address,
                    target,
                    by_ref: false,
                    variable,
                    in_frame,
                }))
            }
        }
    }

    /// Through a pointer: the cursor at what it points at, or the place the
    /// path stops at. `None` for a `REF_TO` whose target has no type.
    ///
    /// A VAR_IN_OUT is read as the variable it is bound to. One that is an
    /// `ARRAY[*]` member of an instance has no type of its own, and the
    /// instance keeps the address alone: the bounds are constants of the
    /// copy of the body each call runs. The walk goes on through the array
    /// of its rank and element that starts at that address in the module's
    /// memory ([`bound_array`](Self::bound_array)). Where the tables
    /// describe none, and in a frame's table, whose types are not the
    /// module's, the path stops at the pointer, with the address it holds.
    /// So does it for a VAR_IN_OUT the table says nothing of, which is not
    /// known to be an array: a block two VAR_IN_OUTs hold through each
    /// other can be one.
    fn deref<'a>(
        &'a self,
        types: &[crate::TypeDesc],
        table: crate::Table,
        pointer: Held<'_>,
        live: &mut dyn Live,
    ) -> Option<Result<Cursor<'a>, Place>> {
        let address = held(pointer.at, live)?;
        if address == 0 {
            return Some(Err(Place::Null {
                pointer: pointer.path.to_string(),
            }));
        }
        if let Some(target) = pointer.target {
            let path = format!("{}^", pointer.path);
            return Some(Ok(typed(
                types,
                table,
                address,
                target,
                pointer.global,
                path,
            )?));
        }
        if !pointer.by_ref {
            return None;
        }
        if let (crate::Table::Module, Some(of)) = (table, pointer.conformand)
            && let Some(arr) = self.bound_array(address, of)
        {
            return Some(Ok(Cursor::Array {
                address,
                global: arr.global,
                path: pointer.path.to_string(),
                arr: std::borrow::Cow::Owned(arr),
            }));
        }
        let variable = match pointer.conformand {
            // What it is bound to is an array. The element that starts at
            // the address names it closer than the program or the instance
            // whose first member that array is, which start there too.
            Some(_) => self
                .element_at(address)
                .or_else(|| self.variable_at(address)),
            None => self.variable_at(address),
        };
        let in_frame = match variable {
            Some(_) => None,
            None => self.frame_variable_at(address, &live.frames(), None),
        };
        Some(Err(Place::Pointer(Pointee {
            address,
            target: None,
            by_ref: true,
            variable,
            in_frame,
        })))
    }

    /// The array an `ARRAY[*]` is bound to: the one of its rank and element
    /// that starts at `address` in the module's memory, as a descriptor of
    /// its own, with no path. It is a whole array a descriptor gives, the
    /// last dimensions of one (a row, `mat[0]`), or an array in an element,
    /// which the type table gives. An address alone says where an array
    /// starts and not which: answering with the descriptor there read a
    /// bound row as the whole, and an element of `recs[0].list` as the
    /// STRUCT `recs[1]`.
    ///
    /// `None` when the tables describe no such array there: one among a
    /// function's locals. And when they describe two, as for two variables
    /// located at one address.
    fn bound_array(&self, address: u32, of: crate::Conformand) -> Option<crate::ArraySym> {
        let mut found = Vec::new();
        for arr in &self.arrays {
            let Some(offset) = address.checked_sub(arr.address) else {
                continue;
            };
            // Past what the descriptor says it takes: most are.
            let takes = u64::from(arr.total_elements) * u64::from(arr.elem_size);
            if u64::from(offset) >= takes {
                continue;
            }
            let shape = Shape {
                dimensions: &arr.dimensions,
                elem_size: arr.elem_size,
                elem_ty: arr.elem_ty,
                elem_type: arr.elem_type,
            };
            self.arrays_from(shape, offset, of, arr.global, NESTING, &mut found);
            if found.len() > 1 {
                return None;
            }
        }
        let mut bound = found.pop()?;
        bound.address = address;
        Some(bound)
    }

    /// Add to `found` each array of `of`'s rank and element that starts
    /// `offset` bytes into the array `shape` is: its last dimensions, or an
    /// array in the element the offset is in.
    fn arrays_from(
        &self,
        shape: Shape,
        offset: u32,
        of: crate::Conformand,
        global: bool,
        depth: u32,
        found: &mut Vec<crate::ArraySym>,
    ) {
        // A hostile table's bounds can span the whole of i64, and its sizes
        // multiply past any memory.
        let Some(extents) = extents(shape.dimensions) else {
            return;
        };
        let elem_size = u64::from(shape.elem_size);
        let whole = extents
            .iter()
            .try_fold(elem_size, |bytes, extent| bytes.checked_mul(*extent));
        let (Some(whole), offset) = (whole, u64::from(offset)) else {
            return;
        };
        if elem_size == 0 || offset >= whole {
            return;
        }
        let rank = of.rank as usize;
        if (1..=extents.len()).contains(&rank) && self.is_element(shape, of.elem) {
            let last = extents.len() - rank;
            let elements: u64 = extents[last..].iter().product();
            if offset.is_multiple_of(elements * elem_size)
                && let Ok(total_elements) = u32::try_from(elements)
            {
                let (elem_ty, elem_type) = self.element_of(shape);
                found.push(crate::ArraySym {
                    path: String::new(),
                    address: 0,
                    dimensions: shape.dimensions[last..].to_vec(),
                    total_elements,
                    elem_size: shape.elem_size,
                    elem_ty,
                    global,
                    elem_type,
                });
            }
        }
        if let (Some(depth), Some(element)) = (depth.checked_sub(1), shape.elem_type) {
            let within = (offset % elem_size) as u32;
            self.arrays_in(element, within, of, global, depth, found);
        }
    }

    /// [`arrays_from`](Self::arrays_from), `offset` bytes into a value of
    /// the type `ty`: in the member of a STRUCT the offset is in, or in the
    /// array it is.
    fn arrays_in(
        &self,
        ty: crate::TypeId,
        offset: u32,
        of: crate::Conformand,
        global: bool,
        depth: u32,
        found: &mut Vec<crate::ArraySym>,
    ) {
        match self.types.get(ty as usize) {
            Some(crate::TypeDesc::Struct { fields, .. }) => {
                // The member the offset is in is the last that starts at or
                // before it.
                let member = fields
                    .iter()
                    .filter(|f| f.offset <= offset)
                    .max_by_key(|f| f.offset);
                if let (Some(depth), Some(f)) = (depth.checked_sub(1), member) {
                    self.arrays_in(f.ty, offset - f.offset, of, global, depth, found);
                }
            }
            Some(crate::TypeDesc::Array {
                dimensions,
                elem_size,
                elem,
                ..
            }) => {
                let shape = Shape {
                    dimensions,
                    elem_size: *elem_size,
                    elem_ty: None,
                    elem_type: Some(*elem),
                };
                self.arrays_from(shape, offset, of, global, depth, found);
            }
            _ => {}
        }
    }

    /// Whether the elements of `shape` are what an `ARRAY[*] OF` the type
    /// `elem` has: that entry of the table, or the same scalar. A STRING is
    /// one whatever its capacity, an element keeping the one its array
    /// declared.
    fn is_element(&self, shape: Shape, elem: crate::TypeId) -> bool {
        if shape.elem_type == Some(elem) {
            return true;
        }
        let scalar = |id: crate::TypeId| match self.types.get(id as usize) {
            Some(crate::TypeDesc::Scalar(ty)) => Some(*ty),
            _ => None,
        };
        let have = match shape.elem_type {
            Some(id) => scalar(id),
            None => shape.elem_ty,
        };
        match (have, scalar(elem)) {
            (Some(SymType::String { .. }), Some(SymType::String { .. })) => true,
            (Some(have), Some(want)) => have == want,
            _ => false,
        }
    }

    /// The element of `shape` as a descriptor says it: how a value decodes,
    /// and the table's entry for an aggregate, a pointer or an enumeration.
    fn element_of(&self, shape: Shape) -> (Option<SymType>, Option<crate::TypeId>) {
        let Some(id) = shape.elem_type else {
            return (shape.elem_ty, None);
        };
        match self.types.get(id as usize) {
            Some(crate::TypeDesc::Scalar(ty)) => (Some(*ty), None),
            Some(crate::TypeDesc::Enum { storage, .. }) => (Some(*storage), Some(id)),
            _ => (shape.elem_ty, Some(id)),
        }
    }

    /// Decode `bytes` for `sym`, naming the variant when the symbol is an
    /// enumeration.
    pub fn decode_symbol(&self, sym: &Symbol, bytes: &[u8]) -> VarValue {
        decode_named(&self.types, sym, bytes)
    }

    /// [`decode_symbol`](Self::decode_symbol) for a symbol of a frame
    /// ([`FuncLocals::memory`](crate::FuncLocals::memory)): its enumeration
    /// is in the type table of `debug-locals`, whose ids are not those of
    /// `debug-symbols`. Read against that one, an enum local showed its raw
    /// integer, or the variant of whatever type shared its id.
    pub fn decode_local(&self, sym: &Symbol, bytes: &[u8]) -> VarValue {
        decode_named(&self.local_types, sym, bytes)
    }

    /// Snapshot every monitorable variable, in symbol order, reading each
    /// slot through `read(address, size)`; an unreadable slot is skipped.
    pub fn read_all_with(
        &self,
        mut read: impl FnMut(u32, u32) -> Option<Vec<u8>>,
    ) -> Vec<(&str, VarValue)> {
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
    /// does not match, and for a part of a wider value, whose bytes are not
    /// its own: [`encode_var_over`](Self::encode_var_over) writes that.
    pub fn encode_var(&self, path: &str, value: VarValue) -> Option<(u32, Vec<u8>)> {
        self.encode_var_over(path, value, |_, _| None)
    }

    /// As [`encode_var`](Self::encode_var), reading through
    /// `read(address, size)` the bytes a part is written into.
    pub fn encode_var_over(
        &self,
        path: &str,
        value: VarValue,
        read: impl FnOnce(u32, u32) -> Option<Vec<u8>>,
    ) -> Option<(u32, Vec<u8>)> {
        // Resolved as a read is, through the type descriptors, so array
        // elements can be written too.
        let loc = self.resolve(path)?;
        let current = match loc.bits {
            Some(_) => read(loc.address, loc.size)?,
            None => Vec::new(),
        };
        let bytes = loc.encode(&current, value).ok()?;
        Some((loc.address, bytes))
    }
}

/// How a resolver reads what only the running module has: its linear
/// memory, and the wasm locals of the frame being looked at. A closure
/// reading memory is one, for the module's variables.
pub trait Live {
    /// `size` bytes at `address`; `None` when they are outside the memory.
    fn read(&mut self, address: u32, size: u32) -> Option<Vec<u8>>;

    /// The i32 held in the wasm local `index` of the frame looked at. Only
    /// a frame has one: by default there is none.
    fn local(&mut self, index: u32) -> Option<u32> {
        let _ = index;
        None
    }

    /// The live frames, innermost first, each as `(defined index, frame
    /// start)`: what names a pointer into a call's locals. By default there
    /// are none.
    fn frames(&mut self) -> Vec<(u32, u32)> {
        Vec::new()
    }
}

impl<F: FnMut(u32, u32) -> Option<Vec<u8>>> Live for F {
    fn read(&mut self, address: u32, size: u32) -> Option<Vec<u8>> {
        self(address, size)
    }
}

/// What a path names, the pointers on it followed.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    /// A value, to read where it is.
    Value(VarLoc),
    /// A pointer, by what it holds.
    Pointer(Pointee),
    /// A STRUCT, or an array in an element of one, at its base: no value
    /// of its own. `ty` is its layout in the type table of the scope asked.
    Aggregate { address: u32, ty: crate::TypeId },
    /// An array at its base, with its shape: the descriptor a table gives
    /// it, or, through an `ARRAY[*]`, that of the array it is bound to,
    /// whose bounds are what lists its elements. The path is the one asked
    /// for, the address where it is, and `elem_type` an entry of the type
    /// table of the scope asked.
    Array(crate::ArraySym),
    /// A pointer on the way holds no address: the path stops at it.
    Null { pointer: String },
}

/// A pointer's value: the address it holds, and what is there.
#[derive(Debug, Clone, PartialEq)]
pub struct Pointee {
    /// The address held, never 0: that is [`Place::Null`].
    pub address: u32,
    /// What it points at, in the type table of the scope asked (`types`
    /// for the module, `DebugLocals::types` for a frame). `None` when the
    /// compiler had no type for it.
    pub target: Option<crate::TypeId>,
    /// A VAR_IN_OUT, read as the variable itself.
    pub by_ref: bool,
    /// The variable at `address`, when a static table names one.
    pub variable: Option<String>,
    /// Else the local of a live frame ([`Live::frames`]): which frame, by
    /// position, and the local's path in it.
    pub in_frame: Option<(usize, String)>,
}

/// The name of the STRUCT `id` is, in `types`; `None` for any other type.
fn struct_name(types: &[crate::TypeDesc], id: Option<crate::TypeId>) -> Option<&str> {
    match types.get(id? as usize)? {
        crate::TypeDesc::Struct { name, .. } => Some(name),
        _ => None,
    }
}

/// One scope's tables, for naming an address in them: a frame's static
/// locals, or the locals in the frame of a call, by offset.
struct Named<'a> {
    memory: &'a [Symbol],
    arrays: &'a [crate::ArraySym],
    pointers: &'a [crate::PointerSym],
    containers: &'a [crate::ContainerSym],
}

impl Named<'_> {
    /// The variable at `address`: the STRUCT named `target` based there,
    /// else a leaf, a pointer, any aggregate based there, or an element of
    /// an array. An aggregate and its first leaf share a base: the target
    /// says which is meant, and without one the leaf is.
    fn name_at(&self, address: u32, target: Option<&str>) -> Option<String> {
        let at = |c: &&crate::ContainerSym| c.address == address;
        if let Some(name) = target
            && let Some(c) = self
                .containers
                .iter()
                .find(|c| at(c) && c.type_name == name)
        {
            return Some(c.path.clone());
        }
        if let Some(s) = self.memory.iter().find(|s| s.address == address) {
            return Some(s.path.clone());
        }
        if let Some(p) = self.pointers.iter().find(|p| p.address == address) {
            return Some(p.path.clone());
        }
        if let Some(c) = self.containers.iter().find(at) {
            return Some(c.path.clone());
        }
        self.arrays
            .iter()
            .filter(|a| a.elem_size > 0)
            .find_map(|arr| {
                let bytes = arr.total_elements.checked_mul(arr.elem_size)?;
                let inside = address >= arr.address && address < arr.address.checked_add(bytes)?;
                (inside && (address - arr.address).is_multiple_of(arr.elem_size)).then(|| {
                    let flat = (address - arr.address) / arr.elem_size;
                    crate::element_path(&arr.path, flat, &arr.dimensions)
                })
            })
    }
}

/// An array as the search for what an `ARRAY[*]` is bound to sees it: a
/// descriptor's, or one the type table gives in an element.
#[derive(Clone, Copy)]
struct Shape<'a> {
    dimensions: &'a [(i64, i64)],
    elem_size: u32,
    elem_ty: Option<SymType>,
    elem_type: Option<crate::TypeId>,
}

/// How many arrays and STRUCTs deep that search goes into an element. A
/// table can describe a STRUCT that holds itself, which no memory does.
const NESTING: u32 = 32;

/// How many elements each dimension has; `None` for bounds that give none,
/// or more than a count holds.
fn extents(dimensions: &[(i64, i64)]) -> Option<Vec<u64>> {
    dimensions
        .iter()
        .map(|(lo, hi)| {
            let extent = hi.checked_sub(*lo)?.checked_add(1)?;
            u64::try_from(extent).ok().filter(|n| *n > 0)
        })
        .collect()
}

/// Where a pointer's 4 bytes are.
#[derive(Clone, Copy)]
enum PointerAt {
    Memory(u32),
    Local(u32),
}

/// A pointer a walk goes through: where it is, what the table says of it,
/// and the path that names it.
struct Held<'p> {
    at: PointerAt,
    target: Option<crate::TypeId>,
    by_ref: bool,
    conformand: Option<crate::Conformand>,
    global: bool,
    path: &'p str,
}

/// One point of a walk along a path.
enum Cursor<'a> {
    Value(VarLoc),
    /// An address with a layout in the scope's type table: a STRUCT or an
    /// ARRAY.
    Typed {
        address: u32,
        ty: crate::TypeId,
        global: bool,
        path: String,
    },
    /// An array by its descriptor, before its subscripts: a table's, or
    /// the one made for the array an `ARRAY[*]` is bound to.
    Array {
        arr: std::borrow::Cow<'a, crate::ArraySym>,
        address: u32,
        global: bool,
        path: String,
    },
    Pointer {
        at: PointerAt,
        target: Option<crate::TypeId>,
        by_ref: bool,
        conformand: Option<crate::Conformand>,
        global: bool,
        path: String,
    },
}

/// A leaf's location, `base` added to its address: 0 for a static one, the
/// frame's start for one in the frame of a call.
fn leaf_loc(sym: &Symbol, base: u32, table: crate::Table) -> Option<VarLoc> {
    Some(VarLoc {
        address: base.checked_add(sym.address)?,
        size: sym.size,
        ty: sym.ty,
        global: sym.global,
        bits: sym.bits,
        named: sym.named_type.map(|id| crate::TypeRef { table, id }),
    })
}

fn pointer_cursor(p: &crate::PointerSym, base: u32) -> Option<Cursor<'_>> {
    Some(Cursor::Pointer {
        at: PointerAt::Memory(base.checked_add(p.address)?),
        target: p.target,
        by_ref: p.by_ref,
        conformand: p.conformand,
        global: p.global,
        path: p.path.clone(),
    })
}

fn array_cursor(arr: &crate::ArraySym, base: u32) -> Option<Cursor<'_>> {
    Some(Cursor::Array {
        arr: std::borrow::Cow::Borrowed(arr),
        address: base.checked_add(arr.address)?,
        global: arr.global,
        path: arr.path.clone(),
    })
}

/// The cursor at `address` for the type `ty` of `types`: a value, a
/// pointer, or an aggregate to walk into. `None` for a type with nothing
/// at it.
fn typed<'a>(
    types: &[crate::TypeDesc],
    table: crate::Table,
    address: u32,
    ty: crate::TypeId,
    global: bool,
    path: String,
) -> Option<Cursor<'a>> {
    Some(match types.get(ty as usize)? {
        crate::TypeDesc::Scalar(t) => Cursor::Value(VarLoc {
            address,
            size: t.size_bytes(),
            ty: *t,
            global,
            bits: None,
            named: None,
        }),
        crate::TypeDesc::Enum { storage, .. } => Cursor::Value(VarLoc {
            address,
            size: storage.size_bytes(),
            ty: *storage,
            global,
            bits: None,
            named: Some(crate::TypeRef { table, id: ty }),
        }),
        crate::TypeDesc::Opaque {
            size: 4,
            target,
            by_ref,
            conformand,
        } => Cursor::Pointer {
            at: PointerAt::Memory(address),
            target: *target,
            by_ref: *by_ref,
            conformand: *conformand,
            global,
            path,
        },
        crate::TypeDesc::Opaque { .. } => return None,
        crate::TypeDesc::Struct { .. } | crate::TypeDesc::Array { .. } => Cursor::Typed {
            address,
            ty,
            global,
            path,
        },
    })
}

/// The address a pointer holds.
fn held(at: PointerAt, live: &mut dyn Live) -> Option<u32> {
    match at {
        PointerAt::Memory(address) => {
            let bytes = live.read(address, 4)?;
            Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?))
        }
        PointerAt::Local(index) => live.local(index),
    }
}

/// Where a path may split into a stem a table names and the accessors
/// after it, longest stem first: before each `[`, `^` and `.`, and the
/// whole path.
fn stems(path: &str) -> Vec<(&str, &str)> {
    let mut cuts: Vec<usize> = path
        .char_indices()
        .filter(|(_, c)| matches!(c, '[' | '^' | '.'))
        .map(|(i, _)| i)
        .collect();
    cuts.push(path.len());
    cuts.sort_unstable_by(|a, b| b.cmp(a));
    cuts.into_iter()
        .filter(|&i| i > 0)
        .map(|i| path.split_at(i))
        .collect()
}

/// One step of a variable path after its stem: a subscript, a field, or
/// `^`, the value a pointer points at.
#[derive(Debug, PartialEq)]
enum Access {
    Index(i64),
    Field(String),
    Deref,
}

/// Tokenize a path's accessor tail (`"[7][2].y"` → `Index(7), Index(2),
/// Field("y")`), chained and comma forms; `None` on anything malformed.
fn tokenize_accessors(mut rest: &str) -> Option<Vec<Access>> {
    let mut out = Vec::new();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('^') {
            out.push(Access::Deref);
            rest = r;
        } else if let Some(r) = rest.strip_prefix('[') {
            let end = r.find(']')?;
            for part in r[..end].split(',') {
                out.push(Access::Index(part.trim().parse().ok()?));
            }
            rest = &r[end + 1..];
        } else {
            let r = rest.strip_prefix('.')?;
            let end = r.find(['.', '[', '^']).unwrap_or(r.len());
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
        // A hostile table's bounds can span the whole of i64.
        let extent = hi.checked_sub(*lo)?.checked_add(1)?;
        flat = flat
            .checked_mul(extent)?
            .checked_add(sub.checked_sub(*lo)?)?;
        *pos += 1;
    }
    Some(flat)
}

/// Decode the bytes of a variable of type `ty`.
pub fn decode(ty: SymType, bytes: &[u8]) -> VarValue {
    // A table can claim a width its type has not: the bytes it was given
    // are all there is to read.
    let needed = ty.size_bytes() as usize;
    if bytes.len() < needed {
        return VarValue::Unavailable(format!(
            "{} bytes for a {}, which takes {needed}",
            bytes.len(),
            ty.rk_name()
        ));
    }
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
/// The element of `arr` that `rest` names (`[7423].history[2].y`), or a
/// member of it: `base + flat * stride`, bounds-checked, then the element's
/// [`crate::TypeDesc`] in `types` for what follows. `types` is the table of
/// the section `arr` came from.
/// What a path locates: address, size, type, whether a global, and the
/// enumeration it is a value of.
type Located = (u32, u32, SymType, bool, Option<crate::TypeId>);

fn locate_in_array(
    arr: &crate::ArraySym,
    rest: &str,
    types: &[crate::TypeDesc],
) -> Option<Located> {
    let toks = tokenize_accessors(rest)?;
    let mut pos = 0;
    let flat = flatten_indices(&arr.dimensions, &toks, &mut pos)?;
    let mut addr = arr
        .address
        .checked_add(u32::try_from(flat).ok()?.checked_mul(arr.elem_size)?)?;
    if pos == toks.len() {
        // The element itself: readable only when scalar — a whole
        // aggregate has no single `VarValue`. An enumeration's variants are
        // in its entry, when the descriptor names one.
        let t = arr.elem_ty?;
        let named = arr
            .elem_type
            .filter(|id| matches!(types.get(*id as usize), Some(crate::TypeDesc::Enum { .. })));
        return Some((addr, arr.elem_size, t, arr.global, named));
    }
    // Members of the element: walk its layout in the type table.
    let mut ty_id = arr.elem_type?;
    loop {
        match types.get(ty_id as usize)? {
            crate::TypeDesc::Scalar(t) => {
                // Reached a leaf; any leftover accessor is a bad path.
                return (pos == toks.len()).then(|| (addr, t.size_bytes(), *t, arr.global, None));
            }
            crate::TypeDesc::Struct { fields, .. } => {
                let Access::Field(name) = toks.get(pos)? else {
                    return None;
                };
                let f = fields
                    .iter()
                    .find(|f| fold_path(&f.name) == fold_path(name))?;
                addr = addr.checked_add(f.offset)?;
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
                addr = addr.checked_add(u32::try_from(flat).ok()?.checked_mul(*elem_size)?)?;
                ty_id = *elem;
            }
            // An enum is a leaf, stored as its underlying integer.
            crate::TypeDesc::Enum { storage, .. } => {
                return (pos == toks.len()).then(|| {
                    (
                        addr,
                        storage.size_bytes(),
                        *storage,
                        arr.global,
                        Some(ty_id),
                    )
                });
            }
            // A pointer: locating THROUGH it needs a live dereference.
            crate::TypeDesc::Opaque { .. } => return None,
        }
    }
}

/// Decode `bytes` for `sym`, naming the variant when `sym` is an enumeration
/// of `types`, the table of the section `sym` came from.
fn decode_named(types: &[crate::TypeDesc], sym: &Symbol, bytes: &[u8]) -> VarValue {
    let raw = match sym.bits {
        Some(bits) => decode(sym.ty, &bits.extract(bytes)),
        None => decode(sym.ty, bytes),
    };
    match sym.named_type {
        Some(id) => name_variant(types, id, raw),
        None => raw,
    }
}

/// `raw` as a variant of the enumeration `id` is in `types`; `raw` itself
/// when `id` is no enumeration or `raw` no integer.
fn name_variant(types: &[crate::TypeDesc], id: crate::TypeId, raw: VarValue) -> VarValue {
    let Some(crate::TypeDesc::Enum { name, variants, .. }) = types.get(id as usize) else {
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

/// A versioned debug section, for [`read_section`].
trait Section: Sized {
    const NAME: &'static str;
    /// The newest version this build knows.
    const VERSION: u16;
    fn decode(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error>;
    fn version(&self) -> u16;
}

macro_rules! section {
    ($($ty:ident, $name:ident, $version:ident;)*) => {$(
        impl Section for crate::$ty {
            const NAME: &'static str = crate::$name;
            const VERSION: u16 = crate::$version;
            fn decode(bytes: &[u8]) -> Result<Self, rmp_serde::decode::Error> {
                Self::from_msgpack(bytes)
            }
            fn version(&self) -> u16 {
                self.version
            }
        }
    )*};
}

section! {
    DebugSymbols, DEBUG_SYMBOLS_SECTION, DEBUG_SYMBOLS_VERSION;
    DebugFunctions, DEBUG_FUNCTIONS_SECTION, DEBUG_FUNCTIONS_VERSION;
    DebugLines, DEBUG_LINES_SECTION, DEBUG_LINES_VERSION;
    DebugLocals, DEBUG_LOCALS_SECTION, DEBUG_LOCALS_VERSION;
}

/// One debug section of `wasm`, decoded; `None` when the module does not
/// carry it, which is not a problem to report. Every section answers the
/// same way: one that is there and unreadable says so, and one from a newer
/// compiler is used for what this build knows of it, and says that too.
/// The stepping sections used to vanish in silence, where `debug-symbols`
/// spoke.
fn read_section<T: Section>(wasm: &[u8], problems: &mut Vec<String>) -> Option<T> {
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CustomSection(reader)) = payload
            && reader.name() == T::NAME
        {
            return match T::decode(reader.data()) {
                Ok(table) => {
                    if table.version() > T::VERSION {
                        problems.push(format!(
                            "`{}` is version {} but this build reads up to {}; rebuild with a \
                             matching toolchain, or update the runtime",
                            T::NAME,
                            table.version(),
                            T::VERSION
                        ));
                    }
                    Some(table)
                }
                // A newer compiler's added fields are skipped, so what is
                // left is damage, or a change no reader can step over.
                Err(e) => {
                    problems.push(format!(
                        "`{}` is present but unreadable ({e}); most likely built by a newer \
                         compiler than this runtime understands",
                        T::NAME
                    ));
                    None
                }
            };
        }
    }
    None
}

/// How many of the module's imports are functions.
fn count_imported_functions(wasm: &[u8]) -> u32 {
    let mut count = 0;
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::ImportSection(imports)) = payload {
            count += imports
                .into_imports()
                .filter(|import| {
                    matches!(
                        import,
                        Ok(wasmparser::Import {
                            ty: wasmparser::TypeRef::Func(_),
                            ..
                        })
                    )
                })
                .count() as u32;
        }
    }
    count
}

/// The `[start, end)` offsets of each defined function's body, by
/// `DefinedFuncIndex`, for turning an absolute `wasm_pc` into a
/// within-body offset.
fn read_body_ranges(wasm: &[u8]) -> Vec<(u32, u32)> {
    let mut ranges = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let Ok(wasmparser::Payload::CodeSectionEntry(body)) = payload {
            let range = body.range();
            ranges.push((range.start as u32, range.end as u32));
        }
    }
    ranges
}

/// The symbols by address, then in table order: a leaf is found by where
/// it is, and two at one address come in the table's order.
fn by_address(symbols: &[Symbol]) -> Vec<(u32, usize)> {
    let mut by_address: Vec<(u32, usize)> = symbols
        .iter()
        .enumerate()
        .map(|(i, s)| (s.address, i))
        .collect();
    by_address.sort_unstable();
    by_address
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

/// A minimal wasm module carrying these custom sections, hand-encoded so
/// the crate gains no dependency for a test.
#[cfg(test)]
fn module_with_sections(sections: &[(&str, &[u8])]) -> Vec<u8> {
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
    let mut m = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    for (name, data) in sections {
        let mut body = Vec::new();
        leb128(name.len() as u32, &mut body);
        body.extend_from_slice(name.as_bytes());
        body.extend_from_slice(data);
        m.push(0); // custom section id
        leb128(body.len() as u32, &mut m);
        m.extend_from_slice(&body);
    }
    m
}

#[cfg(test)]
mod section_health_tests {
    use super::*;

    fn module_with_section(name: &str, data: &[u8]) -> Vec<u8> {
        module_with_sections(&[(name, data)])
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

    /// The stepping sections speak as `debug-symbols` does. They returned
    /// empty in silence: with a module this build could not read, source
    /// lines, frame names and locals were gone and nothing said why.
    #[test]
    fn every_unreadable_section_is_reported() {
        let garbage: &[u8] = b"\xc1 not messagepack";
        let info = DebugInfo::from_wasm(&module_with_sections(&[
            (crate::DEBUG_SYMBOLS_SECTION, garbage),
            (crate::DEBUG_FUNCTIONS_SECTION, garbage),
            (crate::DEBUG_LINES_SECTION, garbage),
            (crate::DEBUG_LOCALS_SECTION, garbage),
        ]));
        assert!(!info.has_lines());
        assert_eq!(info.function_name(0), None);
        assert!(info.frame_locals(0).is_none());
        let problems = info.problems();
        assert_eq!(problems.len(), 4, "one per section: {problems:?}");
        for name in [
            crate::DEBUG_SYMBOLS_SECTION,
            crate::DEBUG_FUNCTIONS_SECTION,
            crate::DEBUG_LINES_SECTION,
            crate::DEBUG_LOCALS_SECTION,
        ] {
            assert!(
                problems
                    .iter()
                    .any(|p| p.contains(&format!("`{name}`")) && p.contains("unreadable")),
                "{name} is not named in {problems:?}"
            );
        }
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

    /// The same for the stepping sections, which carried a version nothing
    /// compared: each newer one is flagged, and still read.
    #[test]
    fn a_future_version_of_a_stepping_section_is_flagged_and_read() {
        let functions = crate::DebugFunctions {
            version: crate::DEBUG_FUNCTIONS_VERSION + 1,
            functions: vec![crate::FuncEntry {
                defined_index: 0,
                name: "Main".to_string(),
            }],
        };
        let lines = crate::DebugLines {
            version: crate::DEBUG_LINES_VERSION + 1,
            files: vec!["main.st".to_string()],
            functions: Vec::new(),
        };
        let locals = crate::DebugLocals {
            version: crate::DEBUG_LOCALS_VERSION + 1,
            functions: Vec::new(),
            types: Vec::new(),
        };
        let info = DebugInfo::from_wasm(&module_with_sections(&[
            (crate::DEBUG_FUNCTIONS_SECTION, &functions.to_msgpack()),
            (crate::DEBUG_LINES_SECTION, &lines.to_msgpack()),
            (crate::DEBUG_LOCALS_SECTION, &locals.to_msgpack()),
        ]));
        assert_eq!(info.function_name(0), Some("Main"), "still read");
        assert_eq!(info.source_files(), ["main.st".to_string()]);
        let problems = info.problems();
        assert_eq!(problems.len(), 3, "one per section: {problems:?}");
        assert!(
            problems
                .iter()
                .all(|p| p.contains("this build reads up to")),
            "got: {problems:?}"
        );
    }

    /// A newer compiler's section, with a field this build has never seen at
    /// the tail of the table and of each symbol, is read for what it knows.
    /// It was the fault a device meets first: the whole table lost, to a
    /// field it had no use for.
    #[test]
    fn a_newer_compiler_s_extra_fields_do_not_cost_the_section() {
        #[derive(serde::Serialize)]
        struct NewerSymbol {
            path: String,
            address: u32,
            size: u32,
            ty: SymType,
            global: bool,
            named_type: Option<u32>,
            bits: Option<crate::SymBits>,
            unit: String,
        }
        #[derive(serde::Serialize)]
        struct NewerSymbols {
            version: u16,
            symbols: Vec<NewerSymbol>,
            arrays: Vec<crate::ArraySym>,
            types: Vec<crate::TypeDesc>,
            containers: Vec<crate::ContainerSym>,
            pointers: Vec<crate::PointerSym>,
            stack: Option<(u32, u32)>,
            checksum: u64,
        }
        let newer = NewerSymbols {
            version: crate::DEBUG_SYMBOLS_VERSION,
            symbols: vec![NewerSymbol {
                path: "P1.speed".to_string(),
                address: 1024,
                size: 4,
                ty: SymType::Real,
                global: false,
                named_type: None,
                bits: None,
                unit: "rpm".to_string(),
            }],
            arrays: Vec::new(),
            types: Vec::new(),
            containers: Vec::new(),
            pointers: Vec::new(),
            stack: None,
            checksum: 7,
        };
        let info = DebugInfo::from_wasm(&module_with_section(
            crate::DEBUG_SYMBOLS_SECTION,
            &rmp_serde::to_vec(&newer).unwrap(),
        ));
        assert!(info.problems().is_empty(), "got: {:?}", info.problems());
        let loc = info.resolve("p1.SPEED").expect("the symbol is there");
        assert_eq!((loc.address, loc.size, loc.ty), (1024, 4, SymType::Real));
    }

    /// One frame with an enum leaf and an array of records, whose type ids
    /// are those of `debug-locals`, beside a `debug-symbols` table where the
    /// same ids name other types.
    fn module_with_a_frame() -> Vec<u8> {
        let lamp = crate::TypeDesc::Enum {
            name: "Lamp".to_string(),
            storage: SymType::Int,
            variants: vec![("Off".to_string(), 0), ("On".to_string(), 1)],
        };
        let gear = crate::TypeDesc::Enum {
            name: "Gear".to_string(),
            storage: SymType::Int,
            variants: vec![("Low".to_string(), 0), ("High".to_string(), 1)],
        };
        let rec = crate::TypeDesc::Struct {
            name: "Rec".to_string(),
            size: 8,
            fields: vec![
                crate::FieldDesc {
                    name: "a".to_string(),
                    offset: 0,
                    ty: 2,
                },
                crate::FieldDesc {
                    name: "b".to_string(),
                    offset: 4,
                    ty: 2,
                },
            ],
        };
        let mut symbols = crate::DebugSymbols::new();
        // Id 0 of the module table is another enum than id 0 of the frames'.
        symbols.types = vec![lamp];
        let leaf = |path: &str, address: u32, named_type| Symbol {
            path: path.to_string(),
            address,
            size: 4,
            ty: SymType::Int,
            global: false,
            named_type,
            bits: None,
        };
        let locals = crate::DebugLocals {
            version: crate::DEBUG_LOCALS_VERSION,
            functions: vec![crate::FuncLocals {
                defined_index: 3,
                locals: Vec::new(),
                memory: vec![leaf("g", 4096, Some(0)), leaf("r.x", 4100, None)],
                arrays: vec![crate::ArraySym {
                    path: "recs".to_string(),
                    address: 8192,
                    elem_size: 8,
                    total_elements: 1000,
                    dimensions: vec![(0, 999)],
                    elem_ty: None,
                    global: false,
                    elem_type: Some(1),
                }],
                this_slot: None,
                pointers: Vec::new(),
                local_pointers: Vec::new(),
                frame: None,
                containers: Vec::new(),
            }],
            types: vec![gear, rec, crate::TypeDesc::Scalar(SymType::DInt)],
        };
        module_with_sections(&[
            (crate::DEBUG_SYMBOLS_SECTION, &symbols.to_msgpack()),
            (crate::DEBUG_LOCALS_SECTION, &locals.to_msgpack()),
        ])
    }

    /// A frame's enum is named from the type table of `debug-locals`. That
    /// table was dropped on reading, and the only decode there was looked
    /// the id up in `debug-symbols`: this `Gear` read as a `Lamp`.
    #[test]
    fn a_frame_s_enum_is_named_from_its_own_table() {
        let info = DebugInfo::from_wasm(&module_with_a_frame());
        assert!(info.problems().is_empty(), "got: {:?}", info.problems());
        let frame = info.frame_locals(3).expect("the frame");
        let gear = &frame.memory[0];
        let one = 1i32.to_le_bytes();
        assert_eq!(
            info.decode_local(gear, &one),
            VarValue::Enum {
                type_name: "Gear".to_string(),
                variant: Some("High".to_string()),
                raw: 1,
            }
        );
        assert_eq!(
            info.decode_symbol(gear, &one),
            VarValue::Enum {
                type_name: "Lamp".to_string(),
                variant: Some("On".to_string()),
                raw: 1,
            },
            "the module's table answers for a module symbol, as before"
        );
    }

    /// A path into a frame resolves like a module path: a leaf by name, an
    /// element of the frame's array through its descriptor and the frames'
    /// type table, in either case of the letters.
    #[test]
    fn a_path_into_a_frame_resolves() {
        let info = DebugInfo::from_wasm(&module_with_a_frame());
        let at = |path: &str| {
            info.resolve_local(3, path)
                .map(|loc| (loc.address, loc.size, loc.ty))
        };
        assert_eq!(at("r.x"), Some((4100, 4, SymType::Int)));
        assert_eq!(at("R.X"), Some((4100, 4, SymType::Int)));
        assert_eq!(
            at("recs[12].b"),
            Some((8192 + 12 * 8 + 4, 4, SymType::DInt))
        );
        assert_eq!(at("RECS[0].a"), Some((8192, 4, SymType::DInt)));
        assert_eq!(at("recs[1000].a"), None, "past the array");
        assert_eq!(at("recs[1].c"), None, "no such field");
        assert_eq!(at("nothing"), None);
        assert_eq!(
            info.resolve_local(4, "r.x").map(|l| l.address),
            None,
            "no such frame"
        );
        assert_eq!(
            info.resolve("recs[12].b").map(|l| l.address),
            None,
            "a frame's array is not a module path"
        );
    }
}

#[cfg(test)]
mod follow_tests {
    use super::*;
    use crate::{ArraySym, ContainerSym, PointerSym, TypeDesc};

    fn leaf(path: &str, address: u32, ty: SymType) -> Symbol {
        Symbol {
            path: path.to_string(),
            address,
            size: ty.size_bytes(),
            ty,
            global: false,
            named_type: None,
            bits: None,
        }
    }

    fn pointer(path: &str, address: u32, target: Option<u32>, by_ref: bool) -> PointerSym {
        PointerSym {
            path: path.to_string(),
            address,
            global: false,
            target,
            by_ref,
            conformand: None,
        }
    }

    /// A VAR_IN_OUT `ARRAY[*]` of `rank` dimensions and of the type `elem`.
    fn conformand(path: &str, address: u32, rank: u32, elem: u32) -> PointerSym {
        PointerSym {
            conformand: Some(crate::Conformand { rank, elem }),
            ..pointer(path, address, None, true)
        }
    }

    fn container(path: &str, address: u32, type_name: &str) -> ContainerSym {
        ContainerSym {
            path: path.to_string(),
            address,
            global: false,
            type_name: type_name.to_string(),
        }
    }

    fn field(name: &str, offset: u32, ty: u32) -> crate::FieldDesc {
        crate::FieldDesc {
            name: name.to_string(),
            offset,
            ty,
        }
    }

    fn opaque(target: Option<u32>, by_ref: bool) -> TypeDesc {
        TypeDesc::Opaque {
            size: 4,
            target,
            by_ref,
            conformand: None,
        }
    }

    /// A program's memory with two `Node`s, `a` at 100 pointing at `b` at
    /// 108, `head` pointing at `a`, an INT `n` at 200 that `m.speed` (by
    /// reference), `ri` and `refs[1]` point at, a `Pt` at 300 `rp` points
    /// at, and two `Node`s in an array at 160.
    fn module() -> (DebugInfo, Vec<u8>) {
        let node = TypeDesc::Struct {
            name: "Node".to_string(),
            size: 8,
            fields: vec![field("value", 0, 1), field("next", 4, 2)],
        };
        let pt = TypeDesc::Struct {
            name: "Pt".to_string(),
            size: 8,
            fields: vec![field("x", 0, 6), field("y", 4, 6)],
        };
        let table = crate::DebugSymbols {
            version: crate::DEBUG_SYMBOLS_VERSION,
            symbols: vec![
                leaf("P1.a.value", 100, SymType::Int),
                leaf("P1.b.value", 108, SymType::Int),
                leaf("P1.n", 200, SymType::Int),
                leaf("P1.pt.x", 300, SymType::Real),
                leaf("P1.pt.y", 304, SymType::Real),
            ],
            arrays: vec![
                ArraySym {
                    path: "P1.refs".to_string(),
                    address: 140,
                    dimensions: vec![(0, 2)],
                    total_elements: 3,
                    elem_size: 4,
                    elem_ty: None,
                    global: false,
                    elem_type: Some(4),
                },
                ArraySym {
                    path: "P1.nodes".to_string(),
                    address: 160,
                    dimensions: vec![(0, 1)],
                    total_elements: 2,
                    elem_size: 8,
                    elem_ty: None,
                    global: false,
                    elem_type: Some(0),
                },
                ArraySym {
                    path: "P1.lamps".to_string(),
                    address: 400,
                    dimensions: vec![(0, 3)],
                    total_elements: 4,
                    elem_size: 4,
                    elem_ty: Some(SymType::Int),
                    global: false,
                    elem_type: Some(7),
                },
                ArraySym {
                    path: "P1.grid".to_string(),
                    address: 440,
                    dimensions: vec![(0, 1), (5, 6)],
                    total_elements: 4,
                    elem_size: 4,
                    elem_ty: Some(SymType::Int),
                    global: false,
                    elem_type: None,
                },
            ],
            types: vec![
                node,
                TypeDesc::Scalar(SymType::Int),
                opaque(Some(0), false),
                opaque(Some(1), true),
                opaque(Some(1), false),
                pt,
                TypeDesc::Scalar(SymType::Real),
                TypeDesc::Enum {
                    name: "Lamp".to_string(),
                    storage: SymType::Int,
                    variants: vec![("Off".to_string(), 0), ("On".to_string(), 1)],
                },
            ],
            containers: vec![
                container("P1.a", 100, "Node"),
                container("P1.b", 108, "Node"),
                container("P1.nodes[0]", 160, "Node"),
                container("P1.nodes[1]", 168, "Node"),
                container("P1.pt", 300, "Pt"),
            ],
            pointers: vec![
                pointer("P1.a.next", 104, Some(0), false),
                pointer("P1.b.next", 112, Some(0), false),
                pointer("P1.head", 120, Some(0), false),
                pointer("P1.m.speed", 124, Some(1), true),
                pointer("P1.ri", 128, Some(1), false),
                pointer("P1.rp", 132, Some(5), false),
                pointer("P1.unknown", 136, None, false),
                pointer("P1.w.a", 420, None, true),
                pointer("P1.w.unbound", 424, None, true),
                pointer("P1.w.elsewhere", 428, None, true),
                pointer("P1.w.row", 432, None, true),
                pointer("P1.w.first", 436, None, true),
            ],
            stack: Some((4096, 8192)),
        };
        let info = DebugInfo::from_wasm(&module_with_sections(&[(
            crate::DEBUG_SYMBOLS_SECTION,
            &table.to_msgpack(),
        )]));
        assert!(info.problems().is_empty(), "{:?}", info.problems());
        let mut mem = vec![0u8; 512];
        let mut put = |at: usize, v: u32| mem[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(100, 1);
        put(104, 108);
        put(108, 2);
        put(120, 100);
        put(124, 200);
        put(128, 200);
        put(132, 300);
        put(136, 200);
        put(144, 200);
        put(148, 100);
        put(160, 7);
        put(164, 168);
        put(168, 8);
        put(200, 42);
        put(300, 1.5f32.to_bits());
        put(304, 2.5f32.to_bits());
        put(404, 1);
        put(420, 400);
        put(428, 200);
        put(432, 440);
        put(436, 160);
        put(444, 6);
        (info, mem)
    }

    /// An element of an array of enumerations reads by its variant: the
    /// descriptor names the enumeration, and `decode_at` its variants. It
    /// read as its integer, the location carrying no table entry.
    #[test]
    fn an_enum_element_reads_by_its_variant() {
        let (info, mem) = module();
        let on = info.resolve("P1.lamps[1]").expect("an element");
        assert_eq!(
            on.named,
            Some(crate::TypeRef {
                table: crate::Table::Module,
                id: 7
            })
        );
        let bytes = &mem[404..408];
        assert_eq!(on.decode(bytes), VarValue::I16(1), "the integer");
        assert_eq!(
            info.decode_at(&on, bytes),
            VarValue::Enum {
                type_name: "Lamp".to_string(),
                variant: Some("On".to_string()),
                raw: 1,
            }
        );
        let live = &mut reader(&mem);
        let Some(Place::Value(through)) = info.follow("P1.lamps[1]", live) else {
            panic!("a value");
        };
        assert_eq!(through.named, on.named, "followed, the same");
        assert_eq!(info.resolve("P1.a.value").unwrap().named, None, "an INT");
    }

    fn reader(mem: &[u8]) -> impl FnMut(u32, u32) -> Option<Vec<u8>> + '_ {
        move |address, size| {
            mem.get(address as usize..(address as usize).checked_add(size as usize)?)
                .map(<[u8]>::to_vec)
        }
    }

    /// The location a `Value` place reads at.
    fn at(place: Option<Place>) -> (u32, SymType) {
        match place {
            Some(Place::Value(loc)) => (loc.address, loc.ty),
            other => panic!("not a value: {other:?}"),
        }
    }

    /// A path reads through its pointers, `^` by `^`, and stops at a null
    /// one, naming it.
    #[test]
    fn a_path_follows_its_pointers() {
        let (info, mem) = module();
        let live = &mut reader(&mem);
        assert_eq!(at(info.follow("P1.head^.value", live)), (100, SymType::Int));
        assert_eq!(
            at(info.follow("P1.head^.next^.value", live)),
            (108, SymType::Int)
        );
        assert_eq!(
            info.follow("P1.head^.next^.next^.value", live),
            Some(Place::Null {
                pointer: "P1.head^.next^.next".to_string()
            })
        );
        assert_eq!(
            at(info.follow("P1.a.next^.value", live)),
            (108, SymType::Int)
        );
        assert_eq!(at(info.follow("P1.rp^.y", live)), (304, SymType::Real));
        assert_eq!(
            at(info.follow("p1.HEAD^.VALUE", live)),
            (100, SymType::Int),
            "caseless"
        );
        assert_eq!(
            info.follow("P1.head^", live),
            Some(Place::Aggregate {
                address: 100,
                ty: 0
            })
        );
        // A plain variable still reads as one.
        assert_eq!(at(info.follow("P1.a.value", live)), (100, SymType::Int));
        let value = info.follow("P1.ri^", live);
        let bytes = live(at(value).0, 4).unwrap();
        assert_eq!(decode(SymType::Int, &bytes), VarValue::I16(42));
    }

    /// A pointer's own value is the address it holds and the variable
    /// there: an aggregate when its target is one, the leaf otherwise.
    #[test]
    fn a_pointer_names_what_it_points_at() {
        let (info, mem) = module();
        let live = &mut reader(&mem);
        assert_eq!(
            info.follow("P1.head", live),
            Some(Place::Pointer(Pointee {
                address: 100,
                target: Some(0),
                by_ref: false,
                variable: Some("P1.a".to_string()),
                in_frame: None,
            }))
        );
        assert_eq!(
            info.follow("P1.ri", live),
            Some(Place::Pointer(Pointee {
                address: 200,
                target: Some(1),
                by_ref: false,
                variable: Some("P1.n".to_string()),
                in_frame: None,
            }))
        );
        assert_eq!(
            info.follow("P1.rp", live).map(|p| match p {
                Place::Pointer(p) => p.variable,
                _ => None,
            }),
            Some(Some("P1.pt".to_string()))
        );
        // No type for the target: the address and the leaf there, nothing
        // to walk through.
        assert_eq!(
            info.follow("P1.unknown", live),
            Some(Place::Pointer(Pointee {
                address: 200,
                target: None,
                by_ref: false,
                variable: Some("P1.n".to_string()),
                in_frame: None,
            }))
        );
        assert_eq!(info.follow("P1.unknown^", live), None);
        assert_eq!(info.pointer("p1.head").map(|p| p.address), Some(120));
        assert_eq!(info.pointer("P1.n"), None);
    }

    /// A VAR_IN_OUT reads as the variable it is bound to, with or without
    /// `^`.
    #[test]
    fn a_by_reference_member_reads_as_its_variable() {
        let (info, mem) = module();
        let live = &mut reader(&mem);
        assert_eq!(at(info.follow("P1.m.speed", live)), (200, SymType::Int));
        assert_eq!(at(info.follow("P1.m.speed^", live)), (200, SymType::Int));
    }

    /// A VAR_IN_OUT the table gives no type, and no rank and element
    /// either, is not known to be an array: a block two VAR_IN_OUTs hold
    /// through each other can be one, and so can an `ARRAY[*]` in a table
    /// from before v10. The path stops at the pointer, with the address it
    /// holds and the variable there, and a subscript after it reaches no
    /// variable: with no target the reader answered nothing. Not bound it
    /// reads as null.
    #[test]
    fn a_by_reference_pointer_the_table_says_nothing_of_stops_at_the_pointer() {
        let (info, mem) = module();
        let live = &mut reader(&mem);
        let held = |address: u32, variable: &str| {
            Some(Place::Pointer(Pointee {
                address,
                target: None,
                by_ref: true,
                variable: Some(variable.to_string()),
                in_frame: None,
            }))
        };
        // Where an array of one dimension starts, a matrix, an array of
        // STRUCTs, and a variable.
        for (path, stop) in [
            ("P1.w.a", held(400, "P1.lamps[0]")),
            ("P1.w.row", held(440, "P1.grid[0, 5]")),
            ("P1.w.first", held(160, "P1.nodes[0]")),
            ("P1.w.elsewhere", held(200, "P1.n")),
        ] {
            assert_eq!(info.follow(path, live), stop, "{path}");
            assert_eq!(info.follow(&format!("{path}[1]"), live), stop, "{path}[1]");
        }
        let null = Some(Place::Null {
            pointer: "P1.w.unbound".to_string(),
        });
        assert_eq!(info.follow("P1.w.unbound", live), null);
        assert_eq!(info.follow("P1.w.unbound[1]", live), null);
        // A `REF_TO` with no target still has nothing behind its `^`.
        assert_eq!(info.follow("P1.unknown^", live), None);

        // The arrays themselves read, each subscript within its own bounds,
        // and end as their shape.
        assert_eq!(at(info.follow("P1.grid[0, 6]", live)), (444, SymType::Int));
        assert_eq!(info.follow("P1.grid[0, 7]", live), None);
        let Some(Place::Array(grid)) = info.follow("P1.grid", live) else {
            panic!("an array, with its shape");
        };
        assert_eq!(
            (grid.path.as_str(), grid.address, &grid.dimensions[..]),
            ("P1.grid", 440, &[(0, 1), (5, 6)][..])
        );
    }

    /// A program's memory for what an `ARRAY[*]` is bound to: a matrix
    /// `mat` at 100, three `Rec`s at 200 whose first member is a matrix,
    /// two `Cell`s at 300 whose second is an array, REALs at 340, a `Loop`
    /// that holds itself at 360, and two arrays located together at 380.
    fn bound() -> (DebugInfo, Vec<u8>) {
        // An element is a value, or an entry of the type table.
        type Elem = Result<SymType, u32>;
        let array =
            |path: &str, address, dimensions: &[(i64, i64)], elem_size, elem: Elem| ArraySym {
                path: path.to_string(),
                address,
                dimensions: dimensions.to_vec(),
                total_elements: dimensions
                    .iter()
                    .map(|(lo, hi)| (hi - lo + 1) as u32)
                    .product(),
                elem_size,
                elem_ty: elem.ok(),
                global: false,
                elem_type: elem.err(),
            };
        let table = crate::DebugSymbols {
            version: crate::DEBUG_SYMBOLS_VERSION,
            symbols: Vec::new(),
            arrays: vec![
                array("P1.mat", 100, &[(-1, 0), (2, 3)], 4, Ok(SymType::Int)),
                array("P1.recs", 200, &[(0, 2)], 20, Err(2)),
                array("P1.cells", 300, &[(0, 1)], 12, Err(4)),
                array("P1.reals", 340, &[(0, 3)], 4, Ok(SymType::Real)),
                array("P1.loops", 360, &[(0, 0)], 8, Err(6)),
                array("P1.over", 380, &[(0, 1)], 4, Ok(SymType::Int)),
                array("P1.lap", 380, &[(5, 6)], 4, Ok(SymType::Int)),
            ],
            types: vec![
                TypeDesc::Scalar(SymType::Int),
                TypeDesc::Array {
                    dimensions: vec![(0, 1), (0, 1)],
                    total_elements: 4,
                    elem_size: 4,
                    elem: 0,
                },
                TypeDesc::Struct {
                    name: "Rec".to_string(),
                    size: 20,
                    fields: vec![field("hist", 0, 1), field("k", 16, 0)],
                },
                TypeDesc::Scalar(SymType::Real),
                TypeDesc::Struct {
                    name: "Cell".to_string(),
                    size: 12,
                    fields: vec![field("tag", 0, 0), field("list", 4, 5)],
                },
                TypeDesc::Array {
                    dimensions: vec![(1, 2)],
                    total_elements: 2,
                    elem_size: 4,
                    elem: 0,
                },
                TypeDesc::Struct {
                    name: "Loop".to_string(),
                    size: 8,
                    fields: vec![field("again", 0, 6)],
                },
            ],
            containers: Vec::new(),
            pointers: vec![
                conformand("P1.g.whole", 500, 2, 0),
                conformand("P1.g.top", 504, 1, 0),
                conformand("P1.g.low", 508, 1, 0),
                conformand("P1.g.first", 512, 2, 0),
                conformand("P1.g.inner", 516, 2, 0),
                conformand("P1.g.strip", 520, 1, 0),
                conformand("P1.g.recs", 524, 1, 2),
                conformand("P1.g.list", 528, 1, 0),
                conformand("P1.g.cube", 532, 3, 0),
                conformand("P1.g.reals", 536, 2, 3),
                conformand("P1.g.mid", 540, 1, 0),
                conformand("P1.g.loop", 544, 1, 0),
                conformand("P1.g.lap", 548, 1, 0),
                conformand("P1.g.none", 552, 1, 0),
            ],
            stack: None,
        };
        let info = DebugInfo::from_wasm(&module_with_sections(&[(
            crate::DEBUG_SYMBOLS_SECTION,
            &table.to_msgpack(),
        )]));
        assert!(info.problems().is_empty(), "{:?}", info.problems());
        let mut mem = vec![0u8; 600];
        let mut put = |at: usize, v: u32| mem[at..at + 4].copy_from_slice(&v.to_le_bytes());
        for (at, v) in [(100, 1), (104, 2), (108, 3), (112, 4)] {
            put(at, v);
        }
        // `recs[0].hist[1, 1]`, `recs[1].k`, `recs[2].hist[1, 0]` and
        // `cells[0].list[2]`.
        for (at, v) in [(212, 5), (236, 77), (248, 6), (308, 9)] {
            put(at, v);
        }
        for (pointer, held) in [
            (500, 100),
            (504, 100),
            (508, 108),
            (512, 200),
            (516, 240),
            (520, 208),
            (524, 200),
            (528, 304),
            (532, 100),
            (536, 100),
            (540, 104),
            (544, 360),
            (548, 380),
        ] {
            put(pointer, held);
        }
        (info, mem)
    }

    /// The shape of the array a path ends at.
    fn shape(place: Option<Place>) -> ArraySym {
        match place {
            Some(Place::Array(shape)) => shape,
            other => panic!("not an array: {other:?}"),
        }
    }

    /// The table gives an `ARRAY[*]` its rank and its element, and it reads
    /// as the array of that rank and element that starts at the address it
    /// holds, with that array's bounds: a whole array, the last dimensions
    /// of one, or an array in an element. Its place is that array's shape,
    /// which is what lists its elements.
    #[test]
    fn an_array_of_any_bounds_reads_as_the_array_of_its_rank_and_element() {
        let (info, mem) = bound();
        let live = &mut reader(&mem);
        let read = |place: Option<Place>| {
            let (address, _) = at(place);
            u32::from_le_bytes(mem[address as usize..][..4].try_into().unwrap())
        };

        // A matrix and its first row start together: the rank says which.
        let whole = shape(info.follow("P1.g.whole", live));
        assert_eq!(whole.path, "P1.g.whole");
        assert_eq!((whole.address, whole.total_elements), (100, 4));
        assert_eq!(whole.dimensions, [(-1, 0), (2, 3)]);
        assert_eq!((whole.elem_ty, whole.elem_type), (Some(SymType::Int), None));
        assert_eq!(read(info.follow("P1.g.whole[0, 3]", live)), 4);
        assert_eq!(
            info.follow("P1.g.whole[-1, 4]", live),
            None,
            "past the second dimension, inside the whole"
        );
        assert_eq!(info.follow("P1.g.whole[1, 2]", live), None);

        let top = shape(info.follow("P1.g.top", live));
        assert_eq!((top.address, &top.dimensions[..]), (100, &[(2, 3)][..]));
        assert_eq!(top.total_elements, 2);
        assert_eq!(read(info.follow("P1.g.top[3]", live)), 2);
        assert_eq!(info.follow("P1.g.top[4]", live), None, "the next row");
        assert_eq!(info.follow("P1.g.top[-1, 2]", live), None, "one dimension");

        // A row no descriptor starts at.
        let low = shape(info.follow("P1.g.low", live));
        assert_eq!((low.address, &low.dimensions[..]), (108, &[(2, 3)][..]));
        assert_eq!(read(info.follow("P1.g.low[2]", live)), 3);

        // An array of STRUCTs and the matrix its first element begins with
        // start together: the element says which.
        let first = shape(info.follow("P1.g.first", live));
        assert_eq!(first.address, 200);
        assert_eq!(first.dimensions, [(0, 1), (0, 1)]);
        assert_eq!((first.elem_ty, first.elem_type), (Some(SymType::Int), None));
        assert_eq!(read(info.follow("P1.g.first[1, 1]", live)), 5);
        let recs = shape(info.follow("P1.g.recs", live));
        assert_eq!((recs.address, &recs.dimensions[..]), (200, &[(0, 2)][..]));
        assert_eq!((recs.elem_ty, recs.elem_type), (None, Some(2)));
        assert_eq!(read(info.follow("P1.g.recs[1].k", live)), 77);
        assert_eq!(read(info.follow("P1.g.recs[2].hist[1, 0]", live)), 6);

        // In a later element, in a row of it, and in a member that is not
        // the first.
        assert_eq!(read(info.follow("P1.g.inner[1, 0]", live)), 6);
        assert_eq!(shape(info.follow("P1.g.strip", live)).dimensions, [(0, 1)]);
        assert_eq!(read(info.follow("P1.g.strip[1]", live)), 5);
        let list = shape(info.follow("P1.g.list", live));
        assert_eq!((list.address, &list.dimensions[..]), (304, &[(1, 2)][..]));
        assert_eq!(read(info.follow("P1.g.list[2]", live)), 9);
        assert_eq!(info.follow("P1.g.list[0]", live), None);

        assert_eq!(
            info.follow("P1.g.none[1]", live),
            Some(Place::Null {
                pointer: "P1.g.none".to_string()
            })
        );
    }

    /// Where the tables describe no array of the rank and element an
    /// `ARRAY[*]` has, the path stops at the pointer, with the address it
    /// holds: more dimensions than the array there, another element, an
    /// address inside a row. So does it where they describe two, located
    /// together. A table can describe a STRUCT that holds itself, which no
    /// memory does: the search ends.
    #[test]
    fn an_array_of_any_bounds_stops_where_no_array_of_its_rank_and_element_starts() {
        let (info, mem) = bound();
        let live = &mut reader(&mem);
        for (path, address, variable) in [
            ("P1.g.cube", 100, "P1.mat[-1, 2]"),
            ("P1.g.reals", 100, "P1.mat[-1, 2]"),
            ("P1.g.mid", 104, "P1.mat[-1, 3]"),
            ("P1.g.loop", 360, "P1.loops[0]"),
            ("P1.g.lap", 380, "P1.over[0]"),
        ] {
            let stop = Some(Place::Pointer(Pointee {
                address,
                target: None,
                by_ref: true,
                variable: Some(variable.to_string()),
                in_frame: None,
            }));
            assert_eq!(info.follow(path, live), stop, "{path}");
            assert_eq!(info.follow(&format!("{path}[0]"), live), stop, "{path}[0]");
        }
    }

    /// An array of pointers, and an array of structs with a pointer in each:
    /// the element is reached through the descriptor, the pointer through
    /// the element.
    #[test]
    fn an_element_can_be_a_pointer_or_hold_one() {
        let (info, mem) = module();
        let live = &mut reader(&mem);
        assert_eq!(at(info.follow("P1.refs[1]^", live)), (200, SymType::Int));
        assert_eq!(
            info.follow("P1.refs[0]^", live),
            Some(Place::Null {
                pointer: "P1.refs[0]".to_string()
            })
        );
        assert_eq!(
            info.follow("P1.refs[2]", live),
            Some(Place::Pointer(Pointee {
                address: 100,
                target: Some(1),
                by_ref: false,
                variable: Some("P1.a.value".to_string()),
                in_frame: None,
            }))
        );
        assert_eq!(
            at(info.follow("P1.nodes[0].next^.value", live)),
            (168, SymType::Int)
        );
        assert_eq!(
            info.follow("P1.nodes[1]", live),
            Some(Place::Aggregate {
                address: 168,
                ty: 0
            })
        );
        assert_eq!(info.follow("P1.refs[3]^", live), None, "past the array");
    }

    /// What names nothing, and a pointer gone through without `^`.
    #[test]
    fn a_path_through_a_pointer_needs_a_caret() {
        let (info, mem) = module();
        let live = &mut reader(&mem);
        assert_eq!(info.follow("P1.a.next.value", live), None);
        assert_eq!(info.follow("P1.n^", live), None, "not a pointer");
        assert_eq!(info.follow("P1.head^.nope", live), None);
        assert_eq!(info.follow("P1.nothing", live), None);
        assert_eq!(info.follow("", live), None);
        assert_eq!(info.follow("^", live), None);
    }

    /// Which variable an address is: a leaf, an element, an aggregate.
    #[test]
    fn an_address_names_its_variable() {
        let (info, _) = module();
        assert_eq!(info.variable_at(100).as_deref(), Some("P1.a.value"));
        assert_eq!(info.variable_at(144).as_deref(), Some("P1.refs[1]"));
        assert_eq!(info.variable_at(168).as_deref(), Some("P1.nodes[1]"));
        assert_eq!(info.variable_at(300).as_deref(), Some("P1.pt.x"));
        assert_eq!(info.variable_at(999), None);
        assert_eq!(info.variable_at(102), None, "inside a value");
    }

    /// A frame's pointers in wasm locals, read through [`Live::local`], and
    /// the locals of a recursive call, at offsets from the frame's start.
    struct Frame<'a> {
        mem: &'a [u8],
        locals: Vec<(u32, u32)>,
    }

    impl Live for Frame<'_> {
        fn read(&mut self, address: u32, size: u32) -> Option<Vec<u8>> {
            reader(self.mem)(address, size)
        }

        fn local(&mut self, index: u32) -> Option<u32> {
            self.locals
                .iter()
                .find(|(i, _)| *i == index)
                .map(|(_, v)| *v)
        }
    }

    fn frame_module() -> (DebugInfo, Vec<u8>) {
        let pt = TypeDesc::Struct {
            name: "Pt".to_string(),
            size: 8,
            fields: vec![field("x", 0, 2), field("y", 4, 2)],
        };
        let locals = crate::DebugLocals {
            version: crate::DEBUG_LOCALS_VERSION,
            functions: vec![crate::FuncLocals {
                defined_index: 7,
                locals: Vec::new(),
                memory: vec![leaf("buf", 400, SymType::Int)],
                arrays: Vec::new(),
                this_slot: None,
                pointers: vec![pointer("q", 404, Some(0), false)],
                local_pointers: vec![
                    crate::LocalPointer {
                        wasm_index: 0,
                        name: "v".to_string(),
                        target: Some(0),
                        by_ref: true,
                    },
                    crate::LocalPointer {
                        wasm_index: 1,
                        name: "p".to_string(),
                        target: Some(1),
                        by_ref: true,
                    },
                    crate::LocalPointer {
                        wasm_index: 2,
                        name: "r".to_string(),
                        target: Some(0),
                        by_ref: false,
                    },
                ],
                frame: Some(crate::FrameLocals {
                    slot: 5,
                    memory: vec![
                        leaf("trail[0]", 0, SymType::Int),
                        leaf("trail[1]", 4, SymType::Int),
                        Symbol {
                            named_type: Some(3),
                            ..leaf("gear", 12, SymType::Int)
                        },
                        leaf("pt.x", 16, SymType::Real),
                        leaf("pt.y", 20, SymType::Real),
                    ],
                    arrays: vec![ArraySym {
                        path: "trail".to_string(),
                        address: 0,
                        dimensions: vec![(0, 1)],
                        total_elements: 2,
                        elem_size: 4,
                        elem_ty: Some(SymType::Int),
                        global: false,
                        elem_type: None,
                    }],
                    pointers: vec![pointer("link", 8, Some(1), false)],
                    size: 24,
                    containers: vec![container("pt", 16, "Pt")],
                }),
                containers: Vec::new(),
            }],
            types: vec![
                TypeDesc::Scalar(SymType::Int),
                pt,
                TypeDesc::Scalar(SymType::Real),
                TypeDesc::Enum {
                    name: "Gear".to_string(),
                    storage: SymType::Int,
                    variants: vec![("Low".to_string(), 0), ("High".to_string(), 1)],
                },
            ],
        };
        let info = DebugInfo::from_wasm(&module_with_sections(&[(
            crate::DEBUG_LOCALS_SECTION,
            &locals.to_msgpack(),
        )]));
        assert!(info.problems().is_empty(), "{:?}", info.problems());
        let mut mem = vec![0u8; 1100];
        let mut put = |at: usize, v: u32| mem[at..at + 4].copy_from_slice(&v.to_le_bytes());
        put(200, 9);
        put(304, 2.5f32.to_bits());
        put(400, 3);
        put(404, 200);
        put(1000, 5);
        put(1004, 6);
        put(1008, 300);
        put(1016, 4.0f32.to_bits());
        (info, mem)
    }

    #[test]
    fn a_frame_s_pointers_are_read_from_its_locals() {
        let (info, mem) = frame_module();
        let live = &mut Frame {
            mem: &mem,
            locals: vec![(0, 200), (1, 300), (2, 0), (5, 1000)],
        };
        assert_eq!(
            at(info.follow_local(7, "v", None, live)),
            (200, SymType::Int)
        );
        assert_eq!(
            at(info.follow_local(7, "p.y", None, live)),
            (304, SymType::Real)
        );
        assert_eq!(
            info.follow_local(7, "r", None, live),
            Some(Place::Null {
                pointer: "r".to_string()
            })
        );
        assert_eq!(
            at(info.follow_local(7, "buf", None, live)),
            (400, SymType::Int)
        );
        assert_eq!(
            at(info.follow_local(7, "q^", None, live)),
            (200, SymType::Int)
        );
        assert_eq!(info.follow_local(8, "v", None, live), None, "no such frame");
    }

    /// The locals in the frame of one call: at the frame's start plus their
    /// offset, with the start the caller read from the frame's slot. Without
    /// it they are not placed at all.
    #[test]
    fn a_recursive_call_s_locals_are_placed_from_its_frame() {
        let (info, mem) = frame_module();
        let live = &mut Frame {
            mem: &mem,
            locals: vec![(5, 1000)],
        };
        assert_eq!(
            at(info.follow_local(7, "trail[1]", Some(1000), live)),
            (1004, SymType::Int)
        );
        assert_eq!(
            at(info.follow_local(7, "link^.x", Some(1000), live)),
            (300, SymType::Real)
        );
        assert_eq!(info.follow_local(7, "trail[1]", None, live), None);
        let loc = |path: &str| info.resolve_in_frame(7, path, 1000).map(|l| l.address);
        assert_eq!(loc("pt.y"), Some(1020));
        assert_eq!(loc("trail[0]"), Some(1000));
        assert_eq!(loc("TRAIL[1]"), Some(1004));
        assert_eq!(loc("trail[2]"), None, "past the array");
        assert_eq!(loc("buf"), None, "a static local is not in the frame");
        // An enumeration in the frame names its variant from the frames'
        // table, not the module's.
        let gear = info
            .resolve_in_frame(7, "gear", 1000)
            .expect("the enum local");
        assert_eq!(
            gear.named,
            Some(crate::TypeRef {
                table: crate::Table::Frames,
                id: 3
            })
        );
        assert_eq!(
            info.decode_at(&gear, &1i32.to_le_bytes()),
            VarValue::Enum {
                type_name: "Gear".to_string(),
                variant: Some("High".to_string()),
                raw: 1,
            }
        );
        assert_eq!(
            info.resolve_in_frame(7, "pt.y", u32::MAX),
            None,
            "past the memory"
        );
    }

    /// An address in a recursive call's frame is named from the live frames:
    /// the frame it is in, by position, and the local there. A STRUCT and
    /// its first leaf share a base: a pointer to the STRUCT names it, one
    /// to its first leaf, or to nothing in particular, names the leaf.
    #[test]
    fn an_address_in_a_frame_names_its_local() {
        let (info, _) = frame_module();
        let frames = [(7, 2000), (7, 1000)];
        let at =
            |address: u32, target: Option<&str>| info.frame_variable_at(address, &frames, target);
        assert_eq!(at(1004, None), Some((1, "trail[1]".to_string())));
        assert_eq!(at(1016, None), Some((1, "pt.x".to_string())));
        assert_eq!(at(1016, Some("Pt")), Some((1, "pt".to_string())));
        assert_eq!(at(1016, Some("Other")), Some((1, "pt.x".to_string())));
        assert_eq!(at(1008, None), Some((1, "link".to_string())));
        assert_eq!(at(2020, None), Some((0, "pt.y".to_string())));
        assert_eq!(
            at(400, None),
            Some((0, "buf".to_string())),
            "a static local"
        );
        assert_eq!(at(1023, None), None, "inside a value");
        assert_eq!(at(1024, None), None, "past the frame: 24 bytes");
        assert_eq!(at(500, None), None);
        assert_eq!(
            info.frame_variable_at(1004, &[(8, 1000)], None),
            None,
            "no such frame"
        );
        let (module, _) = module();
        assert_eq!(module.stack(), Some((4096, 8192)));
        assert_eq!(info.stack(), None);
    }

    /// A pointer into a call's locals is named through the live frames the
    /// reader gives: a `REF_TO Pt` as the STRUCT it points at, a `REF_TO
    /// INT` as the leaf, and nothing without the frames.
    #[test]
    fn a_pointer_into_a_frame_is_named_from_the_live_frames() {
        let (info, mut mem) = frame_module();
        // `link` holds the second frame's own `pt`.
        mem[1008..1012].copy_from_slice(&1016u32.to_le_bytes());
        struct WithFrames<'a>(Frame<'a>);
        impl Live for WithFrames<'_> {
            fn read(&mut self, address: u32, size: u32) -> Option<Vec<u8>> {
                self.0.read(address, size)
            }
            fn local(&mut self, index: u32) -> Option<u32> {
                self.0.local(index)
            }
            fn frames(&mut self) -> Vec<(u32, u32)> {
                vec![(7, 2000), (7, 1000)]
            }
        }
        // `r`, a REF_TO INT in wasm local 2, holds the second frame's `pt.x`.
        let locals = vec![(2, 1016), (5, 1000)];
        let live = &mut WithFrames(Frame {
            mem: &mem,
            locals: locals.clone(),
        });
        let Some(Place::Pointer(r)) = info.follow_local(7, "r", Some(1000), live) else {
            panic!("r is a pointer");
        };
        assert_eq!(r.variable, None, "no static name");
        assert_eq!(r.in_frame, Some((1, "pt.x".to_string())), "the leaf");
        let Some(Place::Pointer(link)) = info.follow_local(7, "link", Some(1000), live) else {
            panic!("link is a pointer");
        };
        assert_eq!(link.in_frame, Some((1, "pt".to_string())), "the STRUCT");

        let live = &mut Frame { mem: &mem, locals };
        let Some(Place::Pointer(r)) = info.follow_local(7, "r", Some(1000), live) else {
            panic!("r is a pointer");
        };
        assert_eq!(r.in_frame, None, "no frames given, no name");
    }

    /// A table a reader was not built for: widths its types have not,
    /// bounds spanning the whole of i64, sizes and addresses at the top of
    /// u32. Nothing may panic; what cannot be read is unavailable.
    #[test]
    fn a_hostile_table_does_not_panic() {
        let mut table = crate::DebugSymbols::new();
        table.symbols = vec![
            Symbol {
                path: "narrow".to_string(),
                address: 0,
                size: 1,
                ty: SymType::LInt,
                global: false,
                named_type: Some(7),
                bits: None,
            },
            Symbol {
                path: "last".to_string(),
                address: u32::MAX,
                size: u32::MAX,
                ty: SymType::String { capacity: u32::MAX },
                global: false,
                named_type: None,
                bits: None,
            },
        ];
        table.arrays = vec![ArraySym {
            path: "wide".to_string(),
            address: u32::MAX - 3,
            dimensions: vec![(i64::MIN, i64::MAX), (-1, 1)],
            total_elements: u32::MAX,
            elem_size: u32::MAX,
            elem_ty: None,
            global: false,
            elem_type: Some(u32::MAX),
        }];
        table.pointers = vec![pointer("p", u32::MAX - 1, Some(u32::MAX), true)];
        let info = DebugInfo::from_wasm(&module_with_sections(&[(
            crate::DEBUG_SYMBOLS_SECTION,
            &table.to_msgpack(),
        )]));
        let mem = vec![0u8; 64];
        let live = &mut reader(&mem);
        assert!(matches!(
            info.read_all_bytes(&mem)[0].1,
            VarValue::Unavailable(_)
        ));
        assert_eq!(
            decode(SymType::Int, &[1]),
            VarValue::Unavailable("1 bytes for a INT, which takes 4".to_string())
        );
        for path in [
            "narrow",
            "narrow^",
            "last",
            "wide[0]",
            "wide[0, 0]",
            "wide[-9223372036854775808, 1]",
            "wide[9223372036854775807, 1]^",
            "p",
            "p^",
            "p^.x",
        ] {
            let _ = info.resolve(path);
            let _ = info.follow(path, live);
        }
        assert_eq!(
            info.follow("wide[0, 0]", live),
            None,
            "no entry at the top of u32"
        );
        for address in [0, u32::MAX - 3, u32::MAX] {
            let _ = info.variable_at(address);
        }
        assert_eq!(
            crate::element_path("w", 5, &[(i64::MIN, i64::MAX)]),
            "w[5]",
            "a bound that cannot be counted is left as the flat index"
        );
    }

    #[test]
    fn a_caret_is_an_accessor() {
        assert_eq!(
            tokenize_accessors("^.x[1]^").unwrap(),
            vec![
                Access::Deref,
                Access::Field("x".to_string()),
                Access::Index(1),
                Access::Deref
            ]
        );
        assert_eq!(
            stems("P1.a.next^.value"),
            vec![
                ("P1.a.next^.value", ""),
                ("P1.a.next^", ".value"),
                ("P1.a.next", "^.value"),
                ("P1.a", ".next^.value"),
                ("P1", ".a.next^.value"),
            ]
        );
        assert!(stems("").is_empty());
    }
}
