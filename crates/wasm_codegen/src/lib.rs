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

    // ANY_* extern functions pending monomorphization
    // Key: function name Ident
    pub(crate) any_extern_functions: FxHashMap<Ident, AnyExternInfo<'db>>,
    // Monomorphized import name → fn_idx (e.g. "ABS.INT" → 5)
    pub(crate) monomorphized_indices: FxHashMap<String, u32>,

    // Tracked core imports: (module, name, param_types, result_type)
    core_import_log: Vec<(String, String, Vec<wasm_encoder::ValType>, Option<wasm_encoder::ValType>)>,
    // Tracked core exports: (name, param_types, result_type)
    core_export_log: Vec<(String, Vec<wasm_encoder::ValType>, Option<wasm_encoder::ValType>)>,

    // Memory layout for arrays, structs, and memory-resident variables
    memory_layout: MemoryLayout,

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
            core_import_log: Vec::new(),
            core_export_log: Vec::new(),
            memory_layout: MemoryLayout::new(),
            config,
            debug_info: DebugInfo::new(),
            debug_enabled_global,
            debug_trap_id_global,
        }
    }

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

        // Log for component model
        self.core_import_log.push((
            module.to_string(),
            name.to_string(),
            param_types.clone(),
            result_types.first().copied(),
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

        // Skip non-extern functions with ANY_* types (can't lower without monomorphization)
        if self.has_any_types(func) {
            return;
        }

        // Skip if already registered (dedup across files/namespaces)
        if self.function_indices.contains_key(&func.name(self.db)) {
            return;
        }

        let scope_id = func.scope_id(self.db);
        let (param_types, result_types) = self.build_func_signature(func);

        // Log for component model (before move)
        let export_result = result_types.first().copied();
        let export_params = param_types.clone();

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
        self.core_export_log.push((export_name, export_params, export_result));

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
            &self.config,
            &mut self.debug_info,
            self.debug_enabled_global,
            self.debug_trap_id_global,
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
                fb,
                &self.config,
                &mut self.debug_info,
                self.debug_enabled_global,
                self.debug_trap_id_global,
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
                class,
                &self.config,
                &mut self.debug_info,
                self.debug_enabled_global,
                self.debug_trap_id_global,
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
            &self.config,
            &mut self.debug_info,
            self.debug_enabled_global,
            self.debug_trap_id_global,
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

            // Log for component model
            self.core_import_log.push((
                module_name.to_string(),
                mono_key.clone(),
                param_types,
                result_types.first().copied(),
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

        let core_module_bytes = self.build_core_module()?;

        let mut builder = ComponentBuilder::default();

        // 1. Embed the core module
        let core_module_idx = builder.core_module_raw(Some("iec-module"), &core_module_bytes);

        // 2. For each core import, define a component function type,
        //    import it, and lower it to a core function
        let mut lowered_funcs: Vec<(String, String, u32)> = Vec::new();

        for (module, name, params, result) in &self.core_import_log {
            // Define component function type
            let param_names: Vec<String> =
                (0..params.len()).map(|i| format!("p{}", i)).collect();
            let (type_idx, enc) = builder.ty(None);
            {
                let mut fenc = enc.function();
                let cparams: Vec<(&str, wasm_encoder::ComponentValType)> = param_names
                    .iter()
                    .zip(params)
                    .map(|(n, vt)| (n.as_str(), val_type_to_component(*vt)))
                    .collect();
                fenc.params(cparams);
                if let Some(ret) = result {
                    fenc.result(Some(val_type_to_component(*ret)));
                } else {
                    fenc.result(None);
                }
            }

            // Import at component level (kebab-case name for component model)
            let import_name = to_kebab_case(&format!("{}-{}", module, name.replace('.', "-")));
            let comp_func_idx = builder.import(&import_name, ComponentTypeRef::Func(type_idx));

            // Lower to core function
            let core_func_idx = builder.lower_func(None, comp_func_idx, vec![]);

            lowered_funcs.push((module.clone(), name.clone(), core_func_idx));
        }

        // 3. Group lowered funcs by import module and create core instances
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

        // 4. Instantiate the core module with the import instances
        let args: Vec<(&str, ModuleArg)> = module_instances
            .iter()
            .map(|(name, idx)| (name.as_str(), ModuleArg::Instance(*idx)))
            .collect();
        let core_instance_idx =
            builder.core_instantiate(Some("iec-instance"), core_module_idx, args);

        // 5. Lift and export each core export
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
                    .map(|(n, vt)| (n.as_str(), val_type_to_component(*vt)))
                    .collect();
                fenc.params(cparams);
                if let Some(ret) = result {
                    fenc.result(Some(val_type_to_component(*ret)));
                } else {
                    fenc.result(None);
                }
            }

            // Lift the core function
            let comp_func_idx =
                builder.lift_func(Some(&kebab_name), core_func_idx, type_idx, vec![]);

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
        let mut import_modules: std::collections::BTreeMap<String, Vec<&(String, String, Vec<wasm_encoder::ValType>, Option<wasm_encoder::ValType>)>> =
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
                    .map(|(i, vt)| format!("p{}: {}", i, valtype_to_wit(*vt)))
                    .collect();
                match result {
                    Some(ret) => {
                        wit.push_str(&format!(
                            "    {}: func({}) -> {};\n",
                            kebab_name,
                            param_list.join(", "),
                            valtype_to_wit(*ret),
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
                .map(|(i, vt)| format!("p{}: {}", i, valtype_to_wit(*vt)))
                .collect();
            match result {
                Some(ret) => {
                    wit.push_str(&format!(
                        "    export {}: func({}) -> {};\n",
                        kebab_name,
                        param_list.join(", "),
                        valtype_to_wit(*ret),
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

    /// Build the core WASM module bytes (internal helper).
    fn build_core_module(&self) -> Result<Vec<u8>, String> {
        let mut module = wasm_encoder::Module::new();
        module.section(&self.type_section);
        if self.num_imports > 0 {
            module.section(&self.import_section);
        }
        module.section(&self.fn_section);

        let memory_size = self.memory_layout.total_size();
        let memory_min_pages = if memory_size > 0 {
            memory_size.div_ceil(65536)
        } else {
            1
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

        let mut export_section = self.export_section.clone();
        export_section.export("memory", wasm_encoder::ExportKind::Memory, 0);
        module.section(&export_section);

        module.section(&self.code_section);

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

/// Convert a core WASM ValType to a WIT type name.
fn valtype_to_wit(vt: wasm_encoder::ValType) -> &'static str {
    match vt {
        wasm_encoder::ValType::I32 => "s32",
        wasm_encoder::ValType::I64 => "s64",
        wasm_encoder::ValType::F32 => "f32",
        wasm_encoder::ValType::F64 => "f64",
        _ => "s32",
    }
}

/// Convert a core WASM ValType to a Component Model PrimitiveValType.
fn val_type_to_component(vt: wasm_encoder::ValType) -> wasm_encoder::ComponentValType {
    use wasm_encoder::{ComponentValType, PrimitiveValType};
    ComponentValType::Primitive(match vt {
        wasm_encoder::ValType::I32 => PrimitiveValType::S32,
        wasm_encoder::ValType::I64 => PrimitiveValType::S64,
        wasm_encoder::ValType::F32 => PrimitiveValType::F32,
        wasm_encoder::ValType::F64 => PrimitiveValType::F64,
        _ => PrimitiveValType::S32,
    })
}
