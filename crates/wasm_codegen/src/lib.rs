//! WebAssembly code generation for IEC 61131-3 programs.

use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        expressions::statement::StmtKind,
        extern_decl::ExternDecl,
        interned::identifier::Ident,
        pous::{function::Function, pou::Pou},
        program::ProgramDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{head::signature::infer_signature, infer::Infer},
};
use rustc_hash::FxHashMap;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub mod body;
pub mod cast;
pub mod debug;
pub mod emitter;
pub mod func_codegen;
pub mod memory;
pub mod wasm_repr;

#[cfg(test)]
pub mod tests;

use debug::{CodeGenConfig, DebugInfo};
use func_codegen::FunctionCodegen;
use memory::MemoryLayout;
use wasm_repr::WasmRepr;

/// A parameter/return type in the component model log.
/// Distinguishes scalar WASM types from string types.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ComponentType {
    /// A primitive WASM type (i32, i64, f32, f64).
    Val(wasm_encoder::ValType),
    /// A string type (ptr + len at core level, `string` at component level).
    String,
}

/// Info for an ANY_* extern function that needs monomorphization at call sites.
#[derive(Clone)]
pub(crate) struct AnyExternInfo<'db> {
    pub func: Function<'db>,
    pub extern_decl: ExternDecl<'db>,
}

pub struct ModuleCodeGen<'db> {
    db: &'db dyn WorkspaceDataBase,
    type_section: wasm_encoder::TypeSection,
    pub(crate) import_section: wasm_encoder::ImportSection,
    fn_section: wasm_encoder::FunctionSection,
    export_section: wasm_encoder::ExportSection,
    code_section: wasm_encoder::CodeSection,
    global_section: wasm_encoder::GlobalSection,

    // Index tracking
    next_type_idx: u32,
    next_fn_idx: u32,
    pub(crate) num_imports: u32,

    // Scope memoization
    scopes: FxHashMap<ScopeId<'db>, ScopeCodegenInfo>,

    // Function name → index mapping for call resolution
    function_indices: FxHashMap<hir::hir_def::interned::identifier::Ident, u32>,

    // ANY_* extern functions pending monomorphization (imports)
    // Key: function name Ident
    pub(crate) any_extern_functions: FxHashMap<Ident, AnyExternInfo<'db>>,
    // Monomorphized import name → fn_idx (e.g. "ABS.INT" → 5)
    pub(crate) monomorphized_indices: FxHashMap<String, u32>,

    // Non-extern ANY_* functions pending monomorphization (local functions)
    // Key: function name Ident
    any_local_functions: FxHashMap<Ident, Function<'db>>,

    // Tracked core imports: (module, name, param_types, result_type)
    core_import_log: Vec<(String, String, Vec<ComponentType>, Option<ComponentType>)>,
    // Tracked core exports: (name, param_types, result_type)
    core_export_log: Vec<(String, Vec<ComponentType>, Option<ComponentType>)>,

    // Memory layout for arrays, structs, and memory-resident variables
    memory_layout: MemoryLayout,

    // String literal data section: (offset_in_data, bytes) — shared with body codegen
    string_data: Rc<RefCell<Vec<(u32, Vec<u8>)>>>,
    // Next available offset in the string data area
    string_data_offset: Rc<Cell<u32>>,

    // Debug configuration and tracking
    pub config: CodeGenConfig,
    pub debug_info: DebugInfo,
    pub debug_enabled_global: Option<u32>,
    pub debug_trap_id_global: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct ScopeCodegenInfo {
    type_idx: u32,
    fn_idx: u32,
}

impl<'db> ModuleCodeGen<'db> {
    pub fn new(db: &'db dyn WorkspaceDataBase) -> Self {
        Self::new_with_config(db, CodeGenConfig::default())
    }

    pub fn new_with_config(db: &'db dyn WorkspaceDataBase, config: CodeGenConfig) -> Self {
        let mut global_section = wasm_encoder::GlobalSection::new();

        // Add debug globals if debug mode is enabled
        let (debug_enabled_global, debug_trap_id_global) =
            if config.debug_mode != debug::DebugMode::None {
                // Global 0: debug_enabled (i32, mutable)
                let debug_enabled_idx = global_section.len();
                global_section.global(
                    wasm_encoder::GlobalType {
                        val_type: wasm_encoder::ValType::I32,
                        mutable: true,
                        shared: false,
                    },
                    &wasm_encoder::ConstExpr::i32_const(0), // Default: disabled
                );

                // Global 1: debug_trap_id (i32, mutable)
                let debug_trap_id_idx = global_section.len();
                global_section.global(
                    wasm_encoder::GlobalType {
                        val_type: wasm_encoder::ValType::I32,
                        mutable: true,
                        shared: false,
                    },
                    &wasm_encoder::ConstExpr::i32_const(0),
                );

                (Some(debug_enabled_idx), Some(debug_trap_id_idx))
            } else {
                (None, None)
            };

        Self {
            db,
            type_section: Default::default(),
            import_section: Default::default(),
            fn_section: Default::default(),
            export_section: Default::default(),
            code_section: Default::default(),
            global_section,
            next_type_idx: 0,
            next_fn_idx: 0,
            num_imports: 0,
            scopes: Default::default(),
            function_indices: Default::default(),
            any_extern_functions: Default::default(),
            monomorphized_indices: Default::default(),
            any_local_functions: Default::default(),
            core_import_log: Vec::new(),
            core_export_log: Vec::new(),
            memory_layout: MemoryLayout::new(),
            string_data: Rc::new(RefCell::new(Vec::new())),
            string_data_offset: Rc::new(Cell::new(0)),
            config,
            debug_info: DebugInfo::new(),
            debug_enabled_global,
            debug_trap_id_global,
        }
    }

    /// Fixed base address for string data in linear memory.
    /// Placed at start of second page (64KiB) to avoid overlap with variable layout.
    pub const STRING_DATA_BASE: u32 = 65536;

    /// Generate a WebAssembly module from the given program declaration.
    pub fn generate_from_program(&mut self, program: ProgramDecl<'db>) -> wasm_encoder::Module {
        // Generate code for the program scope
        self.generate_scope(program.scope_id(self.db));

        // Build the module
        let mut module = wasm_encoder::Module::new();
        module.section(&self.type_section);
        if self.num_imports > 0 {
            module.section(&self.import_section);
        }
        module.section(&self.fn_section);

        // Add memory section if we allocated any memory
        if self.memory_layout.total_size() > 0 {
            let memory_min_pages = self.memory_layout.total_size().div_ceil(65536); // Round up to pages
            let mut memory_section = wasm_encoder::MemorySection::new();
            memory_section.memory(wasm_encoder::MemoryType {
                minimum: memory_min_pages as u64,
                maximum: None, // No maximum limit
                memory64: false,
                shared: false,
                page_size_log2: None,
            });
            module.section(&memory_section);
        }

        // Add global section if we have debug globals
        if !self.global_section.is_empty() {
            module.section(&self.global_section);
        }

        module.section(&self.export_section);
        module.section(&self.code_section);

        // TODO: Add custom debug sections here
        // Debug info is collected in self.debug_collector
        // You can serialize it and add as custom section for the debugger

        module
    }

