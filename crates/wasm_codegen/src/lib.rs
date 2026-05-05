//! WebAssembly code generation for IEC 61131-3 programs.
//!
//! Uses the MIR pipeline: HIR → MIR → WASM (via `from_mir`).

pub mod component;
pub mod debug;
pub mod emit_expr;
pub mod emit_stmt;
pub mod mir_cast;

mod builtins;
mod graft;

#[cfg(test)]
pub mod tests;

use db::WorkspaceDataBase;
use mir::{
    MirModule,
    function::{MirExternFunction, MirFunction, MirLinkage, MirParam, MirParamKind, MirStorage},
    types::{MirElementary, MirType},
};
use rustc_hash::FxHashMap;
use wasm_encoder::{Instruction, ValType};

use self::emit_stmt::emit_stmts_with_return;

/// Generate a WASM module from a fully-lowered MirModule.
/// Needs the db to resolve Ident → string for export names.
pub fn generate_wasm(db: &dyn WorkspaceDataBase, module: &MirModule) -> wasm_encoder::Module {
    let mut wasm_gen = WasmGen::new(db, module);
    wasm_gen.emit_all();
    wasm_gen.finish()
}

/// Maps a MIR local variable to its WASM representation.
#[derive(Debug, Clone)]
pub(crate) enum LocalInfo {
    /// Scalar held in a WASM local.
    Scalar {
        index: u32,
        val_type: ValType,
        elem: MirElementary,
    },
    /// Memory-resident variable at a fixed address.
    Memory {
        address: u32,
        size: u32,
        align: u32,
        elem: Option<MirElementary>,
    },
    /// Pointer (VAR_IN_OUT) — i32 local holding an address.
    Pointer {
        index: u32,
        pointee_elem: Option<MirElementary>,
    },
    /// String parameter — two consecutive i32 locals (ptr, len).
    StringParam { ptr_index: u32, len_index: u32 },
    /// String in memory — two i32s at address (ptr at addr, len at addr+4).
    StringMemory { address: u32 },
}

struct WasmGen<'a> {
    db: &'a dyn WorkspaceDataBase,
    module: &'a MirModule,
    type_section: wasm_encoder::TypeSection,
    import_section: wasm_encoder::ImportSection,
    fn_section: wasm_encoder::FunctionSection,
    global_section: wasm_encoder::GlobalSection,
    export_section: wasm_encoder::ExportSection,
    code_section: wasm_encoder::CodeSection,
    extra_data_section: wasm_encoder::DataSection,
    next_type_idx: u32,
    /// MIR function index → wasm index: wasm requires all imports first, MIR
    /// may interleave them.
    index_remap: FxHashMap<u32, u32>,
    /// Builtin name (`f32.sin`) → wasm index of the grafted implementation.
    builtin_indices: FxHashMap<String, u32>,
}

/// WASM page size.
const WASM_PAGE: u32 = 65536;

/// Number of 64KiB pages the core module needs to cover its static allocations.
pub(crate) fn core_memory_pages(layout: &mir::memory::MirMemoryLayout) -> u64 {
    let total = layout.total_size();
    if total == 0 {
        1
    } else {
        total.div_ceil(WASM_PAGE) as u64
    }
}

impl<'a> WasmGen<'a> {
    fn new(db: &'a dyn WorkspaceDataBase, module: &'a MirModule) -> Self {
        // Import memory from `env` as the first entry of the import section.
        //
        // The main module imports memory (rather than defining it) so the
        // component wrapper can supply a memory via a helper core module that
        // is instantiated *before* canonical lowering. This lets canon
        // `Memory(idx)` options reference a core memory that exists at the
        // time of lowering — if main defined its own memory, memory would
        // only exist after main's instantiation, which happens *after*
        // lowering, breaking the canonical ABI ordering.
        let mut import_section = wasm_encoder::ImportSection::new();
        import_section.import(
            "env",
            "memory",
            wasm_encoder::EntityType::Memory(wasm_encoder::MemoryType {
                minimum: core_memory_pages(&module.memory_layout),
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            }),
        );

        Self {
            db,
            module,
            type_section: Default::default(),
            import_section,
            fn_section: Default::default(),
            global_section: Default::default(),
            export_section: Default::default(),
            code_section: Default::default(),
            extra_data_section: Default::default(),
            next_type_idx: 0,
            index_remap: FxHashMap::default(),
            builtin_indices: FxHashMap::default(),
        }
    }

