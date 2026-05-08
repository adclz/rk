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

use std::cell::Cell;

use db::WorkspaceDataBase;
use mir::{
    MirModule,
    expr::{MirArgKind, MirCall, MirExpr},
    function::{MirExternFunction, MirFunction, MirLinkage, MirParam, MirParamKind, MirStorage},
    stmt::MirStmt,
    types::{MirElementary, MirType},
};
use rustc_hash::FxHashMap;
use wasm_encoder::{Instruction, ValType};

use self::emit_expr::{SNAPSHOT_CTX, StringSnapshotCtx};
use self::emit_stmt::emit_stmts_with_return;

/// Capacity of the per-call-site scratch slots that snapshot nested
/// STRING-returning call results. Matches `mir::types::DEFAULT_STRING_CAPACITY`
/// - sized to fit any plain-`STRING` producer's output. Producers declared
/// `STRING[N]` with N > 80 would silently truncate snapshots; the typical
/// stdlib operates well below that threshold.
const STRING_SCRATCH_CAPACITY: u32 = 80;
const STRING_SCRATCH_SLOT_SIZE: u32 = (4 + STRING_SCRATCH_CAPACITY + 3) & !3;

fn count_nested_string_calls_stmts(stmts: &[MirStmt]) -> u32 {
    let mut total = 0;
    for stmt in stmts {
        total += count_nested_string_calls_stmt(stmt);
    }
    total
}

fn count_nested_string_calls_stmt(stmt: &MirStmt) -> u32 {
    match stmt {
        MirStmt::Assign { value, .. } => count_nested_string_calls_expr(value),
        MirStmt::Return => 0,
        MirStmt::If {
            condition,
            then_body,
            else_ifs,
            else_body,
        } => {
            let mut n = count_nested_string_calls_expr(condition);
            n += count_nested_string_calls_stmts(then_body);
            for (cond, body) in else_ifs {
                n += count_nested_string_calls_expr(cond);
                n += count_nested_string_calls_stmts(body);
            }
            if let Some(eb) = else_body {
                n += count_nested_string_calls_stmts(eb);
            }
            n
        }
        MirStmt::Case {
            selector,
            arms,
            else_body,
        } => {
            let mut n = count_nested_string_calls_expr(selector);
            for arm in arms {
                n += count_nested_string_calls_stmts(&arm.body);
            }
            if let Some(eb) = else_body {
                n += count_nested_string_calls_stmts(eb);
            }
            n
        }
        MirStmt::For {
            start,
            end,
            step,
            body,
            ..
        } => {
            let mut n = count_nested_string_calls_expr(start);
            n += count_nested_string_calls_expr(end);
            n += count_nested_string_calls_expr(step);
            n += count_nested_string_calls_stmts(body);
            n
        }
        MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
            count_nested_string_calls_expr(condition) + count_nested_string_calls_stmts(body)
        }
        MirStmt::Call(call) => count_nested_in_call(call),
        MirStmt::FbCall { input_writes, .. } => {
            // Input value expressions may contain nested STRING calls.
            let mut n = 0;
            for (_, value, _) in input_writes {
                n += count_nested_string_calls_expr(value);
            }
            n
        }
        MirStmt::MemStore { .. }
        | MirStmt::WasmIntrinsic { .. }
        | MirStmt::Exit
        | MirStmt::Continue
        | MirStmt::DebugTrap { .. } => 0,
    }
}

fn count_nested_string_calls_expr(expr: &MirExpr) -> u32 {
    match expr {
        MirExpr::Call(call) => count_nested_in_call(call),
        MirExpr::BinOp { lhs, rhs, .. } => {
            count_nested_string_calls_expr(lhs) + count_nested_string_calls_expr(rhs)
        }
        MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => {
            count_nested_string_calls_expr(expr)
        }
        _ => 0,
    }
}