    fn generate_scope(&mut self, scope_id: ScopeId<'db>) {
        // We memoize and avoid generating code for the same scope multiple times
        if self.scopes.contains_key(&scope_id) {
            return;
        }

        match get_scope(self.db, scope_id).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(func) => {
                    self.generate_function(func);
                }
                Pou::FunctionBlock(func_block) => {
                    self.generate_function_block(func_block);
                }
                Pou::Class(class) => {
                    self.generate_class(class);
                }
                _ => {}
            },
            ScopeKind::Program(program) => {
                self.generate_program(program);
            }
            _ => {}
        }
    }

    /// Extract the first extern pragma from a function's statements, if any.
    fn find_extern_decl(&self, func: Function<'db>) -> Option<ExternDecl<'db>> {
        func.statements(self.db).iter().find_map(|stmt| {
            if let StmtKind::ExternPragma(decl) = stmt.stmt(self.db) {
                Some(decl.clone())
            } else {
                None
            }
        })
    }

    /// Build the WASM parameter and result types for a function signature.
    fn build_func_signature(
        &self,
        func: Function<'db>,
    ) -> (Vec<wasm_encoder::ValType>, Vec<wasm_encoder::ValType>) {
        let scope_id = func.scope_id(self.db);
        let _signature = infer_signature(self.db, scope_id);
        let def_map = scope_id.def_map(self.db);

        let mut param_types = Vec::new();
        for (_name, var) in &def_map.local_variables {
            use hir::hir_def::pous::variable::VariableKind;
            match var.kind(self.db) {
                VariableKind::Input => {
                    if let Ok(repr) = WasmRepr::from_type(self.db, var.spec(self.db).infer(self.db))
                    {
                        param_types.extend(repr.flatten());
                    }
                }
                VariableKind::InOut => {
                    param_types.push(wasm_encoder::ValType::I32);
                }
                _ => {}
            }
        }

        let mut result_types = Vec::new();
        if let Some(return_spec) = func.return_type(self.db)
            && let Ok(repr) = WasmRepr::from_type(self.db, return_spec.infer(self.db))
        {
            result_types.extend(repr.flatten());
        }

        (param_types, result_types)
    }

    /// Build a component-level signature (using ComponentType) for import/export logging.
    fn build_component_signature(
        &self,
        func: Function<'db>,
    ) -> (Vec<ComponentType>, Option<ComponentType>) {
        let scope_id = func.scope_id(self.db);
        let _signature = infer_signature(self.db, scope_id);
        let def_map = scope_id.def_map(self.db);

        let mut params = Vec::new();
        for (_name, var) in &def_map.local_variables {
            use hir::hir_def::pous::variable::VariableKind;
            match var.kind(self.db) {
                VariableKind::Input => {
                    if let Ok(repr) = WasmRepr::from_type(self.db, var.spec(self.db).infer(self.db))
                    {
                        match repr {
                            WasmRepr::StringPtr => params.push(ComponentType::String),
                            WasmRepr::Scalar(vt) => params.push(ComponentType::Val(vt)),
                            WasmRepr::Memory { .. } => params.push(ComponentType::Val(wasm_encoder::ValType::I32)),
                        }
                    }
                }
                VariableKind::InOut => {
                    params.push(ComponentType::Val(wasm_encoder::ValType::I32));
                }
                _ => {}
            }
        }

        let result = func.return_type(self.db).and_then(|return_spec| {
            let repr = WasmRepr::from_type(self.db, return_spec.infer(self.db)).ok()?;
            Some(match repr {
                WasmRepr::StringPtr => ComponentType::String,
                WasmRepr::Scalar(vt) => ComponentType::Val(vt),
                WasmRepr::Memory { .. } => ComponentType::Val(wasm_encoder::ValType::I32),
            })
        });

        (params, result)
    }

    /// Generate an import for an extern function (no body, backed by WASM import).
    fn generate_import(&mut self, func: Function<'db>, extern_decl: &ExternDecl<'db>) {
        // Skip if already registered
        if self.function_indices.contains_key(&func.name(self.db)) {
            return;
        }

        let scope_id = func.scope_id(self.db);
        let (param_types, result_types) = self.build_func_signature(func);

        // Add function type signature
        let type_idx = self.next_type_idx;
        self.type_section
            .ty()
            .function(param_types.clone(), result_types.clone());
        self.next_type_idx += 1;

        // Add import entry
        let module = extern_decl.module.as_str();
        let name = extern_decl.name.as_str();
        self.import_section.import(
            module,
            name,
            wasm_encoder::EntityType::Function(type_idx),
        );

        // Log for component model (using component-level types)
        let (comp_params, comp_result) = self.build_component_signature(func);
        self.core_import_log.push((
            module.to_string(),
            name.to_string(),
            comp_params,
            comp_result,
        ));

        // Imported functions get indices starting from 0, before local functions
        let fn_idx = self.next_fn_idx;
        self.next_fn_idx += 1;
        self.num_imports += 1;

        // Export with the IEC function name (re-export the import)
        let func_name = func.name(self.db).text(self.db);
        self.export_section
            .export(func_name.as_str(), wasm_encoder::ExportKind::Func, fn_idx);

        // Store function name → index mapping
        self.function_indices.insert(func.name(self.db), fn_idx);

        // Store codegen info for memoization
        self.scopes
            .insert(scope_id, ScopeCodegenInfo { type_idx, fn_idx });
    }

    /// Check if a function has ANY_* types in its signature.
    fn has_any_types(&self, func: Function<'db>) -> bool {
        use hir::hir_def::expressions::spec::ElementarySpec;
        if let Some(ret) = func.return_type(self.db) {
            if let hir::hir_ty::ty::Type::Elementary(e) = ret.infer(self.db) {
                if e.is_any() {
                    return true;
                }
            }
        }
        let scope_id = func.scope_id(self.db);
        let def_map = scope_id.def_map(self.db);
        for (_name, var) in &def_map.local_variables {
            if let hir::hir_ty::ty::Type::Elementary(e) = var.spec(self.db).infer(self.db) {
                if e.is_any() {
                    return true;
                }
            }
        }
        false
    }

    /// Generate code for a function.
    fn generate_function(&mut self, func: Function<'db>) {
        // Check if this function has an extern pragma — if so, emit an import instead
        if let Some(extern_decl) = self.find_extern_decl(func) {
            // If the function has ANY_* types, defer monomorphization to call sites
            if self.has_any_types(func) {
                self.any_extern_functions.insert(
                    func.name(self.db),
                    AnyExternInfo { func, extern_decl },
                );
                return;
            }
            self.generate_import(func, &extern_decl);
            return;
        }

        // Non-extern functions with ANY_* types: store for monomorphization
        if self.has_any_types(func) {
            self.any_local_functions.insert(func.name(self.db), func);
            return;
        }

        // Skip if already registered (dedup across files/namespaces)
        if self.function_indices.contains_key(&func.name(self.db)) {
            return;
        }

        let scope_id = func.scope_id(self.db);
        let (param_types, result_types) = self.build_func_signature(func);

        // Build component-level signature for logging
        let (comp_params, comp_result) = self.build_component_signature(func);

        // Add function type signature
        let type_idx = self.next_type_idx;
        self.type_section.ty().function(param_types, result_types);
        self.next_type_idx += 1;

        // Declare the function
        let fn_idx = self.next_fn_idx;
        self.fn_section.function(type_idx);
        self.next_fn_idx += 1;

        // Export the function with its name (test functions get a __test__ prefix)
        let func_name = func.name(self.db).text(self.db);
        let export_name = if func.is_test(self.db) {
            format!("__test__{}", func_name)
        } else {
            func_name.to_string()
        };
        self.export_section
            .export(&export_name, wasm_encoder::ExportKind::Func, fn_idx);

        // Log for component model
        self.core_export_log.push((export_name, comp_params, comp_result));

        // Store function name → index mapping
        self.function_indices.insert(func.name(self.db), fn_idx);

        // Store codegen info for memoization
        self.scopes
            .insert(scope_id, ScopeCodegenInfo { type_idx, fn_idx });

        // Generate function body
        let codegen = FunctionCodegen::new(
            self.db,
            scope_id,
            &self.function_indices,
            &self.monomorphized_indices,
            &self.any_extern_functions,
            &self.any_local_functions,
            &self.config,
            &mut self.debug_info,
            self.debug_enabled_global,
            self.debug_trap_id_global,
            self.string_data.clone(),
            self.string_data_offset.clone(),
        );
        let func_name_dbg = func.name(self.db).text(self.db).to_string();
        let func_body = codegen
            .generate(&mut self.memory_layout)
            .unwrap_or_else(|e| panic!("Failed to generate function body for '{}': {}", func_name_dbg, e));
        self.code_section.function(&func_body);
    }

    /// Generate code for a function block and its methods.
    pub fn generate_function_block(
        &mut self,
        fb: hir::hir_def::pous::function_block::FunctionBlock<'db>,
    ) {
        use hir::hir_def::pous::variable::VariableKind;

        // Generate code for each method in the function block
        for method in fb.methods(self.db) {
            let scope_id = method.scope_id(self.db);
            let def_map = scope_id.def_map(self.db);

            // Build parameter types with implicit 'this' pointer as first parameter
            let mut param_types = vec![wasm_encoder::ValType::I32]; // 'this' pointer

            // Add explicit parameters (Input, InOut) - not all local variables!
            for (_name, var) in &def_map.local_variables {
                match var.kind(self.db) {
                    VariableKind::Input => {
                        // VAR_INPUT parameters are passed by value
                        if let Ok(repr) =
                            WasmRepr::from_type(self.db, var.spec(self.db).infer(self.db))
                        {
                            param_types.extend(repr.flatten());
                        }
                    }
                    VariableKind::InOut => {
                        // VAR_IN_OUT parameters are passed as i32 pointers
                        param_types.push(wasm_encoder::ValType::I32);
                    }
                    _ => {
                        // VAR, VAR_TEMP, VAR_OUTPUT, etc. are NOT parameters
                    }
                }
            }

            // Build return type
            let mut result_types = Vec::new();
            if let Some(return_spec) = method.return_type(self.db)
                && let Ok(repr) = WasmRepr::from_type(self.db, return_spec.infer(self.db))
            {
                result_types.extend(repr.flatten());
            }

            // Add method type signature
            let type_idx = self.next_type_idx;
            self.type_section.ty().function(param_types, result_types);
            self.next_type_idx += 1;

            // Declare the method as a WASM function
            let fn_idx = self.next_fn_idx;
            self.fn_section.function(type_idx);
            self.next_fn_idx += 1;

            // Export the method with qualified name: FB_Name$Method_Name
            let fb_name = fb.name(self.db).text(self.db);
            let method_name = method.name(self.db).text(self.db);
            let qualified_name = format!("{}${}", fb_name, method_name);
            self.export_section
                .export(&qualified_name, wasm_encoder::ExportKind::Func, fn_idx);

            // Store qualified method name → index mapping
            let qualified_ident = Ident::from_slice(self.db, &qualified_name);
            self.function_indices.insert(qualified_ident, fn_idx);

            // Store codegen info for memoization
            self.scopes
                .insert(scope_id, ScopeCodegenInfo { type_idx, fn_idx });

            // Generate method body (with implicit 'this' parameter handling)
            let codegen = FunctionCodegen::new_with_fb(
                self.db,
                scope_id,
                &self.function_indices,
                &self.monomorphized_indices,
                &self.any_extern_functions,
                &self.any_local_functions,
                fb,
                &self.config,
                &mut self.debug_info,
                self.debug_enabled_global,
                self.debug_trap_id_global,
                self.string_data.clone(),
                self.string_data_offset.clone(),
            );
            let func_body = codegen
                .generate(&mut self.memory_layout)
                .expect("Failed to generate method body");
            self.code_section.function(&func_body);
        }
    }

    /// Generate WASM code for a CLASS and its methods.
    ///
    /// Classes are similar to function blocks - they have instance variables and methods.
    /// Each method gets an implicit 'this' pointer as its first parameter.
    /// Methods are exported with the naming convention: ClassName$MethodName
    pub fn generate_class(&mut self, class: hir::hir_def::pous::class::Class<'db>) {
        use hir::hir_def::pous::variable::VariableKind;

        // Generate code for each method in the class
        for method in class.methods(self.db) {
            let scope_id = method.scope_id(self.db);
            let def_map = scope_id.def_map(self.db);

            // Build parameter types with implicit 'this' pointer as first parameter
            let mut param_types = vec![wasm_encoder::ValType::I32]; // 'this' pointer

            // Add explicit parameters (Input, InOut) - not all local variables!
            for (_name, var) in &def_map.local_variables {
                match var.kind(self.db) {
                    VariableKind::Input => {
                        // VAR_INPUT parameters are passed by value
                        if let Ok(repr) =
                            WasmRepr::from_type(self.db, var.spec(self.db).infer(self.db))
                        {
                            param_types.extend(repr.flatten());
                        }
                    }
                    VariableKind::InOut => {
                        // VAR_IN_OUT parameters are passed as i32 pointers
                        param_types.push(wasm_encoder::ValType::I32);
                    }
                    _ => {
                        // VAR, VAR_TEMP, VAR_OUTPUT, etc. are NOT parameters
                    }
                }
            }

            // Build return type
            let mut result_types = Vec::new();
            if let Some(return_spec) = method.return_type(self.db)
                && let Ok(repr) = WasmRepr::from_type(self.db, return_spec.infer(self.db))
            {
                result_types.extend(repr.flatten());
            }

            // Add method type signature
            let type_idx = self.next_type_idx;
            self.type_section.ty().function(param_types, result_types);
            self.next_type_idx += 1;

            // Declare the method as a WASM function
            let fn_idx = self.next_fn_idx;
            self.fn_section.function(type_idx);
            self.next_fn_idx += 1;

            // Export the method with qualified name: ClassName$MethodName
            let class_name = class.name(self.db).text(self.db);
            let method_name = method.name(self.db).text(self.db);
            let qualified_name = format!("{}${}", class_name, method_name);
            self.export_section
                .export(&qualified_name, wasm_encoder::ExportKind::Func, fn_idx);

            // Store qualified method name → index mapping
            let qualified_ident = Ident::from_slice(self.db, &qualified_name);
            self.function_indices.insert(qualified_ident, fn_idx);

            // Store codegen info for memoization
            self.scopes
                .insert(scope_id, ScopeCodegenInfo { type_idx, fn_idx });

            // Generate method body (with implicit 'this' parameter handling)
            // We pass the class as a FunctionBlock-like entity for instance variable access
            // TODO: Handle inheritance by traversing parent class fields
            let codegen = FunctionCodegen::new_with_class(
                self.db,
                scope_id,
                &self.function_indices,
                &self.monomorphized_indices,
                &self.any_extern_functions,
                &self.any_local_functions,
                class,
                &self.config,
                &mut self.debug_info,
                self.debug_enabled_global,
                self.debug_trap_id_global,
                self.string_data.clone(),
                self.string_data_offset.clone(),
            );
            let func_body = codegen
                .generate(&mut self.memory_layout)
                .expect("Failed to generate method body");
            self.code_section.function(&func_body);
        }
    }

    /// Generate WASM code for a PROGRAM POU.
    ///
    /// Programs are entry points for cyclic execution. They have:
    /// - No parameters and no return value
    /// - Variables (including FB/CLASS instances) allocated in linear memory
    /// - A body that executes when called
    ///
    /// The program is exported as a zero-parameter function with the program name.
    pub fn generate_program(&mut self, program: hir::hir_def::program::ProgramDecl<'db>) {
        let scope_id = program.scope_id(self.db);

        // Programs have no parameters and no return value
        let param_types = Vec::new();
        let result_types = Vec::new();

        // Add program type signature
        let type_idx = self.next_type_idx;
        self.type_section.ty().function(param_types, result_types);
        self.next_type_idx += 1;

        // Declare the program as a WASM function
        let fn_idx = self.next_fn_idx;
        self.fn_section.function(type_idx);
        self.next_fn_idx += 1;

        // Export the program with its name (test programs get a __test__ prefix)
        let program_name = program.name(self.db).text(self.db);
        let export_name = if program.is_test(self.db) {
            format!("__test__{}", program_name)
        } else {
            program_name.to_string()
        };
        self.export_section
            .export(&export_name, wasm_encoder::ExportKind::Func, fn_idx);

        // Store program name → index mapping
        self.function_indices.insert(program.name(self.db), fn_idx);

        // Store codegen info for memoization
        self.scopes
            .insert(scope_id, ScopeCodegenInfo { type_idx, fn_idx });

        // Generate program body
        // Note: PROGRAM variables are allocated in linear memory during build_local_map
        let codegen = FunctionCodegen::new(
            self.db,
            scope_id,
            &self.function_indices,
            &self.monomorphized_indices,
            &self.any_extern_functions,
            &self.any_local_functions,
            &self.config,
            &mut self.debug_info,
            self.debug_enabled_global,
            self.debug_trap_id_global,
            self.string_data.clone(),
            self.string_data_offset.clone(),
        );
        let func_body = codegen
            .generate(&mut self.memory_layout)
            .expect("Failed to generate program body");
        self.code_section.function(&func_body);

        // Phase 6: Programs don't collect debug info currently (no fn_idx to reference)
        // TODO: Add program debug collection if needed
    }

    /// Check if a function has an extern pragma (without extracting it).
    fn has_extern_decl(&self, func: Function<'db>) -> bool {
        func.statements(self.db)
            .iter()
            .any(|stmt| matches!(stmt.stmt(self.db), StmtKind::ExternPragma(_)))
    }

    /// Generate all POUs from a semantic index, processing imports before local functions.
    ///
    /// WASM requires that all imported functions have indices 0..N-1 and local
    /// functions have indices N..M-1. This method ensures correct ordering by
    /// doing two passes: first extern (import) functions, then local functions.
    /// Generate monomorphized imports for an ANY_* extern function.
    ///
    /// For each concrete type in the ANY_* group, generate a WASM import with
    /// the type-suffixed name (e.g. `abs.INT`, `abs.REAL`).
    fn generate_monomorphized_imports(&mut self, info: &AnyExternInfo<'db>) {
        use hir::hir_def::expressions::spec::ElementarySpec;

        // Determine which return type ANY group we're dealing with
        let any_spec = if let Some(ret) = info.func.return_type(self.db) {
            if let hir::hir_ty::ty::Type::Elementary(e) = ret.infer(self.db) {
                e
            } else {
                return;
            }
        } else {
            return;
        };

        // Get all concrete types in this ANY group
        let concrete_types: &[ElementarySpec] = match any_spec {
            ElementarySpec::AnyNum => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
                ElementarySpec::Real, ElementarySpec::LReal,
            ],
            ElementarySpec::AnyInt => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
            ],
            ElementarySpec::AnySigned => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
            ],
            ElementarySpec::AnyUnsigned => &[
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
            ],
            ElementarySpec::AnyReal => &[ElementarySpec::Real, ElementarySpec::LReal],
            ElementarySpec::AnyBit => &[
                ElementarySpec::Bool, ElementarySpec::Byte, ElementarySpec::Word,
                ElementarySpec::DWord, ElementarySpec::LWord,
            ],
            ElementarySpec::AnyMagnitude => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
                ElementarySpec::Real, ElementarySpec::LReal,
                ElementarySpec::Time, ElementarySpec::LTime,
            ],
            ElementarySpec::AnyElementary | ElementarySpec::Any => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
                ElementarySpec::Real, ElementarySpec::LReal,
                ElementarySpec::Bool, ElementarySpec::Byte, ElementarySpec::Word,
                ElementarySpec::DWord, ElementarySpec::LWord,
            ],
            _ => return,
        };

        for &concrete_type in concrete_types {
            let val_type = match wasm_repr::elementary::elementary_to_val_type(concrete_type) {
                Ok(vt) => vt,
                Err(_) => continue,
            };

            let type_suffix = concrete_type.type_name();
            let mono_key = format!("{}.{}", info.extern_decl.name, type_suffix);

            // Skip if already generated (dedup across generate_all calls)
            let func_name = info.func.name(self.db).text(self.db);
            let lookup_key = format!("{}.{}", func_name, type_suffix);
            if self.monomorphized_indices.contains_key(&lookup_key) {
                continue;
            }

            // Build param types
            let param_types: Vec<_> = info.extern_decl.params.iter().map(|_| val_type).collect();
            let result_types: Vec<_> = if info.extern_decl.result.is_some() {
                vec![val_type]
            } else {
                vec![]
            };

            // Add type signature
            let type_idx = self.next_type_idx;
            self.type_section
                .ty()
                .function(param_types.clone(), result_types.clone());
            self.next_type_idx += 1;

            // Add import
            let module_name = info.extern_decl.module.as_str();
            self.import_section.import(
                module_name,
                &mono_key,
                wasm_encoder::EntityType::Function(type_idx),
            );

            // Log for component model (monomorphized types are always numeric, never strings)
            self.core_import_log.push((
                module_name.to_string(),
                mono_key.clone(),
                param_types.iter().map(|vt| ComponentType::Val(*vt)).collect(),
                result_types.first().map(|vt| ComponentType::Val(*vt)),
            ));

            let fn_idx = self.next_fn_idx;
            self.next_fn_idx += 1;
            self.num_imports += 1;

            // Store the monomorphized index under a combined key
            let func_name = info.func.name(self.db).text(self.db);
            let lookup_key = format!("{}.{}", func_name, type_suffix);
            self.monomorphized_indices.insert(lookup_key, fn_idx);
        }
    }

    /// Generate monomorphized versions of a non-extern ANY_* function.
    /// E.g., ASSERT_EQ(value: ANY, target: INTO(value)) generates
    /// ASSERT_EQ_INT, ASSERT_EQ_REAL, etc.
    fn generate_monomorphized_local(&mut self, func: Function<'db>) {
        use hir::hir_def::expressions::spec::ElementarySpec;

        // Skip variadic functions (need FoldExpr support)
        let scope_id = func.scope_id(self.db);
        let def_map = scope_id.def_map(self.db);
        let has_variadic = def_map.local_variables.values().any(|v| v.variadic(self.db));
        if has_variadic {
            return;
        }

        // Find the ANY_* spec to determine which concrete types to generate
        let any_spec = self.find_any_spec(func);
        let any_spec = match any_spec {
            Some(s) => s,
            None => return,
        };

        // Get the concrete types for this ANY group
        let concrete_types: &[ElementarySpec] = match any_spec {
            ElementarySpec::AnyNum => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
                ElementarySpec::Real, ElementarySpec::LReal,
            ],
            ElementarySpec::AnyInt => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
            ],
            ElementarySpec::AnySigned => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
            ],
            ElementarySpec::AnyUnsigned => &[
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
            ],
            ElementarySpec::AnyReal => &[ElementarySpec::Real, ElementarySpec::LReal],
            ElementarySpec::AnyBit => &[
                ElementarySpec::Bool, ElementarySpec::Byte, ElementarySpec::Word,
                ElementarySpec::DWord, ElementarySpec::LWord,
            ],
            ElementarySpec::AnyMagnitude => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
                ElementarySpec::Real, ElementarySpec::LReal,
            ],
            ElementarySpec::AnyElementary | ElementarySpec::Any => &[
                ElementarySpec::SInt, ElementarySpec::Int, ElementarySpec::DInt, ElementarySpec::LInt,
                ElementarySpec::USInt, ElementarySpec::UInt, ElementarySpec::UDInt, ElementarySpec::ULInt,
                ElementarySpec::Real, ElementarySpec::LReal,
                ElementarySpec::Bool, ElementarySpec::Byte, ElementarySpec::Word,
                ElementarySpec::DWord, ElementarySpec::LWord,
            ],
            _ => return,
        };

        let func_name = func.name(self.db).text(self.db);
        let scope_id = func.scope_id(self.db);

        for &concrete_type in concrete_types {
            let val_type = match wasm_repr::elementary::elementary_to_val_type(concrete_type) {
                Ok(vt) => vt,
                Err(_) => continue,
            };

            let type_suffix = concrete_type.type_name();
            let lookup_key = format!("{}.{}", func_name, type_suffix);

            // Skip if already generated
            if self.monomorphized_indices.contains_key(&lookup_key) {
                continue;
            }

            // Build concrete param types: replace ANY/INTO params with the concrete type
            let def_map = scope_id.def_map(self.db);
            let mut param_types = Vec::new();
            for (_name, var) in &def_map.local_variables {
                use hir::hir_def::pous::variable::VariableKind;
                if matches!(var.kind(self.db), VariableKind::Input) {
                    let var_type = var.spec(self.db).infer(self.db).normalize(self.db);
                    match var_type {
                        hir::hir_ty::ty::Type::Elementary(e) if e.is_any() => {
                            param_types.push(val_type);
                        }
                        _ => {
                            if let Ok(repr) = WasmRepr::from_type(self.db, var_type) {
                                param_types.extend(repr.flatten());
                            }
                        }
                    }
                }
            }

            // Build concrete return type
            let mut result_types = Vec::new();
            if let Some(return_spec) = func.return_type(self.db) {
                let ret_type = return_spec.infer(self.db).normalize(self.db);
                match ret_type {
                    hir::hir_ty::ty::Type::Elementary(e) if e.is_any() => {
                        result_types.push(val_type);
                    }
                    _ => {
                        if let Ok(repr) = WasmRepr::from_type(self.db, ret_type) {
                            result_types.extend(repr.flatten());
                        }
                    }
                }
            }

            // Add function type signature
            let type_idx = self.next_type_idx;
            self.type_section.ty().function(param_types.clone(), result_types.clone());
            self.next_type_idx += 1;

            // Declare the function
            let fn_idx = self.next_fn_idx;
            self.fn_section.function(type_idx);
            self.next_fn_idx += 1;

            // Export with monomorphized name
            let export_name = format!("{}_{}", func_name, type_suffix);
            self.export_section
                .export(&export_name, wasm_encoder::ExportKind::Func, fn_idx);

            // Store monomorphized index
            self.monomorphized_indices.insert(lookup_key, fn_idx);

            // Log for component model
            let comp_params: Vec<_> = param_types.iter().map(|vt| ComponentType::Val(*vt)).collect();
            let comp_result = result_types.first().map(|vt| ComponentType::Val(*vt));
            self.core_export_log.push((export_name.clone(), comp_params, comp_result));

            // Generate function body with concrete type override
            let codegen = FunctionCodegen::new(
                self.db,
                scope_id,
                &self.function_indices,
                &self.monomorphized_indices,
                &self.any_extern_functions,
                &self.any_local_functions,
                &self.config,
                &mut self.debug_info,
                self.debug_enabled_global,
                self.debug_trap_id_global,
                self.string_data.clone(),
                self.string_data_offset.clone(),
            );
            let func_body = codegen
                .generate_with_type_override(&mut self.memory_layout, concrete_type)
                .unwrap_or_else(|e| panic!(
                    "Failed to generate monomorphized body for '{}': {}", export_name, e
                ));
            self.code_section.function(&func_body);
        }
    }

    /// Find the ANY_* spec in a function's signature (params or return type).
    fn find_any_spec(&self, func: Function<'db>) -> Option<hir::hir_def::expressions::spec::ElementarySpec> {
        use hir::hir_def::expressions::spec::ElementarySpec;
        // Check return type first
        if let Some(ret) = func.return_type(self.db) {
            if let hir::hir_ty::ty::Type::Elementary(e) = ret.infer(self.db) {
                if e.is_any() {
                    return Some(e);
                }
            }
        }
        // Check input params
        let scope_id = func.scope_id(self.db);
        let def_map = scope_id.def_map(self.db);
        for (_name, var) in &def_map.local_variables {
            use hir::hir_def::pous::variable::VariableKind;
            if matches!(var.kind(self.db), VariableKind::Input) {
                if let hir::hir_ty::ty::Type::Elementary(e) = var.spec(self.db).infer(self.db) {
                    if e.is_any() {
                        return Some(e);
                    }
                }
            }
        }
        None
    }

    /// Recursively collect all POUs from a namespace and its children.
    fn collect_namespace_pous(
        &self,
        ns: &hir::hir_def::namespace::NamespaceDecl<'db>,
        pous: &mut Vec<Pou<'db>>,
    ) {
        pous.extend(ns.pous(self.db).iter().cloned());
        for child_ns in ns.namespaces(self.db).iter() {
            self.collect_namespace_pous(child_ns, pous);
        }
    }

    /// Collect all POUs from a semantic index (global + namespaced).
    fn collect_all_pous(&self, sem_idx: &hir::hir_def::semantic_index::SemanticIndex<'db>) -> Vec<Pou<'db>> {
        let mut all_pous: Vec<Pou<'db>> = sem_idx.global_pous.as_ref().clone();
        for ns in sem_idx.namespaces.iter() {
            self.collect_namespace_pous(ns, &mut all_pous);
        }
        all_pous
    }

    /// Generate WASM code from multiple semantic indices (one per file).
    ///
    /// This processes ALL files in two phases to ensure WASM import ordering:
    /// 1. All imports (extern functions + monomorphized ANY_*) across all files
    /// 2. All local functions, function blocks, classes, and programs
    pub fn generate_all_files(&mut self, sem_indices: &[&hir::hir_def::semantic_index::SemanticIndex<'db>]) {
        // Collect all POUs and programs from all files
        let mut all_pous = Vec::new();
        let mut all_programs = Vec::new();
        for sem_idx in sem_indices {
            all_pous.extend(self.collect_all_pous(sem_idx));
            all_programs.extend(sem_idx.programs.iter().cloned());
        }

        // Phase 1: ALL imports first (across all files)
        for pou in all_pous.iter() {
            if let Pou::Function(func) = pou {
                if self.has_extern_decl(*func) {
                    self.generate_function(*func);
                }
            }
        }

        // Phase 1b: Monomorphized imports for ANY_* extern functions
        let any_externs: Vec<_> = self.any_extern_functions.values().cloned().collect();
        for info in &any_externs {
            self.generate_monomorphized_imports(info);
        }

        // Phase 2a: Non-test local functions, FBs, classes (declarations first)
        for pou in all_pous.iter() {
            match pou {
                Pou::Function(func) => {
                    if !self.has_extern_decl(*func) && !func.is_test(self.db) {
                        self.generate_function(*func);
                    }
                }
                Pou::FunctionBlock(fb) => {
                    self.generate_function_block(*fb);
                }
                Pou::Class(class) => {
                    self.generate_class(*class);
                }
                _ => {}
            }
        }

        // Phase 2c: Monomorphized local functions (non-extern ANY_* functions)
        let any_locals: Vec<_> = self.any_local_functions.values().cloned().collect();
        for func in &any_locals {
            self.generate_monomorphized_local(*func);
        }

        // Phase 2b: Test functions (after all declarations are registered)
        for pou in all_pous.iter() {
            if let Pou::Function(func) = pou {
                if !self.has_extern_decl(*func) && func.is_test(self.db) {
                    self.generate_function(*func);
                }
            }
        }

        // Phase 3: ALL programs
        for program in all_programs.iter() {
            self.generate_program(*program);
        }
    }

    /// Generate WASM code from a single semantic index.
    ///
    /// WARNING: When compiling multiple files, use `generate_all_files` instead
    /// to ensure correct WASM import ordering.
    pub fn generate_all(&mut self, sem_idx: &hir::hir_def::semantic_index::SemanticIndex<'db>) {
        self.generate_all_files(&[sem_idx]);
    }

    /// Build the final WASM module with all generated code and debug information.
    ///
    /// This method consumes the codegen and returns the complete WASM binary as bytes.
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        // Build the module
        let mut module = wasm_encoder::Module::new();
        module.section(&self.type_section);
        if self.num_imports > 0 {
            module.section(&self.import_section);
        }
        module.section(&self.fn_section);

        // Add memory section
        let memory_size = self.memory_layout.total_size();
        let memory_min_pages = if memory_size > 0 {
            memory_size.div_ceil(65536)
        } else {
            1 // At least 1 page for pointer operations
        };

        let mut memory_section = wasm_encoder::MemorySection::new();
        memory_section.memory(wasm_encoder::MemoryType {
            minimum: memory_min_pages as u64,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });
        module.section(&memory_section);

        // Export section
        self.export_section
            .export("memory", wasm_encoder::ExportKind::Memory, 0);
        module.section(&self.export_section);

        // Code section
        module.section(&self.code_section);

        // TODO: Add custom debug sections here
        // Debug info is available via self.debug_collector.compilation_units()
        // Example:
        //
        // #[cfg(feature = "debug_info")]
        // {
        //     let debug_data = serialize_to_json(self.debug_collector.compilation_units());
        //     let custom_section = wasm_encoder::CustomSection {
        //         name: std::borrow::Cow::Borrowed("rk_debug"),
        //         data: std::borrow::Cow::Owned(debug_data),
        //     };
        //     module.section(&custom_section);
        // }

        Ok(module.finish())
    }

    /// Build a WASM Component wrapping the core module.
    ///
    /// This produces a Component Model binary that:
    /// - Embeds the core module
    /// - Defines component-level imports for each extern function
    /// - Lowers component imports → core functions
    /// - Bundles core functions into instances (one per import module)
    /// - Instantiates the core module with those instances
    /// - Lifts and exports all core exports as component functions
    pub fn finish_component(self) -> Result<Vec<u8>, String> {
        use wasm_encoder::{
            ComponentBuilder, ComponentExportKind, ComponentTypeRef, ExportKind, ModuleArg,
        };

        let uses_strings = self.has_string_types();
        // When strings are used, the core module imports memory so the component can
        // provide it before lowering imports (breaks the circular dependency).
        // Don't emit realloc in core module — the component's helper module handles it
        let core_module_bytes = self.build_core_module(false, uses_strings)?;

        let mut builder = ComponentBuilder::default();

        // 1. Embed the core module
        let core_module_idx = builder.core_module_raw(Some("iec-module"), &core_module_bytes);

        // 2. If strings are used, define an inline core memory and create canonical options
        let (lower_opts, memory_instance) = if uses_strings {
            use wasm_encoder::CanonicalOption;

            // Calculate memory size
            let string_base = Self::STRING_DATA_BASE;
            let string_end = string_base + self.string_data_offset.get();
            let layout_end = self.memory_layout.total_size();
            let heap_start = std::cmp::max(string_end, layout_end);
            let total_memory = (heap_start as usize) + 2 * 65536;
            let memory_min_pages = total_memory.div_ceil(65536);

            // Define inline core memory
            let core_memory_idx = {
                // Build a tiny core module that defines and exports memory
                let mut mem_module = wasm_encoder::Module::new();
                let mut mem_section = wasm_encoder::MemorySection::new();
                mem_section.memory(wasm_encoder::MemoryType {
                    minimum: memory_min_pages as u64,
                    maximum: None,
                    memory64: false,
                    shared: false,
                    page_size_log2: None,
                });
                mem_module.section(&mem_section);
                let mut exp_section = wasm_encoder::ExportSection::new();
                exp_section.export("memory", wasm_encoder::ExportKind::Memory, 0);
                mem_module.section(&exp_section);

                let mem_module_idx = builder.core_module_raw(None, &mem_module.finish());
                let mem_instance_idx = builder.core_instantiate(None, mem_module_idx, vec![]);
                builder.core_alias_export(None, mem_instance_idx, "memory", ExportKind::Memory)
            };

            // We also need a realloc function. Build it as a tiny core module too.
            let core_realloc_idx = {
                let mut realloc_module = wasm_encoder::Module::new();

                // Type: (i32, i32, i32, i32) -> i32
                let mut types = wasm_encoder::TypeSection::new();
                types.ty().function(vec![wasm_encoder::ValType::I32; 4], vec![wasm_encoder::ValType::I32]);
                realloc_module.section(&types);

                // Import the memory
                let mut imp = wasm_encoder::ImportSection::new();
                imp.import("env", "memory", wasm_encoder::MemoryType {
                    minimum: 0, maximum: None, memory64: false, shared: false, page_size_log2: None,
                });
                realloc_module.section(&imp);

                let mut fns = wasm_encoder::FunctionSection::new();
                fns.function(0);
                realloc_module.section(&fns);

                // Global for heap pointer
                let mut globals = wasm_encoder::GlobalSection::new();
                let heap_start_aligned = (heap_start + 15) & !15;
                globals.global(
                    wasm_encoder::GlobalType { val_type: wasm_encoder::ValType::I32, mutable: true, shared: false },
                    &wasm_encoder::ConstExpr::i32_const(heap_start_aligned as i32),
                );
                realloc_module.section(&globals);

                let mut exp = wasm_encoder::ExportSection::new();
                exp.export("cabi_realloc", wasm_encoder::ExportKind::Func, 0);
                realloc_module.section(&exp);

                // Bump allocator body
                use wasm_encoder::Instruction;
                let mut code = wasm_encoder::CodeSection::new();
                let mut func = wasm_encoder::Function::new([(1, wasm_encoder::ValType::I32)]);
                let result_local = 4u32;
                func.instruction(&Instruction::GlobalGet(0));
                func.instruction(&Instruction::LocalSet(result_local));
                func.instruction(&Instruction::GlobalGet(0));
                func.instruction(&Instruction::LocalGet(3)); // new_size
                func.instruction(&Instruction::I32Add);
                func.instruction(&Instruction::GlobalSet(0));
                func.instruction(&Instruction::LocalGet(result_local));
                func.instruction(&Instruction::End);
                code.function(&func);
                realloc_module.section(&code);

                let realloc_module_idx = builder.core_module_raw(None, &realloc_module.finish());
                // Instantiate realloc module with the shared memory
                let mem_exports = vec![("memory", ExportKind::Memory, core_memory_idx)];
                let env_instance = builder.core_instantiate_exports(None, mem_exports);
                let realloc_instance = builder.core_instantiate(None, realloc_module_idx,
                    vec![("env", ModuleArg::Instance(env_instance))]);
                builder.core_alias_export(None, realloc_instance, "cabi_realloc", ExportKind::Func)
            };

            let opts = vec![
                CanonicalOption::UTF8,
                CanonicalOption::Memory(core_memory_idx),
                CanonicalOption::Realloc(core_realloc_idx),
            ];

            // Create an "env" instance exporting the memory for the main module
            let env_exports = vec![("memory", ExportKind::Memory, core_memory_idx)];
            let env_instance_idx = builder.core_instantiate_exports(None, env_exports);

            (opts, Some(env_instance_idx))
        } else {
            (vec![], None)
        };

        // 3. For each core import, define a component function type,
        //    import it, and lower it to a core function
        let mut lowered_funcs: Vec<(String, String, u32)> = Vec::new();

        for (module, name, params, result) in &self.core_import_log {
            // Special case: assert.fail → provide as unreachable trap (avoids reentrance)
            if module == "assert" && name == "fail" {
                use wasm_encoder::Instruction;
                // Build a tiny core module with an unreachable function
                let mut trap_module = wasm_encoder::Module::new();
                let mut types = wasm_encoder::TypeSection::new();
                // Match the import's core type: () -> ()
                let core_params: Vec<wasm_encoder::ValType> = params.iter().flat_map(|ct| match ct {
                    ComponentType::Val(vt) => vec![*vt],
                    ComponentType::String => vec![wasm_encoder::ValType::I32, wasm_encoder::ValType::I32],
                }).collect();
                types.ty().function(core_params, vec![]);
                trap_module.section(&types);
                let mut fns = wasm_encoder::FunctionSection::new();
                fns.function(0);
                trap_module.section(&fns);
                let mut exp = wasm_encoder::ExportSection::new();
                exp.export("fail", wasm_encoder::ExportKind::Func, 0);
                trap_module.section(&exp);
                let mut code = wasm_encoder::CodeSection::new();
                let mut f = wasm_encoder::Function::new([]);
                f.instruction(&Instruction::Unreachable);
                f.instruction(&Instruction::End);
                code.function(&f);
                trap_module.section(&code);

                let trap_module_idx = builder.core_module_raw(None, &trap_module.finish());
                let trap_instance = builder.core_instantiate(None, trap_module_idx, vec![]);
                let core_func_idx = builder.core_alias_export(None, trap_instance, "fail", ExportKind::Func);

                lowered_funcs.push((module.clone(), name.clone(), core_func_idx));
                continue;
            }

            // Define component function type
            let param_names: Vec<String> =
                (0..params.len()).map(|i| format!("p{}", i)).collect();
            let (type_idx, enc) = builder.ty(None);
            {
                let mut fenc = enc.function();
                let cparams: Vec<(&str, wasm_encoder::ComponentValType)> = param_names
                    .iter()
                    .zip(params)
                    .map(|(n, ct)| (n.as_str(), component_type_to_component(*ct)))
                    .collect();
                fenc.params(cparams);
                if let Some(ret) = result {
                    fenc.result(Some(component_type_to_component(*ret)));
                } else {
                    fenc.result(None);
                }
            }

            // Import at component level
            let import_name = to_kebab_case(&format!("{}-{}", module, name.replace('.', "-")));
            let comp_func_idx = builder.import(&import_name, ComponentTypeRef::Func(type_idx));

            // Lower to core function (with canonical options if any function uses strings)
            let func_uses_strings = params.iter().any(|p| matches!(p, ComponentType::String))
                || matches!(result, Some(ComponentType::String));
            let opts = if func_uses_strings { lower_opts.clone() } else { vec![] };
            let core_func_idx = builder.lower_func(None, comp_func_idx, opts);

            lowered_funcs.push((module.clone(), name.clone(), core_func_idx));
        }

        // 4. Group lowered funcs by import module and create core instances
        let mut module_groups: Vec<(String, Vec<(String, u32)>)> = Vec::new();
        {
            let mut current_module = String::new();
            let mut current_funcs: Vec<(String, u32)> = Vec::new();

            for (module, name, core_func_idx) in &lowered_funcs {
                if *module != current_module {
                    if !current_funcs.is_empty() {
                        module_groups.push((current_module.clone(), current_funcs.clone()));
                        current_funcs.clear();
                    }
                    current_module = module.clone();
                }
                current_funcs.push((name.clone(), *core_func_idx));
            }
            if !current_funcs.is_empty() {
                module_groups.push((current_module, current_funcs));
            }
        }

        // Create core instances for each import module
        let mut module_instances: Vec<(String, u32)> = Vec::new();
        for (module_name, funcs) in &module_groups {
            let exports: Vec<(&str, ExportKind, u32)> = funcs
                .iter()
                .map(|(name, idx)| (name.as_str(), ExportKind::Func, *idx))
                .collect();
            let instance_idx = builder.core_instantiate_exports(None, exports);
            module_instances.push((module_name.clone(), instance_idx));
        }

        // 5. Instantiate the core module with the import instances + memory
        let mut args: Vec<(&str, ModuleArg)> = module_instances
            .iter()
            .map(|(name, idx)| (name.as_str(), ModuleArg::Instance(*idx)))
            .collect();
        if let Some(env_idx) = memory_instance {
            args.push(("env", ModuleArg::Instance(env_idx)));
        }
        let core_instance_idx =
            builder.core_instantiate(Some("iec-instance"), core_module_idx, args);

        // 6. For lifting exports, reuse the same canonical options (shared memory)
        let lift_opts = lower_opts.clone();

        // 7. Lift and export each core export
        for (name, params, result) in &self.core_export_log {
            // Convert to kebab-case for component model
            let kebab_name = to_kebab_case(name);

            // Alias the core export from the instance
            let core_func_idx =
                builder.core_alias_export(None, core_instance_idx, name, ExportKind::Func);

            // Define component function type for the export
            let param_names: Vec<String> =
                (0..params.len()).map(|i| format!("p{}", i)).collect();
            let (type_idx, enc) = builder.ty(None);
            {
                let mut fenc = enc.function();
                let cparams: Vec<(&str, wasm_encoder::ComponentValType)> = param_names
                    .iter()
                    .zip(params)
                    .map(|(n, ct)| (n.as_str(), component_type_to_component(*ct)))
                    .collect();
                fenc.params(cparams);
                if let Some(ret) = result {
                    fenc.result(Some(component_type_to_component(*ret)));
                } else {
                    fenc.result(None);
                }
            }

            // Lift the core function (with canonical options if function uses strings)
            let func_uses_strings = params.iter().any(|p| matches!(p, ComponentType::String))
                || matches!(result, Some(ComponentType::String));
            let opts = if func_uses_strings { lift_opts.clone() } else { vec![] };
            let comp_func_idx =
                builder.lift_func(Some(&kebab_name), core_func_idx, type_idx, opts);

            // Export it from the component
            builder.export(&kebab_name, ComponentExportKind::Func, comp_func_idx, None);
        }

        Ok(builder.finish())
    }

    /// Generate WIT interface definition from the codegen's import/export log.
    pub fn generate_wit(&self) -> String {
        let mut wit = String::new();
        wit.push_str("package iec:program;\n\n");

        // Group imports by module
        let mut import_modules: std::collections::BTreeMap<String, Vec<&(String, String, Vec<ComponentType>, Option<ComponentType>)>> =
            Default::default();
        for entry in &self.core_import_log {
            import_modules
                .entry(entry.0.clone())
                .or_default()
                .push(entry);
        }

        // Generate an interface per import module
        for (module, funcs) in &import_modules {
            wit.push_str(&format!("interface {} {{\n", module));
            for (_, name, params, result) in funcs.iter() {
                let kebab_name = to_kebab_case(name);
                let param_list: Vec<String> = params
                    .iter()
                    .enumerate()
                    .map(|(i, ct)| format!("p{}: {}", i, component_type_to_wit(*ct)))
                    .collect();
                match result {
                    Some(ret) => {
                        wit.push_str(&format!(
                            "    {}: func({}) -> {};\n",
                            kebab_name,
                            param_list.join(", "),
                            component_type_to_wit(*ret),
                        ));
                    }
                    None => {
                        wit.push_str(&format!(
                            "    {}: func({});\n",
                            kebab_name,
                            param_list.join(", "),
                        ));
                    }
                }
            }
            wit.push_str("}\n\n");
        }

        // Generate the world with imports and exports
        wit.push_str("world program {\n");
        for module in import_modules.keys() {
            wit.push_str(&format!("    import {};\n", module));
        }
        wit.push('\n');
        for (name, params, result) in &self.core_export_log {
            let kebab_name = to_kebab_case(name);
            let param_list: Vec<String> = params
                .iter()
                .enumerate()
                .map(|(i, ct)| format!("p{}: {}", i, component_type_to_wit(*ct)))
                .collect();
            match result {
                Some(ret) => {
                    wit.push_str(&format!(
                        "    export {}: func({}) -> {};\n",
                        kebab_name,
                        param_list.join(", "),
                        component_type_to_wit(*ret),
                    ));
                }
                None => {
                    wit.push_str(&format!(
                        "    export {}: func({});\n",
                        kebab_name,
                        param_list.join(", "),
                    ));
                }
            }
        }
        wit.push_str("}\n");

        wit
    }

    /// Check if any import or export uses string types.
    fn has_string_types(&self) -> bool {
        let has_in_imports = self.core_import_log.iter().any(|(_, _, params, result)| {
            params.iter().any(|p| matches!(p, ComponentType::String))
                || matches!(result, Some(ComponentType::String))
        });
        let has_in_exports = self.core_export_log.iter().any(|(_, params, result)| {
            params.iter().any(|p| matches!(p, ComponentType::String))
                || matches!(result, Some(ComponentType::String))
        });
        has_in_imports || has_in_exports
    }

    /// Build the core WASM module bytes (internal helper).
    /// If `emit_realloc` is true, appends a `cabi_realloc` function and exports it.
    /// If `import_memory` is true, the module imports memory from "env"/"memory" instead of
    /// defining its own — needed for the component model so canonical lower can provide memory.
    fn build_core_module(&self, emit_realloc: bool, import_memory: bool) -> Result<Vec<u8>, String> {
        let mut module = wasm_encoder::Module::new();

        // Type section: clone and optionally add cabi_realloc type
        let mut type_section = self.type_section.clone();
        let realloc_type_idx = if emit_realloc {
            let idx = self.next_type_idx;
            type_section.ty().function(
                vec![wasm_encoder::ValType::I32; 4], // (old_ptr, old_size, align, new_size)
                vec![wasm_encoder::ValType::I32],    // -> ptr
            );
            Some(idx)
        } else {
            None
        };

        module.section(&type_section);

        // Import section: existing imports + optionally memory import
        let mut import_section = self.import_section.clone();
        if import_memory {
            import_section.import(
                "env",
                "memory",
                wasm_encoder::MemoryType {
                    minimum: 0,
                    maximum: None,
                    memory64: false,
                    shared: false,
                    page_size_log2: None,
                },
            );
        }
        if self.num_imports > 0 || import_memory {
            module.section(&import_section);
        }

        // Function section: clone and optionally add cabi_realloc
        let mut fn_section = self.fn_section.clone();
        let realloc_fn_idx = if let Some(type_idx) = realloc_type_idx {
            let idx = self.next_fn_idx;
            fn_section.function(type_idx);
            Some(idx)
        } else {
            None
        };

        module.section(&fn_section);

        // Calculate total memory: variable layout + string data + heap space
        let string_base = Self::STRING_DATA_BASE;
        let string_end = string_base + self.string_data_offset.get();
        let layout_end = self.memory_layout.total_size();
        let heap_start = std::cmp::max(string_end, layout_end);
        let total_memory = if emit_realloc {
            (heap_start as usize) + 2 * 65536
        } else {
            std::cmp::max(heap_start as usize, 65536)
        };
        let memory_min_pages = total_memory.div_ceil(65536);

        if !import_memory {
            // Define memory locally
            let mut memory_section = wasm_encoder::MemorySection::new();
            memory_section.memory(wasm_encoder::MemoryType {
                minimum: memory_min_pages as u64,
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            });
            module.section(&memory_section);
        }

        // Global section: clone and add heap pointer global if realloc is needed
        let mut global_section = self.global_section.clone();
        let heap_ptr_global = if emit_realloc {
            let idx = global_section.len();
            let heap_start_aligned = (heap_start + 15) & !15;
            global_section.global(
                wasm_encoder::GlobalType {
                    val_type: wasm_encoder::ValType::I32,
                    mutable: true,
                    shared: false,
                },
                &wasm_encoder::ConstExpr::i32_const(heap_start_aligned as i32),
            );
            Some(idx)
        } else {
            None
        };
        module.section(&global_section);

        let mut export_section = self.export_section.clone();
        export_section.export("memory", wasm_encoder::ExportKind::Memory, 0);
        if let Some(fn_idx) = realloc_fn_idx {
            export_section.export("cabi_realloc", wasm_encoder::ExportKind::Func, fn_idx);
        }
        module.section(&export_section);

        // DataCount section (required before Code if Data section will be emitted)
        let string_data = self.string_data.borrow();
        if !string_data.is_empty() {
            let data_count_section = wasm_encoder::DataCountSection { count: 1 };
            module.section(&data_count_section);
        }
        drop(string_data);

        // Code section: clone and add cabi_realloc body
        let mut code_section = self.code_section.clone();
        if let Some(heap_global) = heap_ptr_global {
            // cabi_realloc: simple bump allocator
            // params: old_ptr(0), old_size(1), align(2), new_size(3)
            // result: new_ptr
            use wasm_encoder::Instruction;
            let mut realloc_func = wasm_encoder::Function::new([(1, wasm_encoder::ValType::I32)]); // 1 local for result
            let result_local = 4u32; // after 4 params

            // result = heap_ptr
            realloc_func.instruction(&Instruction::GlobalGet(heap_global));
            realloc_func.instruction(&Instruction::LocalSet(result_local));

            // heap_ptr += new_size
            realloc_func.instruction(&Instruction::GlobalGet(heap_global));
            realloc_func.instruction(&Instruction::LocalGet(3)); // new_size
            realloc_func.instruction(&Instruction::I32Add);
            realloc_func.instruction(&Instruction::GlobalSet(heap_global));

            // return result
            realloc_func.instruction(&Instruction::LocalGet(result_local));
            realloc_func.instruction(&Instruction::End);

            code_section.function(&realloc_func);
        }
        module.section(&code_section);

        // Data section for string literals
        let string_data = self.string_data.borrow();
        if !string_data.is_empty() {
            let mut data_section = wasm_encoder::DataSection::new();
            let mut all_bytes = Vec::with_capacity(self.string_data_offset.get() as usize);
            for (_offset, bytes) in string_data.iter() {
                all_bytes.extend_from_slice(bytes);
            }
            data_section.active(
                0,
                &wasm_encoder::ConstExpr::i32_const(string_base as i32),
                all_bytes,
            );
            module.section(&data_section);
        }

        Ok(module.finish())
    }
}