    fn emit_all(&mut self) {
        // Pre-scan: which `wasm_builtins` exports any function references. The
        // graft goes between imports and user functions, so user indices skip
        // past it.
        let names_used = self.collect_builtin_names();

        // Index layout in the output module:
        //   [imports] [grafted builtins] [user functions]
        let n_imports = self.module.extern_functions.len() as u32;
        let n_builtins = self.preflight_graft_count(&names_used);

        let mut index_remap: FxHashMap<u32, u32> = FxHashMap::default();
        let mut wasm_idx: u32 = 0;

        // 1. Assign WASM indices for all imports.
        for ext_fn in &self.module.extern_functions {
            index_remap.insert(ext_fn.index, wasm_idx);
            wasm_idx += 1;
        }

        // 2. Reserve indices for grafted builtins; the graft runs after the
        //    imports.
        wasm_idx += n_builtins;

        // 3. Assign WASM indices for all local (user) functions.
        for func in &self.module.functions {
            index_remap.insert(func.index, wasm_idx);
            wasm_idx += 1;
        }
        let _ = n_imports;
        self.index_remap = index_remap;

        // 4. Emit imports.
        for ext_fn in &self.module.extern_functions {
            self.emit_import(ext_fn);
        }

        // 5. Graft builtins. Their function indices land in
        //    [n_imports, n_imports + n_builtins) — exactly the slot we
        //    reserved above.
        if !names_used.is_empty() {
            let plan = crate::graft::graft_builtins(
                names_used.iter().map(String::as_str),
                &mut self.type_section,
                &mut self.fn_section,
                &mut self.code_section,
                &mut self.global_section,
                &mut self.extra_data_section,
                &mut self.next_type_idx,
                /* base_fn_idx = */ self.module.extern_functions.len() as u32,
            );
            self.builtin_indices = plan.name_to_wasm_idx;
        }

        // 6. Emit user functions.
        for func in &self.module.functions {
            self.emit_function(func);
        }
    }

    /// Walk every user function's body and collect the set of WASM
    /// instruction names that hit `BUILTIN_NAMES`. Returned as a sorted
    /// `Vec` for deterministic graft order.
    fn collect_builtin_names(&self) -> Vec<String> {
        use mir::stmt::MirStmt;
        let mut found = rustc_hash::FxHashSet::default();
        fn walk(stmts: &[MirStmt], found: &mut rustc_hash::FxHashSet<String>) {
            for stmt in stmts {
                match stmt {
                    MirStmt::WasmIntrinsic { instruction, .. } => {
                        if crate::builtins::lookup(instruction.as_str()).is_some() {
                            found.insert(instruction.to_string());
                        }
                    }
                    MirStmt::If {
                        then_body,
                        else_ifs,
                        else_body,
                        ..
                    } => {
                        walk(then_body, found);
                        for (_, body) in else_ifs {
                            walk(body, found);
                        }
                        if let Some(b) = else_body {
                            walk(b, found);
                        }
                    }
                    MirStmt::Case { arms, else_body, .. } => {
                        for arm in arms {
                            walk(&arm.body, found);
                        }
                        if let Some(b) = else_body {
                            walk(b, found);
                        }
                    }
                    MirStmt::For { body, .. }
                    | MirStmt::While { body, .. }
                    | MirStmt::Repeat { body, .. } => walk(body, found),
                    _ => {}
                }
            }
        }
        for func in &self.module.functions {
            walk(&func.body, &mut found);
        }
        let mut v: Vec<String> = found.into_iter().collect();
        v.sort();
        v
    }