fn count_nested_in_call(call: &MirCall) -> u32 {
    let mut n = 0;
    for arg in &call.args {
        if matches!(arg.kind, MirArgKind::ByValue) {
            // This arg position needs a snapshot if the value is itself a
            // STRING-returning Call.
            if let MirExpr::Call(inner) = &arg.value
                && matches!(inner.return_type, MirType::String { .. })
            {
                n += 1;
            }
        }
        // An arg's expression tree may contain its own nested STRING calls.
        n += count_nested_string_calls_expr(&arg.value);
    }
    n
}

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
    Scalar { index: u32, elem: MirElementary },
    /// Memory-resident variable at a fixed address.
    Memory {
        address: u32,
        elem: Option<MirElementary>,
    },
    /// Pointer (VAR_IN_OUT) - i32 local holding an address.
    Pointer {
        index: u32,
        pointee_elem: Option<MirElementary>,
    },
    /// String parameter - two consecutive i32 locals (ptr, len).
    StringParam { ptr_index: u32, len_index: u32 },
    /// STRING `VAR_IN_OUT`: the function may mutate the caller's buffer,
    /// writing the length to `*addr` and bytes to `addr + 4`; `cap_index`
    /// clamps writes.
    StringInOutParam { addr_index: u32, cap_index: u32 },
    /// String in memory. Layout starting at `address`:
    ///   `addr + 0..4`  - `ptr` (i32), points at the embedded buffer
    ///   `addr + 4..8`  - `len` (i32), current byte length, ≤ `capacity`
    ///   `addr + 8..8+capacity` - embedded buffer
    /// `capacity` comes from the declared `STRING[N]` (or
    /// `DEFAULT_STRING_CAPACITY` for plain `STRING`). The header is
    /// initialized at function entry so `ptr` always points at this
    /// variable's own buffer; assignment is a bounded `memcpy` into the
    /// buffer rather than a header alias.
    StringMemory { address: u32, capacity: u32 },
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
    /// Next free address for STRING snapshot scratch slots, past the MIR
    /// static layout.
    string_scratch_floor: Cell<u32>,
}

/// Sum the snapshot scratch needs across every function in the module.
fn module_total_scratch_slots(module: &MirModule) -> u32 {
    let mut total = 0;
    for func in &module.functions {
        total += count_nested_string_calls_stmts(&func.body);
    }
    total
}

/// WASM page size.
const WASM_PAGE: u32 = 65536;

/// Number of 64KiB pages the core module needs to cover its static
/// allocations plus per-call-site STRING snapshot scratch slots.
pub(crate) fn core_memory_pages(module: &MirModule) -> u64 {
    let static_total = module.memory_layout.total_size();
    let scratch_total = module_total_scratch_slots(module) * STRING_SCRATCH_SLOT_SIZE;
    let total = static_total + scratch_total;
    if total == 0 {
        1
    } else {
        total.div_ceil(WASM_PAGE) as u64
    }
}

