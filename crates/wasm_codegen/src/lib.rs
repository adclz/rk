//! WebAssembly code generation for IEC 61131-3 programs.
//!
//! Uses the MIR pipeline: HIR → MIR → WASM (via `from_mir`).

pub mod emit_expr;
pub mod emit_stmt;
pub mod mir_cast;

mod builtins;
mod graft;

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
/// STRING-returning call results: the default, so any plain-`STRING`
/// producer's output fits.
const STRING_SCRATCH_CAPACITY: u32 = mir::types::DEFAULT_STRING_CAPACITY;
const STRING_SCRATCH_SLOT_SIZE: u32 = (4 + STRING_SCRATCH_CAPACITY + 3) & !3;

/// Collect every `Call` callee's Ident → text, for the unresolved-callee
/// panic.
fn walk_stmts_for_callees(
    stmts: &[MirStmt],
    db: &dyn WorkspaceDataBase,
    names: &mut FxHashMap<hir::hir_def::interned::identifier::Ident, String>,
) {
    fn add(
        call: &MirCall,
        db: &dyn WorkspaceDataBase,
        names: &mut FxHashMap<hir::hir_def::interned::identifier::Ident, String>,
    ) {
        names
            .entry(call.callee)
            .or_insert_with(|| call.callee.text(db).to_string());
        for arg in &call.args {
            walk_expr_for_callees(&arg.value, db, names);
        }
    }
    for stmt in stmts {
        match stmt {
            MirStmt::Call(call) => add(call, db, names),
            MirStmt::Assign { value, .. } => walk_expr_for_callees(value, db, names),
            MirStmt::If {
                condition,
                then_body,
                else_ifs,
                else_body,
            } => {
                walk_expr_for_callees(condition, db, names);
                walk_stmts_for_callees(then_body, db, names);
                for (c, b) in else_ifs {
                    walk_expr_for_callees(c, db, names);
                    walk_stmts_for_callees(b, db, names);
                }
                if let Some(eb) = else_body {
                    walk_stmts_for_callees(eb, db, names);
                }
            }
            MirStmt::Case {
                selector,
                arms,
                else_body,
            } => {
                walk_expr_for_callees(selector, db, names);
                for arm in arms {
                    // A STRING label's test is a call (`str.byte_cmp`), so patterns
                    // are scanned too.
                    for pattern in &arm.patterns {
                        if let mir::stmt::MirCasePattern::Test(test) = pattern {
                            walk_expr_for_callees(test, db, names);
                        }
                    }
                    walk_stmts_for_callees(&arm.body, db, names);
                }
                if let Some(eb) = else_body {
                    walk_stmts_for_callees(eb, db, names);
                }
            }
            MirStmt::For {
                start,
                end,
                step,
                body,
                ..
            } => {
                walk_expr_for_callees(start, db, names);
                walk_expr_for_callees(end, db, names);
                walk_expr_for_callees(step, db, names);
                walk_stmts_for_callees(body, db, names);
            }
            MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
                walk_expr_for_callees(condition, db, names);
                walk_stmts_for_callees(body, db, names);
            }
            MirStmt::FbCall { input_writes, .. } => {
                for (_, v, _) in input_writes {
                    walk_expr_for_callees(v, db, names);
                }
            }
            MirStmt::Raise { message } => walk_expr_for_callees(message, db, names),
            _ => {}
        }
    }
}

fn walk_expr_for_callees(
    expr: &MirExpr,
    db: &dyn WorkspaceDataBase,
    names: &mut FxHashMap<hir::hir_def::interned::identifier::Ident, String>,
) {
    match expr {
        MirExpr::Call(call) => {
            names
                .entry(call.callee)
                .or_insert_with(|| call.callee.text(db).to_string());
            for arg in &call.args {
                walk_expr_for_callees(&arg.value, db, names);
            }
        }
        MirExpr::BinOp { lhs, rhs, .. } => {
            walk_expr_for_callees(lhs, db, names);
            walk_expr_for_callees(rhs, db, names);
        }
        MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => {
            walk_expr_for_callees(expr, db, names);
        }
        _ => {}
    }
}

/// Allocate the per-`For` end/step snapshot locals a body needs (IEC:
/// evaluated once at entry), returning the index pairs in emitter order.
/// Lane-typed.
fn alloc_for_scratch(
    body: &[MirStmt],
    params_len: u32,
    extra_locals: &mut Vec<(u32, ValType)>,
) -> std::collections::VecDeque<(Option<u32>, Option<u32>)> {
    let mut next = params_len + extra_locals.iter().map(|(c, _)| *c).sum::<u32>();
    crate::emit_stmt::for_scratch_requests(body)
        .into_iter()
        .map(|req| {
            let vt = if req.is_64 { ValType::I64 } else { ValType::I32 };
            let mut take = |need: bool| {
                need.then(|| {
                    let idx = next;
                    next += 1;
                    extra_locals.push((1, vt));
                    idx
                })
            };
            (take(req.need_end), take(req.need_step))
        })
        .collect()
}

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
        MirStmt::Raise { message } => count_nested_string_calls_expr(message),
        MirStmt::MemStore { .. }
        | MirStmt::WasmIntrinsic { .. }
        | MirStmt::Exit
        | MirStmt::Continue
        | MirStmt::DebugTrap { .. } => 0,
    }
}

/// Whether any function assigns to a buffer-backed STRING *field* or *global*.
/// These lower to `rk.str_assign` just like STRING-local assignments, so the
/// helper must be grafted even when the module has no STRING locals (otherwise
/// `emit_string_assign` hits an ungrafted index). STRING locals are detected
/// separately by the `any_string_local` scan.
fn module_assigns_static_string(module: &MirModule) -> bool {
    module
        .functions
        .iter()
        .any(|f| stmts_assign_static_string(&f.body))
}

fn stmts_assign_static_string(stmts: &[MirStmt]) -> bool {
    stmts.iter().any(stmt_assigns_static_string)
}

fn stmt_assigns_static_string(stmt: &MirStmt) -> bool {
    match stmt {
        MirStmt::Assign { target, .. } => place_is_static_string(target),
        // An FB call's STRING input writes and output reads both route through
        // `rk.str_assign` (emit_fb_call), so they arm the graft like any other
        // static-string assignment. Without this, a caller writing a literal
        // into a STRING input — with no other string activity in the module —
        // compiled to the `.expect` in emit_stmt instead of a module.
        MirStmt::FbCall {
            input_writes,
            output_reads,
            ..
        } => {
            input_writes
                .iter()
                .any(|(_, _, ty)| matches!(ty, MirType::String { .. }))
                || output_reads
                    .iter()
                    .any(|(_, _, ty, _)| matches!(ty, MirType::String { .. }))
        }
        MirStmt::If {
            then_body,
            else_ifs,
            else_body,
            ..
        } => {
            stmts_assign_static_string(then_body)
                || else_ifs.iter().any(|(_, b)| stmts_assign_static_string(b))
                || else_body
                    .as_ref()
                    .is_some_and(|b| stmts_assign_static_string(b))
        }
        MirStmt::Case {
            arms, else_body, ..
        } => {
            arms.iter().any(|a| stmts_assign_static_string(&a.body))
                || else_body
                    .as_ref()
                    .is_some_and(|b| stmts_assign_static_string(b))
        }
        MirStmt::For { body, .. } => stmts_assign_static_string(body),
        MirStmt::While { body, .. } | MirStmt::Repeat { body, .. } => {
            stmts_assign_static_string(body)
        }
        _ => false,
    }
}