    /// Count how many bundle functions a graft of these names would pull
    /// in, without actually grafting. Mirrors the closure walk in `graft`.
    fn preflight_graft_count(&self, names: &[String]) -> u32 {
        if names.is_empty() {
            return 0;
        }
        let mut seen = rustc_hash::FxHashSet::default();
        for name in names {
            if let Some(root) = crate::builtins::lookup(name) {
                for idx in crate::builtins::transitive_closure(root) {
                    seen.insert(idx);
                }
            }
        }
        seen.len() as u32
    }

    fn emit_import(&mut self, ext_fn: &MirExternFunction) {
        let (params, results) = build_signature(&ext_fn.params, &ext_fn.return_type);

        let type_idx = self.next_type_idx;
        self.type_section
            .ty()
            .function(params.iter().copied(), results.iter().copied());
        self.next_type_idx += 1;

        self.import_section.import(
            ext_fn.module.as_str(),
            ext_fn.import_name.as_str(),
            wasm_encoder::EntityType::Function(type_idx),
        );

        // Re-export the import so it can be called by name from tests
        let export_name = ext_fn.name.text(self.db).to_string();
        let wasm_idx = self
            .index_remap
            .get(&ext_fn.index)
            .copied()
            .unwrap_or(ext_fn.index);
        self.export_section
            .export(&export_name, wasm_encoder::ExportKind::Func, wasm_idx);
    }

    fn emit_function(&mut self, func: &MirFunction) {
        let (params, results) = build_signature(&func.params, &func.return_type);

        // Register type
        let type_idx = self.next_type_idx;
        self.type_section
            .ty()
            .function(params.iter().copied(), results.iter().copied());
        self.next_type_idx += 1;

        // Register function
        self.fn_section.function(type_idx);

        // Export every public function.
        if func.linkage == MirLinkage::Export {
            let export_name = func
                .export_name
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| func.name.text(self.db).to_string());
            let wasm_idx = self
                .index_remap
                .get(&func.index)
                .copied()
                .unwrap_or(func.index);
            self.export_section
                .export(&export_name, wasm_encoder::ExportKind::Func, wasm_idx);
        }

        // Build local map from params + locals
        let local_map = build_local_map(func);

        // Build extra locals (non-parameter WASM locals)
        let mut extra_locals: Vec<(u32, ValType)> = Vec::new();
        // Return value local
        if let Some(ref ret_ty) = func.return_type
            && let Some(vt) = mir_type_to_val_type(ret_ty)
        {
            extra_locals.push((1, vt));
        }
        // Local variables that are scalars
        for local in &func.locals {
            if let MirStorage::Scalar { .. } = local.storage
                && let Some(vt) = mir_type_to_val_type(&local.ty)
            {
                extra_locals.push((1, vt));
            }
        }

        // Emit function body
        let mut wasm_func = wasm_encoder::Function::new(extra_locals);

        // Find the return slot for scalar-or-string returns.
        // Use origin_name because methods store return locals under the bare method name,
        // while func.name is the qualified "FB$Method" name.
        enum ReturnSlot {
            Scalar(u32),
            StringMem(u32),
        }
        let return_slot: Option<ReturnSlot> = if func.return_type.is_some() {
            local_map
                .get(&func.origin_name)
                .and_then(|info| match info {
                    LocalInfo::Scalar { index, .. } => Some(ReturnSlot::Scalar(*index)),
                    LocalInfo::StringMemory { address } => Some(ReturnSlot::StringMem(*address)),
                    _ => None,
                })
        } else {
            None
        };
        let return_local = match &return_slot {
            Some(ReturnSlot::Scalar(idx)) => Some(*idx),
            _ => None,
        };