impl<'a> WasmGen<'a> {
    fn new(db: &'a dyn WorkspaceDataBase, module: &'a MirModule) -> Self {
        // Memory is imported from `env`, first in the import section, so a host
        // supplies it.
        let mut import_section = wasm_encoder::ImportSection::new();
        import_section.import(
            "env",
            "memory",
            wasm_encoder::EntityType::Memory(wasm_encoder::MemoryType {
                minimum: core_memory_pages(module),
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            }),
        );

        let scratch_floor = Cell::new(module.memory_layout.total_size());

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
            string_scratch_floor: scratch_floor,
        }
    }

    /// Allocate a fresh per-call-site STRING snapshot scratch slot.
    /// Returns the base address (where the 4-byte length prefix lives).
    fn alloc_scratch_slot(&self) -> u32 {
        let addr = self.string_scratch_floor.get();
        self.string_scratch_floor
            .set(addr + STRING_SCRATCH_SLOT_SIZE);
        addr
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
        //    [n_imports, n_imports + n_builtins) - exactly the slot we
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

        // Force-include `rk.str_assign` whenever any function has a STRING
        // local - every `string_var := <expr>` assignment lowers to a call
        // into this helper. The pre-pass needs to know in advance so the
        // helper is grafted alongside math intrinsics. Also force-include
        // when *any* function nests STRING-returning calls, since the
        // codegen-driven snapshot dance dispatches through the same helper.
        let any_string_local = self.module.functions.iter().any(|f| {
            f.locals
                .iter()
                .any(|l| matches!(l.ty, MirType::String { .. }))
        });
        let any_nested_string_call = self
            .module
            .functions
            .iter()
            .any(|f| count_nested_string_calls_stmts(&f.body) > 0);
        if (any_string_local || any_nested_string_call)
            && crate::builtins::lookup("rk.str_assign").is_some()
        {
            found.insert("rk.str_assign".to_string());
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

        // Build local map from params + locals.
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

        // STRING snapshot dance temps (ptr_tmp, len_tmp) - only allocated
        // when the function actually contains nested STRING-returning calls.
        let nested_str_count = count_nested_string_calls_stmts(&func.body);
        let snapshot_local_indices = if nested_str_count > 0 {
            let next_idx = (params.len() as u32)
                + extra_locals.iter().map(|(c, _)| *c).sum::<u32>();
            extra_locals.push((2, ValType::I32));
            Some((next_idx, next_idx + 1))
        } else {
            None
        };

        // One scratch slot per nested STRING call, past the MIR static layout.
        let scratch_slots: Vec<u32> = (0..nested_str_count)
            .map(|_| self.alloc_scratch_slot())
            .collect();

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
                    LocalInfo::StringMemory { address, .. } => {
                        Some(ReturnSlot::StringMem(*address))
                    }
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

        // The per-function snapshot context `emit_call` consults for nested
        // STRING-returning calls.
        let prev_ctx = if let Some((ptr_tmp, len_tmp)) = snapshot_local_indices {
            let str_assign_idx = self
                .builtin_indices
                .get("rk.str_assign")
                .copied()
                .expect(
                    "rk.str_assign must be grafted whenever a function nests STRING-returning calls",
                );
            let ctx = StringSnapshotCtx {
                slots: scratch_slots,
                slot_capacity: STRING_SCRATCH_CAPACITY,
                next_slot: 0,
                ptr_tmp,
                len_tmp,
                str_assign_idx,
            };
            SNAPSHOT_CTX.with(|cell| cell.replace(Some(ctx)))
        } else {
            SNAPSHOT_CTX.with(|cell| cell.replace(None))
        };

        // Emit statements
        emit_stmts_with_return(
            &mut wasm_func,
            &func.body,
            &local_map,
            &remapped_fn_indices,
            &self.builtin_indices,
            return_local,
        );

        // Restore the prior context.
        SNAPSHOT_CTX.with(|cell| cell.replace(prev_ctx));

        // Push return value at function end.
        match return_slot {
            Some(ReturnSlot::Scalar(ret_idx)) => {
                wasm_func.instruction(&Instruction::LocalGet(ret_idx));
            }
            Some(ReturnSlot::StringMem(addr)) => {
                // Multi-value return: push (ptr, len). Layout: 4-byte
                // length at `addr`, embedded buffer starts at `addr + 4`.
                // Push the buffer base as ptr (constant, no load) then
                // load the length.
                wasm_func.instruction(&Instruction::I32Const(addr as i32 + 4));
                wasm_func.instruction(&Instruction::I32Const(addr as i32));
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
            MirParamKind::InOut | MirParamKind::Output
                if matches!(&param.ty, MirType::Pointer(inner) if matches!(inner.as_ref(), MirType::String { .. })) =>
            {
                // STRING `VAR_IN_OUT` / `VAR_OUTPUT` is (addr, cap), so mutators
                // can clamp writes.
                wasm_params.push(ValType::I32); // addr
                wasm_params.push(ValType::I32); // cap
            }
            MirParamKind::This | MirParamKind::InOut | MirParamKind::Output => {
                wasm_params.push(ValType::I32); // pointer
            }
            MirParamKind::Input => {
                match &param.ty {
                    MirType::String { .. } => {
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
        // canonical ABI. The function body pushes the two i32s in that
        // order at the epilogue (see `emit_function`).
        Some(MirType::String { .. }) => vec![ValType::I32, ValType::I32],
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

    // Parameters - mapped by their position in the WASM signature
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
                // STRING `VAR_IN_OUT` / `VAR_OUTPUT` flatten to (addr, cap) (see
                // `build_signature`).
                if let MirType::Pointer(inner) = &param.ty
                    && matches!(inner.as_ref(), MirType::String { .. })
                {
                    map.insert(
                        param.name,
                        LocalInfo::StringInOutParam {
                            addr_index: param_idx,
                            cap_index: param_idx + 1,
                        },
                    );
                    param_idx += 2;
                    continue;
                }

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
                    MirType::String { .. } => {
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
                        map.insert(
                            param.name,
                            LocalInfo::Scalar {
                                index: param_idx,
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
                map.insert(
                    local.name,
                    LocalInfo::Scalar {
                        index: local_index,
                        elem,
                    },
                );
            }
            MirStorage::Memory { address, .. } => match &local.ty {
                MirType::String { capacity } => {
                    map.insert(
                        local.name,
                        LocalInfo::StringMemory {
                            address,
                            capacity: *capacity,
                        },
                    );
                }
                ty => {
                    let elem = match ty {
                        MirType::Elementary(e) => Some(*e),
                        _ => None,
                    };
                    map.insert(local.name, LocalInfo::Memory { address, elem });
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