/// A STRING param, possibly by reference: `VAR_IN_OUT`/`VAR_OUTPUT` strings lower
/// to `Pointer(String)`. These can be assignment targets routed through
/// `rk.str_assign`, so they must arm the graft trigger.
fn param_is_stringish(ty: &MirType) -> bool {
    match ty {
        MirType::String { .. } => true,
        MirType::Pointer(inner) => matches!(inner.as_ref(), MirType::String { .. }),
        _ => false,
    }
}

/// A STRING field/global/element place (not a `Local` — local strings are
/// covered by `any_string_local`).
fn place_is_static_string(place: &mir::expr::MirPlace) -> bool {
    use mir::expr::MirPlace;
    match place {
        MirPlace::ThisField { field_type, .. } | MirPlace::Field { field_type, .. } => {
            matches!(field_type, MirType::String { .. })
        }
        MirPlace::Global { ty, .. } => matches!(ty, MirType::String { .. }),
        MirPlace::Index { element_type, .. } => matches!(element_type, MirType::String { .. }),
        MirPlace::Deref { pointee_type, .. } => matches!(pointee_type, MirType::String { .. }),
        MirPlace::Local(_) => false,
    }
}

/// Returns `true` when any function in `module` contains a `MirStmt::Raise`,
/// triggering the codegen to declare the module-level `$rk_exception` tag.
pub(crate) fn module_uses_raise(module: &MirModule) -> bool {
    module.functions.iter().any(|f| stmts_use_raise(&f.body))
}

fn stmts_use_raise(stmts: &[MirStmt]) -> bool {
    stmts.iter().any(stmt_uses_raise)
}

fn stmt_uses_raise(stmt: &MirStmt) -> bool {
    match stmt {
        MirStmt::Raise { .. } => true,
        MirStmt::If {
            then_body,
            else_ifs,
            else_body,
            ..
        } => {
            stmts_use_raise(then_body)
                || else_ifs.iter().any(|(_, b)| stmts_use_raise(b))
                || else_body.as_ref().is_some_and(|b| stmts_use_raise(b))
        }
        MirStmt::Case {
            arms, else_body, ..
        } => {
            arms.iter().any(|a| stmts_use_raise(&a.body))
                || else_body.as_ref().is_some_and(|b| stmts_use_raise(b))
        }
        MirStmt::For { body, .. } | MirStmt::While { body, .. } | MirStmt::Repeat { body, .. } => {
            stmts_use_raise(body)
        }
        _ => false,
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

/// Which artifact this build is. One loader, two profiles: only the
/// sections differ. Debug carries the stepping tier
/// (`debug-functions`/`debug-lines`/`debug-locals`); Release omits it,
/// since optimization re-encodes bodies and would orphan the line table.
/// The monitoring tier and the load-bearing sections are in both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Debug,
    Release,
}

/// Generate a WASM module from a fully-lowered MirModule.
/// Needs the db to resolve Ident → string for export names.
pub fn generate_wasm(db: &dyn WorkspaceDataBase, module: &MirModule) -> wasm_encoder::Module {
    generate_wasm_profile(db, module, Profile::Debug)
}

/// As [`generate_wasm`], choosing which profile's sections to emit.
pub fn generate_wasm_profile(
    db: &dyn WorkspaceDataBase,
    module: &MirModule,
    profile: Profile,
) -> wasm_encoder::Module {
    let mut wasm_gen = WasmGen::new(db, module);
    wasm_gen.profile = profile;
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
    ///   `addr + 0..4`           - `len` (i32), current byte length, ≤ `capacity`
    ///   `addr + 4..4+capacity`  - embedded buffer
    /// There is no stored pointer — the buffer pointer is implicit (`addr + 4`).
    /// `capacity` comes from the declared `STRING[N]` (or `DEFAULT_STRING_CAPACITY`
    /// for plain `STRING`). Assignment is a bounded `memcpy` into the buffer via
    /// `rk.str_assign`. Reads/writes are addressed the same way as any string
    /// place (local, field, global) — see `emit_str_place_value` /
    /// `emit_string_assign`.
    StringMemory { address: u32, capacity: u32 },
}

struct WasmGen<'a> {
    db: &'a dyn WorkspaceDataBase,
    module: &'a MirModule,
    /// Which sections ride along; never changes what the code does.
    profile: Profile,
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
    /// Tag index of the module-level `$rk_exception` tag. `Some` exactly
    /// when at least one function contains a `MirStmt::Raise`. The tag
    /// signature is `(i32, i32) -> ()` — the two params carry the
    /// raised STRING's `(ptr, len)` payload.
    rk_exception_tag_idx: Option<u32>,
    /// Type index of `(i32, i32) -> ()`, for the `TagSection`.
    rk_exception_tag_type_idx: Option<u32>,
    /// Type index of `() -> (i32, i32)` — the block signature for the
    /// `$on_catch` block wrapping each `{test}` function's body. The
    /// `catch $rk_exception` clause delivers the tag's params as the
    /// block's result, so a `{test}` function's catch handler sees
    /// `(ptr, len)` on the stack.
    test_catch_block_type_idx: Option<u32>,
    /// Bump allocator for `{test}` functions' 12-byte result areas, past the
    /// STRING-scratch region.
    test_result_floor: Cell<u32>,
    /// Per-function `DebugTrap` records, for the `debug-lines` section.
    func_lines: FxHashMap<u32, Vec<(u32, mir::stmt::MirSourceLocation)>>,
    /// Per-function scalar locals `(wasm index, name, elem)`, for the
    /// `debug-locals` section.
    func_locals: FxHashMap<u32, Vec<(u32, String, MirElementary)>>,
    /// Per function: leaf symbols and array descriptors for its memory-resident
    /// locals, whose addresses are static (IEC forbids recursion). Emitted
    /// into `debug-locals` v2.
    func_memory_locals:
        FxHashMap<u32, (Vec<debug_format::Symbol>, Vec<debug_format::ArraySym>)>,
    /// Type table shared by every frame's array descriptors (aggregate
    /// element layouts) — becomes `DebugLocals::types` (v3).
    local_type_table: mir::debug_symbols::TypeTable,
}