        // Build remapped function indices for call instructions
        let remapped_fn_indices: FxHashMap<_, _> = self
            .module
            .function_indices
            .iter()
            .map(|(name, &mir_idx)| {
                let wasm_idx = self.index_remap.get(&mir_idx).copied().unwrap_or(mir_idx);
                (*name, wasm_idx)
            })
            .collect();

        // Emit statements
        emit_stmts_with_return(
            &mut wasm_func,
            &func.body,
            &local_map,
            &remapped_fn_indices,
            &self.builtin_indices,
            return_local,
        );

        // Push return value at function end.
        match return_slot {
            Some(ReturnSlot::Scalar(ret_idx)) => {
                wasm_func.instruction(&Instruction::LocalGet(ret_idx));
            }
            Some(ReturnSlot::StringMem(addr)) => {
                // Multi-value return: push (ptr, len) read from the return slot.
                wasm_func.instruction(&Instruction::I32Const(addr as i32));
                wasm_func.instruction(&Instruction::I32Load(wasm_encoder::MemArg {
                    offset: 0,
                    align: 2,
                    memory_index: 0,
                }));
                wasm_func.instruction(&Instruction::I32Const(addr as i32 + 4));
                wasm_func.instruction(&Instruction::I32Load(wasm_encoder::MemArg {
                    offset: 0,
                    align: 2,
                    memory_index: 0,
                }));
            }
            None => {}
        }