// ── Component Model helpers ─────────────────────────────────────────────

/// Convert a name to kebab-case for component model exports.
/// `__test__test_abs_positive` → `test-abs-positive`
/// `ASSERT` → `assert`
/// `assert_eq_int` → `assert-eq-int`
fn to_kebab_case(name: &str) -> String {
    let name = name.strip_prefix("__test__").unwrap_or(name);
    name.to_lowercase().replace('_', "-").replace('.', "-")
}

/// Convert a ComponentType to a WIT type name.
fn component_type_to_wit(ct: ComponentType) -> &'static str {
    match ct {
        ComponentType::Val(vt) => match vt {
            wasm_encoder::ValType::I32 => "s32",
            wasm_encoder::ValType::I64 => "s64",
            wasm_encoder::ValType::F32 => "f32",
            wasm_encoder::ValType::F64 => "f64",
            _ => "s32",
        },
        ComponentType::String => "string",
    }
}

/// Convert a ComponentType to a Component Model ComponentValType.
fn component_type_to_component(ct: ComponentType) -> wasm_encoder::ComponentValType {
    use wasm_encoder::{ComponentValType, PrimitiveValType};
    match ct {
        ComponentType::Val(vt) => ComponentValType::Primitive(match vt {
            wasm_encoder::ValType::I32 => PrimitiveValType::S32,
            wasm_encoder::ValType::I64 => PrimitiveValType::S64,
            wasm_encoder::ValType::F32 => PrimitiveValType::F32,
            wasm_encoder::ValType::F64 => PrimitiveValType::F64,
            _ => PrimitiveValType::S32,
        }),
        ComponentType::String => ComponentValType::Primitive(PrimitiveValType::String),
    }
}