/// Size of the per-`{test}` canonical-ABI `result<unit, string>` area: an
/// i8 discriminant at 0, the string ptr at 4 and len at 8.
const TEST_RESULT_AREA_SIZE: u32 = 12;

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

/// Number of `{test}` functions; one 12-byte result area is reserved per
/// test.
fn module_test_count(module: &MirModule) -> u32 {
    module.functions.iter().filter(|f| f.is_test).count() as u32
}

/// First byte past all MIR static memory: the layout plus the string-pool
/// data rebased past it. Scratch slots and test result areas go past this.
fn static_data_end(module: &MirModule) -> u32 {
    let layout_end = module.memory_layout.total_size();
    let strings_end = module
        .string_data
        .iter()
        .map(|(off, bytes)| off + bytes.len() as u32)
        .max()
        .unwrap_or(0);
    layout_end.max(strings_end)
}

pub(crate) fn core_memory_pages(module: &MirModule) -> u64 {
    let static_total = static_data_end(module);
    let scratch_total = module_total_scratch_slots(module) * STRING_SCRATCH_SLOT_SIZE;
    let test_results_total = module_test_count(module) * TEST_RESULT_AREA_SIZE;
    let total = static_total + scratch_total + test_results_total;
    if total == 0 {
        1
    } else {
        total.div_ceil(WASM_PAGE) as u64
    }
}