        // End function
        wasm_func.instruction(&Instruction::End);
        self.code_section.function(&wasm_func);
    }

    fn finish(mut self) -> wasm_encoder::Module {
        // Re-export the imported memory, for tests and inspection tools.
        self.export_section
            .export("memory", wasm_encoder::ExportKind::Memory, 0);

        let mut module = wasm_encoder::Module::new();
        module.section(&self.type_section);
        module.section(&self.import_section);
        module.section(&self.fn_section);
        // Global section is only emitted when builtins were grafted (the
        // bundle's stack pointer + static markers). Empty otherwise.
        if !self.builtin_indices.is_empty() {
            module.section(&self.global_section);
        }
        module.section(&self.export_section);
        module.section(&self.code_section);

        // IEC string data and grafted data segments both target memory 0 and
        // never overlap (the IEC layout starts at `BUILTIN_RESERVED_FLOOR`).
        let has_iec_data = !self.module.string_data.is_empty();
        let has_builtin_data = !self.builtin_indices.is_empty();
        if has_iec_data || has_builtin_data {
            let mut data_section = self.extra_data_section;
            for (offset, bytes) in &self.module.string_data {
                data_section.active(
                    0,
                    &wasm_encoder::ConstExpr::i32_const(*offset as i32),
                    bytes.iter().copied(),
                );
            }
            module.section(&data_section);
        }

        module
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Build WASM function signature from MIR params and return type.
fn build_signature(
    params: &[MirParam],
    return_type: &Option<MirType>,
) -> (Vec<ValType>, Vec<ValType>) {
    let mut wasm_params = Vec::new();
    for param in params {
        match param.kind {
            MirParamKind::This | MirParamKind::InOut | MirParamKind::Output => {
                wasm_params.push(ValType::I32); // pointer
            }
            MirParamKind::Input => {
                match &param.ty {
                    MirType::String => {
                        wasm_params.push(ValType::I32); // ptr
                        wasm_params.push(ValType::I32); // len
                    }
                    ty => {
                        if let Some(vt) = mir_type_to_val_type(ty) {
                            wasm_params.push(vt);
                        } else {
                            wasm_params.push(ValType::I32); // pointer for memory-resident
                        }
                    }
                }
            }
        }
    }

    let results = match return_type {
        // A STRING return flattens to (ptr, len) per the component-model
        // canonical ABI. The function body pushes the two i32s in that order
        // at the epilogue (see `emit_function`).
        Some(MirType::String) => vec![ValType::I32, ValType::I32],
        Some(ty) => mir_type_to_val_type(ty)
            .map(|vt| vec![vt])
            .unwrap_or_default(),
        None => Vec::new(),
    };

    (wasm_params, results)
}

/// Build LocalInfo map from a MirFunction's params and locals.
pub(crate) fn build_local_map(
    func: &MirFunction,
) -> FxHashMap<hir::hir_def::interned::identifier::Ident, LocalInfo> {
    let mut map = FxHashMap::default();
    let mut param_idx: u32 = 0;

    // Parameters — mapped by their position in the WASM signature
    for param in &func.params {
        match param.kind {
            MirParamKind::This => {
                map.insert(
                    param.name,
                    LocalInfo::Pointer {
                        index: param_idx,
                        pointee_elem: None,
                    },
                );
                param_idx += 1;
            }
            MirParamKind::InOut | MirParamKind::Output => {
                // For InOut/Output, the pointee is the actual type (unwrap Pointer wrapper)
                let pointee_elem = match &param.ty {
                    MirType::Pointer(inner) => match inner.as_ref() {
                        MirType::Elementary(e) => Some(*e),
                        _ => None,
                    },
                    MirType::Elementary(e) => Some(*e),
                    _ => None,
                };
                map.insert(
                    param.name,
                    LocalInfo::Pointer {
                        index: param_idx,
                        pointee_elem,
                    },
                );
                param_idx += 1;
            }
            MirParamKind::Input => {
                match &param.ty {
                    MirType::String => {
                        // String params take 2 WASM params (ptr, len)
                        map.insert(
                            param.name,
                            LocalInfo::StringParam {
                                ptr_index: param_idx,
                                len_index: param_idx + 1,
                            },
                        );
                        param_idx += 2;
                    }
                    ty => {
                        let elem = match ty {
                            MirType::Elementary(e) => *e,
                            MirType::Pointer(_) => MirElementary::Int,
                            _ => MirElementary::Int,
                        };
                        let val_type = mir_elementary_to_val_type(elem);
                        map.insert(
                            param.name,
                            LocalInfo::Scalar {
                                index: param_idx,
                                val_type,
                                elem,
                            },
                        );
                        param_idx += 1;
                    }
                }
            }
        }
    }

    // Locals
    for local in &func.locals {
        match local.storage {
            MirStorage::Scalar { local_index } => {
                let elem = match &local.ty {
                    MirType::Elementary(e) => *e,
                    _ => MirElementary::Int,
                };
                let val_type = mir_elementary_to_val_type(elem);
                map.insert(
                    local.name,
                    LocalInfo::Scalar {
                        index: local_index,
                        val_type,
                        elem,
                    },
                );
            }
            MirStorage::Memory {
                address,
                size,
                align,
            } => match &local.ty {
                MirType::String => {
                    map.insert(local.name, LocalInfo::StringMemory { address });
                }
                ty => {
                    let elem = match ty {
                        MirType::Elementary(e) => Some(*e),
                        _ => None,
                    };
                    map.insert(
                        local.name,
                        LocalInfo::Memory {
                            address,
                            size,
                            align,
                            elem,
                        },
                    );
                }
            },
        }
    }

    map
}

/// Map MirType to a WASM ValType (None for memory-resident types).
pub(crate) fn mir_type_to_val_type(ty: &MirType) -> Option<ValType> {
    match ty {
        MirType::Elementary(e) => Some(mir_elementary_to_val_type(*e)),
        MirType::Enum(_) | MirType::Subrange(_) => Some(ValType::I32),
        MirType::Pointer(_) => Some(ValType::I32),
        _ => None, // Memory-resident types don't have a single ValType
    }
}

/// Map MirElementary to WASM ValType.
pub(crate) fn mir_elementary_to_val_type(elem: MirElementary) -> ValType {
    if elem.is_float() {
        if elem.is_64bit() {
            ValType::F64
        } else {
            ValType::F32
        }
    } else if elem.is_64bit() {
        ValType::I64
    } else {
        ValType::I32
    }
}
