//! WASM code generation from MIR.
//!
//! This module takes a fully-lowered `MirModule` and produces a `wasm_encoder::Module`.
//! It is purely mechanical — no type inference, no monomorphization, no HIR queries.
//! All decisions have been made in MIR; codegen just emits bytes.

mod emit_expr;
mod emit_stmt;
mod mir_cast;

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
    Memory { address: u32, size: u32, align: u32, elem: Option<MirElementary> },
    /// Pointer (VAR_IN_OUT) — i32 local holding an address.
    Pointer { index: u32, pointee_elem: Option<MirElementary> },
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
    export_section: wasm_encoder::ExportSection,
    code_section: wasm_encoder::CodeSection,
    next_type_idx: u32,
    /// MIR function index → wasm index: wasm requires all imports first, MIR
    /// may interleave them.
    index_remap: FxHashMap<u32, u32>,
}

impl<'a> WasmGen<'a> {
    fn new(db: &'a dyn WorkspaceDataBase, module: &'a MirModule) -> Self {
        Self {
            db,
            module,
            type_section: Default::default(),
            import_section: Default::default(),
            fn_section: Default::default(),
            export_section: Default::default(),
            code_section: Default::default(),
            next_type_idx: 0,
            index_remap: FxHashMap::default(),
        }
    }