/// Name of the builtin that faults a null dereference.
pub(crate) const NULL_CHECK: &str = "rk.null_check";

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

        // STRING scratch slots start past the MIR static layout; test result
        // areas past those.
        let static_total = static_data_end(module);
        let scratch_total = module_total_scratch_slots(module) * STRING_SCRATCH_SLOT_SIZE;
        let scratch_floor = Cell::new(static_total);
        let test_result_floor = Cell::new(static_total + scratch_total);

        Self {
            db,
            module,
            profile: Profile::Debug,
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
            rk_exception_tag_idx: None,
            rk_exception_tag_type_idx: None,
            test_catch_block_type_idx: None,
            test_result_floor,
            func_lines: FxHashMap::default(),
            func_locals: FxHashMap::default(),
            func_memory_locals: FxHashMap::default(),
            local_type_table: mir::debug_symbols::TypeTable::new(),
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

    /// Allocate a 12-byte `{test}` result area, 4-aligned; the function
    /// returns its address.
    fn alloc_test_result_area(&self) -> u32 {
        let raw = self.test_result_floor.get();
        let aligned = (raw + 3) & !3;
        self.test_result_floor.set(aligned + TEST_RESULT_AREA_SIZE);
        aligned
    }
    fn emit_all(&mut self) {
        // The diagnostic reverse-lookup: every function Ident, call-site
        // callees included, to its text.
        crate::emit_expr::FN_NAMES_FOR_DIAGNOSTIC.with(|cell| {
            let mut names = cell.borrow_mut();
            names.clear();
            for ext in &self.module.extern_functions {
                names.insert(ext.name, ext.name.text(self.db).to_string());
            }
            for func in &self.module.functions {
                names.insert(func.name, func.name.text(self.db).to_string());
                walk_stmts_for_callees(&func.body, self.db, &mut names);
            }
        });

        // Pre-scan: which `wasm_builtins` exports any function references. The
        // graft goes between imports and user functions, so user indices skip
        // past it.
        let names_used = self.collect_builtin_names();

        // Index layout: [imports] [grafted builtins] [user functions]; the
        // grafted block may include a synthesized `__iec_raise` helper, which
        // `preflight_graft_count` accounts for.
        let n_imports = self.module.extern_functions.len() as u32;
        let needs_iec_raise_synth = self.preflight_needs_iec_raise(&names_used);
        let n_builtins =
            self.preflight_graft_count(&names_used) + if needs_iec_raise_synth { 1 } else { 0 };

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

        // 5. Register the module-level `$rk_exception` tag *before*
        //    grafting. The synth `__iec_raise` helper (emitted inside
        //    `graft_builtins` when needed) must reference the tag's
        //    index, so the tag has to exist first. Also covers `__RAISE`
        //    in user code and `{test}` function wrapping.
        //    Signature: `(i32, i32) -> ()` carries the raised STRING's
        //    `(ptr, len)`. Tag index starts at 0 (only one tag per module).
        if module_uses_raise(self.module)
            || module_test_count(self.module) > 0
            || needs_iec_raise_synth
        {
            let tag_type_idx = self.next_type_idx;
            self.type_section.ty().function(
                vec![wasm_encoder::ValType::I32, wasm_encoder::ValType::I32],
                std::iter::empty(),
            );
            self.next_type_idx += 1;
            self.rk_exception_tag_idx = Some(0);
            // Stash the type idx for the `finish()` TagSection emission.
            self.rk_exception_tag_type_idx = Some(tag_type_idx);
        }

        // 6. Graft builtins into the reserved slot.
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
                self.rk_exception_tag_idx,
            );
            self.builtin_indices = plan.name_to_wasm_idx;
        }

        // 7. If any `{test}` function exists, register the block-type
        //    used by their `$on_catch` blocks: `() -> (i32, i32)`. The
        //    `catch $rk_exception` clause hands the tag's (ptr, len)
        //    to this block as its result, so the catch handler sees
        //    them on the stack and can write them into the test's
        //    `result<unit, string>` area.
        if module_test_count(self.module) > 0 {
            let test_catch_ty = self.next_type_idx;
            self.type_section.ty().function(
                std::iter::empty::<wasm_encoder::ValType>(),
                vec![wasm_encoder::ValType::I32, wasm_encoder::ValType::I32],
            );
            self.next_type_idx += 1;
            self.test_catch_block_type_idx = Some(test_catch_ty);
        }

        // 8. Emit user functions.
        for func in &self.module.functions {
            self.emit_function(func);
        }
    }

    /// Name → wasm index for `emit_call`: user functions remapped past imports
    /// and grafted builtins, plus the builtins themselves (expression lowering
    /// calls `str_byte_cmp` by name).
    fn build_call_indices(
        &self,
    ) -> FxHashMap<hir::hir_def::interned::identifier::Ident, u32> {
        let mut map: FxHashMap<_, _> = self
            .module
            .function_indices
            .iter()
            .map(|(name, &mir_idx)| {
                let wasm_idx = self.index_remap.get(&mir_idx).copied().unwrap_or(mir_idx);
                (*name, wasm_idx)
            })
            .collect();
        for (name, &idx) in &self.builtin_indices {
            let ident = hir::hir_def::interned::identifier::Ident::new(
                self.db,
                compact_str::CompactString::from(name.as_str()),
            );
            map.entry(ident).or_insert(idx);
        }
        map
    }

    /// Hand the emit layer the null-check builtin's index, or `None`.
    fn publish_null_check_index(
        &self,
        fn_indices: &rustc_hash::FxHashMap<hir::hir_def::interned::identifier::Ident, u32>,
    ) {
        let ident = hir::hir_def::interned::identifier::Ident::new(
            self.db,
            compact_str::CompactString::from(NULL_CHECK),
        );
        let idx = fn_indices.get(&ident).copied();
        crate::emit_expr::NULL_CHECK_IDX.with(|cell| *cell.borrow_mut() = idx);
    }

    fn collect_builtin_names(&self) -> Vec<String> {
        use mir::expr::{MirExpr, MirPlace};
        use mir::stmt::MirStmt;
        let db = self.db;
        let mut found = rustc_hash::FxHashSet::default();

        // Builtins are also called from expression position, so every
        // expression is walked.
        fn walk_place(
            db: &dyn db::WorkspaceDataBase,
            place: &MirPlace,
            found: &mut rustc_hash::FxHashSet<String>,
        ) {
            match place {
                MirPlace::Index { base, index, .. } => {
                    walk_place(db, base, found);
                    walk_expr(db, index, found);
                }
                MirPlace::Deref { base, checked, .. } => {
                    // A user-written `^` is null-checked, so the builtin has
                    // to be grafted even though no MirExpr::Call names it.
                    if *checked && crate::builtins::lookup(NULL_CHECK).is_some() {
                        found.insert(NULL_CHECK.to_string());
                    }
                    walk_place(db, base, found)
                }
                MirPlace::Field { base, .. } => walk_place(db, base, found),
                MirPlace::Local(_) | MirPlace::ThisField { .. } | MirPlace::Global { .. } => {}
            }
        }
        fn walk_expr(
            db: &dyn db::WorkspaceDataBase,
            expr: &MirExpr,
            found: &mut rustc_hash::FxHashSet<String>,
        ) {
            match expr {
                MirExpr::Call(call) => {
                    let name = call.callee.text(db);
                    if crate::builtins::lookup(name).is_some() {
                        found.insert(name.to_string());
                    }
                    for arg in &call.args {
                        walk_expr(db, &arg.value, found);
                    }
                }
                MirExpr::BinOp { lhs, rhs, .. } => {
                    walk_expr(db, lhs, found);
                    walk_expr(db, rhs, found);
                }
                MirExpr::UnaryOp { expr, .. } | MirExpr::Cast { expr, .. } => {
                    walk_expr(db, expr, found)
                }
                MirExpr::Load(place, _) | MirExpr::AddrOf(place) => walk_place(db, place, found),
                // `src` may be an aggregate-returning Call, whose args can
                // reach builtins — recurse rather than walk a place.
                MirExpr::CopyIntoScratch { src, .. } => walk_expr(db, src, found),
                MirExpr::Constant(_) | MirExpr::StringLiteral { .. } => {}
            }
        }
        fn walk(
            db: &dyn db::WorkspaceDataBase,
            stmts: &[MirStmt],
            found: &mut rustc_hash::FxHashSet<String>,
        ) {
            for stmt in stmts {
                match stmt {
                    MirStmt::WasmIntrinsic { instruction, .. } => {
                        if crate::builtins::lookup(instruction.as_str()).is_some() {
                            found.insert(instruction.to_string());
                        }
                    }
                    MirStmt::Assign { target, value } => {
                        walk_place(db, target, found);
                        walk_expr(db, value, found);
                    }
                    MirStmt::Call(call) => {
                        let name = call.callee.text(db);
                        if crate::builtins::lookup(name).is_some() {
                            found.insert(name.to_string());
                        }
                        for arg in &call.args {
                            walk_expr(db, &arg.value, found);
                        }
                    }
                    MirStmt::FbCall {
                        instance,
                        input_writes,
                        output_reads,
                        ..
                    } => {
                        walk_place(db, instance, found);
                        for (_, value, _) in input_writes {
                            walk_expr(db, value, found);
                        }
                        for (_, place, _, _) in output_reads {
                            walk_place(db, place, found);
                        }
                    }
                    MirStmt::If {
                        condition,
                        then_body,
                        else_ifs,
                        else_body,
                    } => {
                        walk_expr(db, condition, found);
                        walk(db, then_body, found);
                        for (cond, body) in else_ifs {
                            walk_expr(db, cond, found);
                            walk(db, body, found);
                        }
                        if let Some(b) = else_body {
                            walk(db, b, found);
                        }
                    }
                    MirStmt::Case {
                        selector,
                        arms,
                        else_body,
                        ..
                    } => {
                        walk_expr(db, selector, found);
                        for arm in arms {
                            // A STRING label's test is a `str.byte_cmp` call;
                            // the graft only pulls in builtins this scan sees.
                            for pattern in &arm.patterns {
                                if let mir::stmt::MirCasePattern::Test(test) = pattern {
                                    walk_expr(db, test, found);
                                }
                            }
                            walk(db, &arm.body, found);
                        }
                        if let Some(b) = else_body {
                            walk(db, b, found);
                        }
                    }
                    MirStmt::For {
                        start,
                        end,
                        step,
                        body,
                        ..
                    } => {
                        walk_expr(db, start, found);
                        walk_expr(db, end, found);
                        walk_expr(db, step, found);
                        walk(db, body, found);
                    }
                    MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
                        walk_expr(db, condition, found);
                        walk(db, body, found);
                    }
                    MirStmt::Raise { message } => walk_expr(db, message, found),
                    _ => {}
                }
            }
        }
        for func in &self.module.functions {
            walk(db, &func.body, &mut found);
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
                // STRING params too: a `VAR_IN_OUT`/`VAR_OUTPUT` string (lowered
                // to `Pointer(String)`) is assigned via rk.str_assign.
                || f.params.iter().any(|p| param_is_stringish(&p.ty))
        });
        let any_nested_string_call = self
            .module
            .functions
            .iter()
            .any(|f| count_nested_string_calls_stmts(&f.body) > 0);
        // Also force-include when a STRING field/global is assigned: those route
        // through rk.str_assign too, but live outside `f.locals`.
        let any_static_string = module_assigns_static_string(self.module);
        if (any_string_local || any_nested_string_call || any_static_string)
            && crate::builtins::lookup("rk.str_assign").is_some()
        {
            found.insert("rk.str_assign".to_string());
        }

        let mut v: Vec<String> = found.into_iter().collect();
        v.sort();
        v
    }

    /// How many bundle functions a graft of `names` would pull in, excluding
    /// the synthesized helper ([`preflight_needs_iec_raise`]).
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

    /// True when the graft of `names` would pull in any function that
    /// calls the `__iec_raise` import — implying we need to emit a
    /// codegen-synthesized helper that throws `$rk_exception` and reserve
    /// the corresponding function-index slot.
    fn preflight_needs_iec_raise(&self, names: &[String]) -> bool {
        if names.is_empty() {
            return false;
        }
        let mut closure: Vec<u32> = Vec::new();
        let mut seen = rustc_hash::FxHashSet::default();
        for name in names {
            if let Some(root) = crate::builtins::lookup(name) {
                for idx in crate::builtins::transitive_closure(root) {
                    if seen.insert(idx) {
                        closure.push(idx);
                    }
                }
            }
        }
        crate::builtins::closure_uses_import(&closure, "__iec_raise")
    }

    fn emit_import(&mut self, ext_fn: &MirExternFunction) {
        // Params from the declaration; results are the scalar VAR_OUTPUTs then
        // the return type last.
        let (params, _) = build_signature(&ext_fn.params, &None);
        let results: Vec<ValType> = ext_fn
            .results()
            .map(|ty| {
                mir_type_to_val_type(ty).unwrap_or_else(|| {
                    panic!(
                        "internal compiler error: extern result of `{}` is not scalar \
                         (E0243 admits scalars only): {ty:?}",
                        ext_fn.name.text(self.db)
                    )
                })
            })
            .collect();

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
        if func.is_test {
            self.emit_test_function(func);
            return;
        }

        // The diagnostic marker naming the containing function, restored on
        // exit.
        crate::emit_expr::CURRENT_EMIT_FN
            .with(|c| c.replace(Some(func.name.text(self.db).to_string())));

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

        // Collect scalar locals (params + VARs held in wasm locals) for the
        // `debug-locals` table, so a debugger can label `FrameHandle::local(i)`.
        let mut scalar_locals: Vec<(u32, String, MirElementary)> = local_map
            .iter()
            .filter_map(|(name, info)| match info {
                LocalInfo::Scalar { index, elem } => {
                    Some((*index, name.text(self.db).to_string(), *elem))
                }
                _ => None,
            })
            .collect();
        scalar_locals.sort_by_key(|(idx, _, _)| *idx);
        if !scalar_locals.is_empty() {
            self.func_locals.insert(func.index, scalar_locals);
        }

        // Memory-resident locals, leaf-walked like module symbols with
        // frame-relative paths.
        let mut mem_symbols = Vec::new();
        let mut mem_arrays = Vec::new();
        for local in &func.locals {
            if let mir::function::MirStorage::Memory { address, .. } = local.storage {
                mir::debug_symbols::collect_frame_root(
                    self.db,
                    local.name.text(self.db).as_ref(),
                    address,
                    &local.ty,
                    &mut mem_symbols,
                    &mut mem_arrays,
                    &mut self.local_type_table,
                );
            }
        }
        if !mem_symbols.is_empty() || !mem_arrays.is_empty() {
            mem_symbols.sort_by(|a, b| a.path.cmp(&b.path));
            mem_arrays.sort_by(|a, b| a.path.cmp(&b.path));
            self.func_memory_locals
                .insert(func.index, (mem_symbols, mem_arrays));
        }

        // Extra (non-parameter) wasm locals. The scalar return slot is not
        // declared here: MIR allocated it in `func.locals` with a real
        // `local_index`.
        let mut extra_locals: Vec<(u32, ValType)> = Vec::new();
        // Local variables that are scalars (includes the return slot).
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
            let next_idx =
                (params.len() as u32) + extra_locals.iter().map(|(c, _)| *c).sum::<u32>();
            extra_locals.push((2, ValType::I32));
            Some((next_idx, next_idx + 1))
        } else {
            None
        };

        // One i32 scratch for a runtime-computed `FbCall` receiver address,
        // appended after the existing locals.
        let fb_recv_tmp = if crate::emit_stmt::stmts_need_dynamic_fb_base(&func.body, &local_map) {
            let idx = (params.len() as u32) + extra_locals.iter().map(|(c, _)| *c).sum::<u32>();
            extra_locals.push((1, ValType::I32));
            Some(idx)
        } else {
            None
        };

        // Per-FOR scratch locals snapshotting each loop's end/step at entry
        // (IEC: evaluated once). Appended last, so no existing index moves.
        let for_scratch = alloc_for_scratch(&func.body, params.len() as u32, &mut extra_locals);

        // One i64 scratch for the calendar floor-division casts, which need
        // their dividend twice.
        let datetime_floor_tmp = if crate::mir_cast::body_needs_datetime_floor_tmp(&func.body) {
            let idx = (params.len() as u32) + extra_locals.iter().map(|(c, _)| *c).sum::<u32>();
            extra_locals.push((1, ValType::I64));
            Some(idx)
        } else {
            None
        };

        // One scratch slot per nested STRING call, past the MIR static layout.
        let scratch_slots: Vec<u32> = (0..nested_str_count)
            .map(|_| self.alloc_scratch_slot())
            .collect();

        // Emit function body
        let mut wasm_func = wasm_encoder::Function::new(extra_locals);

        // The return slot, by `origin_name`: methods store it under the bare
        // method name.
        let return_value: Option<crate::emit_stmt::ReturnValue> = if func.return_type.is_some() {
            use crate::emit_stmt::ReturnValue;
            local_map
                .get(&func.origin_name)
                .and_then(|info| match info {
                    LocalInfo::Scalar { index, .. } => Some(ReturnValue::ScalarLocal(*index)),
                    LocalInfo::StringMemory { address, .. } => {
                        Some(ReturnValue::StringMem(*address))
                    }
                    // A memory-resident return slot that is not a STRING is an
                    // aggregate; its address IS the return value.
                    LocalInfo::Memory { address, .. } => {
                        Some(ReturnValue::AggregateMem(*address))
                    }
                    _ => None,
                })
        } else {
            None
        };

        // Build remapped function indices for call instructions
        let remapped_fn_indices = self.build_call_indices();
        self.publish_null_check_index(&remapped_fn_indices);

        // The per-function snapshot context `emit_call` consults for nested
        // STRING-returning calls.
        let prev_ctx = if let Some((ptr_tmp, len_tmp)) = snapshot_local_indices {
            let str_assign_idx = self.builtin_indices.get("rk.str_assign").copied().expect(
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
        let prev_floor_tmp =
            crate::mir_cast::DATETIME_FLOOR_TMP.with(|cell| cell.replace(datetime_floor_tmp));

        // Automatic storage is fresh at every invocation: wasm locals are zeroed
        // by the engine, but an aggregate at a fixed address must be reset
        // before the body, so initializers lowered into it win.
        for local in &func.locals {
            if local.var_storage == mir::function::MirVariableStorage::Automatic
                && let MirStorage::Memory { address, size, .. } = local.storage
                && size > 0
            {
                wasm_func.instruction(&Instruction::I32Const(address as i32));
                wasm_func.instruction(&Instruction::I32Const(0));
                wasm_func.instruction(&Instruction::I32Const(size as i32));
                wasm_func.instruction(&Instruction::MemoryFill(0));
            }
        }

        // Emit statements
        let lines = emit_stmts_with_return(
            &mut wasm_func,
            &func.body,
            &local_map,
            &remapped_fn_indices,
            &self.builtin_indices,
            return_value,
            self.rk_exception_tag_idx,
            fb_recv_tmp,
            for_scratch,
        );
        if !lines.is_empty() {
            self.func_lines.insert(func.index, lines);
        }

        // Restore the prior context.
        SNAPSHOT_CTX.with(|cell| cell.replace(prev_ctx));
        crate::mir_cast::DATETIME_FLOOR_TMP.with(|cell| cell.replace(prev_floor_tmp));

        // Push return value at function end — the same shapes a mid-body
        // RETURN pushes, from one implementation.
        if let Some(ret) = return_value {
            crate::emit_stmt::emit_return_value(&mut wasm_func, ret);
        }

        // End function
        wasm_func.instruction(&Instruction::End);
        self.code_section.function(&wasm_func);
    }

    /// Emit a `{test}` function. The core-wasm signature is `() -> i32`:
    /// the returned i32 is the address of a 12-byte canonical-ABI
    /// `result<unit, string>` area. The function body is wrapped in a
    /// `try_table (catch $rk_exception)` so that a `__RAISE` from any
    /// nested call lands in the catch handler and is encoded as the Err
    /// variant. Normal completion leaves the result area's discriminant
    /// at its zero-initialized value (Ok).
    ///
    /// Layout emitted (pseudo-WAT):
    /// ```text
    ///   block $on_catch (result i32 i32)
    ///     try_table (catch $rk_exception 0)
    ///       <body>
    ///     end
    ///     ;; success path
    ///     i32.const <result_area>
    ///     return
    ///   end
    ///   ;; catch path: (msg_ptr, msg_len) on stack
    ///   local.set $len_tmp
    ///   local.set $ptr_tmp
    ///   i32.const <result_area>; i32.const 1; i32.store8       ;; Err discriminant
    ///   i32.const <result_area + 4>; local.get $ptr_tmp; i32.store
    ///   i32.const <result_area + 8>; local.get $len_tmp; i32.store
    ///   i32.const <result_area>
    /// ;; falls through to function end with the area's address as the i32 result
    /// ```
    fn emit_test_function(&mut self, func: &MirFunction) {
        // The diagnostic marker, as in `emit_function`.
        crate::emit_expr::CURRENT_EMIT_FN
            .with(|c| c.replace(Some(func.name.text(self.db).to_string())));

        // Test functions always have signature `() -> i32` regardless of
        // their MIR-declared params/return (which is `()` for `{test}`).
        let type_idx = self.next_type_idx;
        self.type_section
            .ty()
            .function(std::iter::empty::<ValType>(), vec![ValType::I32]);
        self.next_type_idx += 1;
        self.fn_section.function(type_idx);

        // Exported under the test's name.
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

        // Per-test 12-byte canonical-ABI result area.
        let result_area = self.alloc_test_result_area();

        // Build local map + extra locals (mirrors `emit_function`).
        let local_map = build_local_map(func);
        let mut extra_locals: Vec<(u32, ValType)> = Vec::new();
        for local in &func.locals {
            if let MirStorage::Scalar { .. } = local.storage
                && let Some(vt) = mir_type_to_val_type(&local.ty)
            {
                extra_locals.push((1, vt));
            }
        }

        // Two i32 scratch locals for the catch handler to stash `(ptr, len)`.
        let scratch_base: u32 = extra_locals.iter().map(|(c, _)| *c).sum();
        extra_locals.push((2, ValType::I32));
        let ptr_tmp = scratch_base;
        let len_tmp = scratch_base + 1;

        // Receiver scratch, as in `emit_function`; the wrapper takes no params,
        // so `scratch_base` omits them.
        let fb_recv_tmp = if crate::emit_stmt::stmts_need_dynamic_fb_base(&func.body, &local_map) {
            let idx = extra_locals.iter().map(|(c, _)| *c).sum::<u32>();
            extra_locals.push((1, ValType::I32));
            Some(idx)
        } else {
            None
        };

        // Per-FOR end/step snapshots, as in `emit_function`. The test
        // wrapper takes no params, hence the 0.
        let for_scratch = alloc_for_scratch(&func.body, 0, &mut extra_locals);

        // Calendar floor-division scratch, as in `emit_function` (no params).
        let datetime_floor_tmp = if crate::mir_cast::body_needs_datetime_floor_tmp(&func.body) {
            let idx = extra_locals.iter().map(|(c, _)| *c).sum::<u32>();
            extra_locals.push((1, ValType::I64));
            Some(idx)
        } else {
            None
        };

        // Per-call-site STRING snapshot slots for nested STRING-returning
        // calls inside the test body. Same as `emit_function`.
        let nested_str_count = count_nested_string_calls_stmts(&func.body);
        let scratch_slots: Vec<u32> = (0..nested_str_count)
            .map(|_| self.alloc_scratch_slot())
            .collect();

        let mut wasm_func = wasm_encoder::Function::new(extra_locals);

        let remapped_fn_indices = self.build_call_indices();
        self.publish_null_check_index(&remapped_fn_indices);

        // SNAPSHOT_CTX reuses the two scratch locals; snapshots and the catch
        // shuffle never run concurrently.
        let prev_ctx = if nested_str_count > 0 {
            let str_assign_idx = self.builtin_indices.get("rk.str_assign").copied().expect(
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
        let prev_floor_tmp =
            crate::mir_cast::DATETIME_FLOOR_TMP.with(|cell| cell.replace(datetime_floor_tmp));

        let tag_idx = self.rk_exception_tag_idx.expect(
            "rk_exception_tag_idx must be set whenever any function (including tests) is emitted: \
             the test wrapper always uses the tag for `try_table (catch $rk_exception)`",
        );
        let catch_block_ty = self
            .test_catch_block_type_idx
            .expect("test_catch_block_type_idx must be set when emitting a {test} function");

        // block $on_catch (result i32 i32)
        wasm_func.instruction(&Instruction::Block(wasm_encoder::BlockType::FunctionType(
            catch_block_ty,
        )));
        //   try_table (catch $rk_exception 0)
        wasm_func.instruction(&Instruction::TryTable(
            wasm_encoder::BlockType::Empty,
            std::borrow::Cow::Owned(vec![wasm_encoder::Catch::One {
                tag: tag_idx,
                label: 0, // -> $on_catch (outer scope of try_table)
            }]),
        ));

        // The body has no scalar return slot: tests are void at the MIR level.
        let lines = emit_stmts_with_return(
            &mut wasm_func,
            &func.body,
            &local_map,
            &remapped_fn_indices,
            &self.builtin_indices,
            None,
            self.rk_exception_tag_idx,
            fb_recv_tmp,
            for_scratch,
        );
        if !lines.is_empty() {
            self.func_lines.insert(func.index, lines);
        }

        SNAPSHOT_CTX.with(|cell| cell.replace(prev_ctx));
        crate::mir_cast::DATETIME_FLOOR_TMP.with(|cell| cell.replace(prev_floor_tmp));

        // end try_table — only reached on the success (no-throw) path.
        wasm_func.instruction(&Instruction::End);

        // Success: leave the zero-initialized Ok discriminant in place,
        // push the area's address as the i32 result, return early.
        wasm_func.instruction(&Instruction::I32Const(result_area as i32));
        wasm_func.instruction(&Instruction::Return);

        // end $on_catch — entered only via the catch branch with
        // `(msg_ptr, msg_len)` on the stack as the block's result.
        wasm_func.instruction(&Instruction::End);

        // Catch handler: stash, then write Err variant into the area.
        wasm_func.instruction(&Instruction::LocalSet(len_tmp));
        wasm_func.instruction(&Instruction::LocalSet(ptr_tmp));
        // discriminant = 1 (Err) at offset 0
        wasm_func.instruction(&Instruction::I32Const(result_area as i32));
        wasm_func.instruction(&Instruction::I32Const(1));
        wasm_func.instruction(&Instruction::I32Store8(wasm_encoder::MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        }));
        // string ptr at offset 4
        wasm_func.instruction(&Instruction::I32Const((result_area + 4) as i32));
        wasm_func.instruction(&Instruction::LocalGet(ptr_tmp));
        wasm_func.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 2,
            memory_index: 0,
        }));
        // string len at offset 8
        wasm_func.instruction(&Instruction::I32Const((result_area + 8) as i32));
        wasm_func.instruction(&Instruction::LocalGet(len_tmp));
        wasm_func.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
            offset: 0,
            align: 2,
            memory_index: 0,
        }));
        // Push the area's address as the i32 result.
        wasm_func.instruction(&Instruction::I32Const(result_area as i32));

        // End function — i32 on stack.
        wasm_func.instruction(&Instruction::End);
        self.code_section.function(&wasm_func);
    }

    fn finish(mut self) -> wasm_encoder::Module {
        // Re-export the imported memory, for tests and inspection tools.
        self.export_section
            .export("memory", wasm_encoder::ExportKind::Memory, 0);

        // The RETAIN band bounds as two immutable i32 globals, appended after
        // any grafted globals; `retain_size == 0` means no retained variables.
        let retain_global = |value: u32| {
            (
                wasm_encoder::GlobalType {
                    val_type: wasm_encoder::ValType::I32,
                    mutable: false,
                    shared: false,
                },
                wasm_encoder::ConstExpr::i32_const(value as i32),
            )
        };
        let retain_base_idx = self.global_section.len();
        let (ty, init) = retain_global(self.module.retain_base);
        self.global_section.global(ty, &init);
        let retain_size_idx = self.global_section.len();
        let (ty, init) = retain_global(self.module.retain_size);
        self.global_section.global(ty, &init);
        self.export_section.export(
            "retain_base",
            wasm_encoder::ExportKind::Global,
            retain_base_idx,
        );
        self.export_section.export(
            "retain_size",
            wasm_encoder::ExportKind::Global,
            retain_size_idx,
        );

        // The host-visible GLOBALS band, same two-integer contract; RETAIN
        // globals appear in both bands.
        let globals_base_idx = self.global_section.len();
        let (ty, init) = retain_global(self.module.globals_base);
        self.global_section.global(ty, &init);
        let globals_size_idx = self.global_section.len();
        let (ty, init) = retain_global(self.module.globals_size);
        self.global_section.global(ty, &init);
        self.export_section.export(
            "globals_base",
            wasm_encoder::ExportKind::Global,
            globals_base_idx,
        );
        self.export_section.export(
            "globals_size",
            wasm_encoder::ExportKind::Global,
            globals_size_idx,
        );

        let mut module = wasm_encoder::Module::new();
        module.section(&self.type_section);
        module.section(&self.import_section);
        module.section(&self.fn_section);
        // Tag section (per the exception-handling proposal, between Memory
        // and Global). Only emitted when the module actually `__RAISE`s.
        if let Some(type_idx) = self.rk_exception_tag_type_idx {
            let mut tags = wasm_encoder::TagSection::new();
            tags.tag(wasm_encoder::TagType {
                kind: wasm_encoder::TagKind::Exception,
                func_type_idx: type_idx,
            });
            module.section(&tags);
        }
        // The global section always carries the two RETAIN-band globals;
        // grafted globals precede them.
        module.section(&self.global_section);
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

        // The wasm `name` section: every function by its final index, for
        // backtraces and tooling.
        let n_func_imports = self.module.extern_functions.len() as u32;
        let mut named: Vec<(u32, String)> = Vec::new();
        for ext in &self.module.extern_functions {
            if let Some(&idx) = self.index_remap.get(&ext.index) {
                named.push((idx, ext.name.text(self.db).to_string()));
            }
        }
        for func in &self.module.functions {
            if let Some(&idx) = self.index_remap.get(&func.index) {
                named.push((idx, func.name.text(self.db).to_string()));
            }
        }
        named.sort_by_key(|(idx, _)| *idx);
        let mut fn_names = wasm_encoder::NameMap::new();
        for (idx, name) in &named {
            fn_names.append(*idx, name);
        }
        let mut name_section = wasm_encoder::NameSection::new();
        name_section.functions(&fn_names);
        module.section(&name_section);

        // The stepping tier rides only in a Debug artifact: optimization
        // re-encodes bodies and would orphan these tables.
        if self.profile == Profile::Debug {
            // `debug-functions`: DefinedFuncIndex → IEC name, keyed as `FrameHandle`
            // reports it (imports excluded).
            let mut func_entries: Vec<debug_format::FuncEntry> = self
                .module
                .functions
                .iter()
                .filter_map(|func| {
                    self.index_remap
                        .get(&func.index)
                        .map(|&widx| debug_format::FuncEntry {
                            defined_index: widx - n_func_imports,
                            name: func.name.text(self.db).to_string(),
                        })
                })
                .collect();
            func_entries.sort_by_key(|e| e.defined_index);
            let debug_functions = debug_format::DebugFunctions {
                version: debug_format::DEBUG_FUNCTIONS_VERSION,
                functions: func_entries,
            };
            module.section(&wasm_encoder::CustomSection {
                name: std::borrow::Cow::Borrowed(debug_format::DEBUG_FUNCTIONS_SECTION),
                data: std::borrow::Cow::Owned(debug_functions.to_msgpack()),
            });

            // `debug-lines`: per-function (within-body offset → source position) by
            // DefinedFuncIndex, each statement's file resolved to its index in the
            // module's file table.
            let file_index: FxHashMap<&str, u32> = self
                .module
                .source_files
                .iter()
                .enumerate()
                .map(|(i, f)| (f.as_str(), i as u32))
                .collect();
            let mut func_line_tables: Vec<debug_format::FuncLines> = self
                .func_lines
                .iter()
                .filter_map(|(mir_idx, recs)| {
                    let widx = *self.index_remap.get(mir_idx)?;
                    let mut lines: Vec<debug_format::LineEntry> = recs
                        .iter()
                        .map(|(offset, loc)| debug_format::LineEntry {
                            offset: *offset,
                            file: file_index.get(loc.file_url.as_str()).copied().unwrap_or(0),
                            line: loc.line,
                            col: loc.column,
                        })
                        .collect();
                    lines.sort_by_key(|e| e.offset);
                    Some(debug_format::FuncLines {
                        defined_index: widx - n_func_imports,
                        lines,
                    })
                })
                .collect();
            func_line_tables.sort_by_key(|f| f.defined_index);
            let debug_lines = debug_format::DebugLines {
                version: debug_format::DEBUG_LINES_VERSION,
                files: self.module.source_files.clone(),
                functions: func_line_tables,
            };
            module.section(&wasm_encoder::CustomSection {
                name: std::borrow::Cow::Borrowed(debug_format::DEBUG_LINES_SECTION),
                data: std::borrow::Cow::Owned(debug_lines.to_msgpack()),
            });

            // `debug-locals`: per-function scalar-local labels (wasm local index →
            // IEC name/type), keyed by DefinedFuncIndex, so a debugger names the
            // values `FrameHandle::local(i)` returns.
            let all_indices: std::collections::BTreeSet<u32> = self
                .func_locals
                .keys()
                .chain(self.func_memory_locals.keys())
                .copied()
                .collect();
            let mut func_local_tables: Vec<debug_format::FuncLocals> = all_indices
                .into_iter()
                .filter_map(|mir_idx| {
                    let widx = *self.index_remap.get(&mir_idx)?;
                    let mut vars: Vec<debug_format::LocalVar> = self
                        .func_locals
                        .get(&mir_idx)
                        .map(|locals| {
                            locals
                                .iter()
                                .map(|(wasm_index, name, elem)| debug_format::LocalVar {
                                    wasm_index: *wasm_index,
                                    name: name.clone(),
                                    ty: mir::debug_symbols::sym_type_of(*elem),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    vars.sort_by_key(|v| v.wasm_index);
                    let (memory, arrays) = self
                        .func_memory_locals
                        .get(&mir_idx)
                        .cloned()
                        .unwrap_or_default();
                    Some(debug_format::FuncLocals {
                        defined_index: widx - n_func_imports,
                        locals: vars,
                        memory,
                        arrays,
                    })
                })
                .collect();
            func_local_tables.sort_by_key(|f| f.defined_index);
            let debug_locals = debug_format::DebugLocals {
                version: debug_format::DEBUG_LOCALS_VERSION,
                functions: func_local_tables,
                types: std::mem::take(&mut self.local_type_table).into_entries(),
            };
            module.section(&wasm_encoder::CustomSection {
                name: std::borrow::Cow::Borrowed(debug_format::DEBUG_LOCALS_SECTION),
                data: std::borrow::Cow::Owned(debug_locals.to_msgpack()),
            });
        }

        // The debug-symbol table (`debug-symbols`), for by-name monitoring;
        // strippable.
        let debug_bytes = self.module.debug_symbols.to_msgpack();
        module.section(&wasm_encoder::CustomSection {
            name: std::borrow::Cow::Borrowed(mir::debug_symbols::DEBUG_SYMBOLS_SECTION),
            data: std::borrow::Cow::Owned(debug_bytes),
        });

        // The retain map is load-bearing: the runtime restores only these
        // ranges. Release optimization must keep it.
        if !self.module.retain_map.ranges.is_empty() {
            module.section(&wasm_encoder::CustomSection {
                name: std::borrow::Cow::Borrowed(debug_format::RETAIN_MAP_SECTION),
                data: std::borrow::Cow::Owned(self.module.retain_map.to_msgpack()),
            });
        }

        // The schedule, load-bearing too: policy as data, so a task keeps its
        // name, priority and RESOURCE.
        if let Some(manifest) = &self.module.schedule_manifest {
            module.section(&wasm_encoder::CustomSection {
                name: std::borrow::Cow::Borrowed(debug_format::SCHEDULE_SECTION),
                data: std::borrow::Cow::Owned(manifest.to_msgpack()),
            });
        }

        // The `{test}` functions this module carries. LOAD-BEARING for `rk
        // test`: the runner reads it back to know what to call and what to
        // name each result. Emitted here rather than by a wrapper, so the core
        // module a test runs against is the one a plant runs.
        if !self.module.test_manifest.tests.is_empty() {
            module.section(&wasm_encoder::CustomSection {
                name: std::borrow::Cow::Borrowed(debug_format::test_manifest::TEST_MANIFEST_SECTION),
                data: std::borrow::Cow::Owned(self.module.test_manifest.to_msgpack()),
            });
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
            MirParamKind::InOut | MirParamKind::Output if matches!(&param.ty, MirType::Pointer(inner) if matches!(inner.as_ref(), MirType::String { .. })) =>
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
        // A STRING return is (ptr, len), pushed in that order at the epilogue.
        Some(MirType::String { .. }) => vec![ValType::I32, ValType::I32],
        // An aggregate returns the address of the callee's static return slot;
        // the caller copies out of it.
        Some(MirType::Struct(_)) | Some(MirType::Array(_)) => vec![ValType::I32],
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
                    // Aggregate VAR_INPUT: a pointer to the caller's call-entry
                    // snapshot, dereferenced like an InOut param.
                    MirType::Pointer(inner)
                        if matches!(inner.as_ref(), MirType::Struct(_) | MirType::Array(_)) =>
                    {
                        map.insert(
                            param.name,
                            LocalInfo::Pointer {
                                index: param_idx,
                                pointee_elem: None,
                            },
                        );
                        param_idx += 1;
                    }
                    // A REF_TO VAR_INPUT is a value param: only a written `^`
                    // dereferences it.
                    MirType::Pointer(_) => {
                        map.insert(
                            param.name,
                            LocalInfo::Scalar {
                                index: param_idx,
                                elem: MirElementary::DInt,
                            },
                        );
                        param_idx += 1;
                    }
                    ty => {
                        let elem = match ty {
                            MirType::Elementary(e) => *e,
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
        // An enum's lane is its declared storage, a subrange's its base.
        MirType::Enum(e) => Some(mir_elementary_to_val_type(e.storage)),
        MirType::Subrange(s) => Some(mir_elementary_to_val_type(s.base)),
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