    fn emit_all(&mut self) {
        // Build a corrected index map: all imports first, then locals.
        // MIR may have assigned indices in a different order (e.g., monomorphized
        // externs appended after locals), but WASM requires imports before locals.
        let mut index_remap: FxHashMap<u32, u32> = FxHashMap::default();
        let mut wasm_idx: u32 = 0;

        // 1. Assign WASM indices for all imports
        for ext_fn in &self.module.extern_functions {
            index_remap.insert(ext_fn.index, wasm_idx);
            wasm_idx += 1;
        }

        // 2. Assign WASM indices for all local functions
        for func in &self.module.functions {
            index_remap.insert(func.index, wasm_idx);
            wasm_idx += 1;
        }

        // Store the remap for use during code emission
        self.index_remap = index_remap;

        // 3. Emit imports
        for ext_fn in &self.module.extern_functions {
            self.emit_import(ext_fn);
        }

        // 4. Emit local functions
        for func in &self.module.functions {
            self.emit_function(func);
        }
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
        let wasm_idx = self.index_remap.get(&ext_fn.index).copied().unwrap_or(ext_fn.index);
        self.export_section.export(
            &export_name,
            wasm_encoder::ExportKind::Func,
            wasm_idx,
        );
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
            let export_name = func.export_name.as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| func.name.text(self.db).to_string());
            let wasm_idx = self.index_remap.get(&func.index).copied().unwrap_or(func.index);
            self.export_section.export(
                &export_name,
                wasm_encoder::ExportKind::Func,
                wasm_idx,
            );
        }

        // Build local map from params + locals
        let local_map = build_local_map(func);

        // Build extra locals (non-parameter WASM locals)
        let mut extra_locals: Vec<(u32, ValType)> = Vec::new();
        // Return value local
        if let Some(ref ret_ty) = func.return_type {
            if let Some(vt) = mir_type_to_val_type(ret_ty) {
                extra_locals.push((1, vt));
            }
        }
        // Local variables that are scalars
        for local in &func.locals {
            if let MirStorage::Scalar { .. } = local.storage {
                if let Some(vt) = mir_type_to_val_type(&local.ty) {
                    extra_locals.push((1, vt));
                }
            }
        }

        // Emit function body
        let mut wasm_func = wasm_encoder::Function::new(extra_locals);

        // Find return local index if function returns a value.
        // Use origin_name because methods store return locals under the bare method name,
        // while func.name is the qualified "FB$Method" name.
        let return_local = if func.return_type.is_some() {
            local_map.get(&func.origin_name).and_then(|info| match info {
                LocalInfo::Scalar { index, .. } => Some(*index),
                _ => None,
            })
        } else {
            None
        };

        // Build remapped function indices for call instructions
        let remapped_fn_indices: FxHashMap<_, _> = self.module.function_indices
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
            return_local,
        );

        // Push return value at function end
        if let Some(ret_idx) = return_local {
            wasm_func.instruction(&Instruction::LocalGet(ret_idx));
        }

        // End function
        wasm_func.instruction(&Instruction::End);
        self.code_section.function(&wasm_func);
    }

    fn finish(mut self) -> wasm_encoder::Module {
        let mut module = wasm_encoder::Module::new();
        module.section(&self.type_section);

        if !self.import_section.is_empty() {
            module.section(&self.import_section);
        }

        module.section(&self.fn_section);

        // Memory section — always at least 1 page for pointer operations
        let total_mem = self.module.memory_layout.total_size();
        let pages = if total_mem > 0 {
            total_mem.div_ceil(65536)
        } else {
            1
        };
        let mut mem_section = wasm_encoder::MemorySection::new();
        mem_section.memory(wasm_encoder::MemoryType {
            minimum: pages as u64,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        module.section(&mem_section);

        // Export memory
        self.export_section.export("memory", wasm_encoder::ExportKind::Memory, 0);

        module.section(&self.export_section);
        module.section(&self.code_section);

        // Data section for string literals
        if !self.module.string_data.is_empty() {
            let mut data_section = wasm_encoder::DataSection::new();
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
fn build_signature(params: &[MirParam], return_type: &Option<MirType>) -> (Vec<ValType>, Vec<ValType>) {
    let mut wasm_params = Vec::new();
    for param in params {
        match param.kind {
            MirParamKind::This | MirParamKind::InOut | MirParamKind::Output => {
                wasm_params.push(ValType::I32); // pointer
            }
            MirParamKind::Input => {
                match &param.ty {
                    MirType::String(_) => {
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
        Some(ty) => mir_type_to_val_type(ty).map(|vt| vec![vt]).unwrap_or_default(),
        None => Vec::new(),
    };

    (wasm_params, results)
}

/// Build LocalInfo map from a MirFunction's params and locals.
pub(crate) fn build_local_map(func: &MirFunction) -> FxHashMap<hir::hir_def::interned::identifier::Ident, LocalInfo> {
    let mut map = FxHashMap::default();
    let mut param_idx: u32 = 0;

    // Parameters — mapped by their position in the WASM signature
    for param in &func.params {
        match param.kind {
            MirParamKind::This => {
                map.insert(param.name, LocalInfo::Pointer { index: param_idx, pointee_elem: None });
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
                map.insert(param.name, LocalInfo::Pointer { index: param_idx, pointee_elem });
                param_idx += 1;
            }
            MirParamKind::Input => {
                match &param.ty {
                    MirType::String(_) => {
                        // String params take 2 WASM params (ptr, len)
                        map.insert(param.name, LocalInfo::StringParam {
                            ptr_index: param_idx,
                            len_index: param_idx + 1,
                        });
                        param_idx += 2;
                    }
                    ty => {
                        let elem = match ty {
                            MirType::Elementary(e) => *e,
                            MirType::Pointer(_) => MirElementary::Int,
                            _ => MirElementary::Int,
                        };
                        let val_type = mir_elementary_to_val_type(elem);
                        map.insert(param.name, LocalInfo::Scalar {
                            index: param_idx,
                            val_type,
                            elem,
                        });
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
                map.insert(local.name, LocalInfo::Scalar {
                    index: local_index,
                    val_type,
                    elem,
                });
            }
            MirStorage::Memory { address, size, align } => {
                match &local.ty {
                    MirType::String(_) => {
                        map.insert(local.name, LocalInfo::StringMemory { address });
                    }
                    ty => {
                        let elem = match ty {
                            MirType::Elementary(e) => Some(*e),
                            _ => None,
                        };
                        map.insert(local.name, LocalInfo::Memory { address, size, align, elem });
                    }
                }
            }
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
        if elem.is_64bit() { ValType::F64 } else { ValType::F32 }
    } else if elem.is_64bit() {
        ValType::I64
    } else {
        ValType::I32
    }
}
