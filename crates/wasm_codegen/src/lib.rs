//! WebAssembly code generation for IEC 61131-3 programs.

use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        interned::identifier::Ident,
        pous::{function::Function, pou::Pou},
        program::ProgramDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{head::signature::infer_signature, infer::Infer},
};
use rustc_hash::FxHashMap;

pub mod wasm_repr;
pub mod func_codegen;
pub mod body;
pub mod cast;
pub mod memory;

#[cfg(feature = "debug_info")]
pub mod debug_info;

#[cfg(test)]
pub mod tests;

use func_codegen::FunctionCodegen;
use memory::MemoryLayout;
use wasm_repr::WasmRepr;

pub struct ModuleCodeGen<'db> {
    db: &'db dyn WorkspaceDataBase,
    type_section: wasm_encoder::TypeSection,
    fn_section: wasm_encoder::FunctionSection,
    export_section: wasm_encoder::ExportSection,
    code_section: wasm_encoder::CodeSection,

    // Index tracking
    next_type_idx: u32,
    next_fn_idx: u32,

    // Scope memoization
    scopes: FxHashMap<ScopeId<'db>, ScopeCodegenInfo>,

    // Function name → index mapping for call resolution
    function_indices: FxHashMap<hir::hir_def::interned::identifier::Ident, u32>,

    // Memory layout for arrays, structs, and memory-resident variables
    memory_layout: MemoryLayout,

    // Debug information collector
    #[cfg(feature = "debug_info")]
    debug_collector: debug_info::DebugInfoCollector,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct ScopeCodegenInfo {
    type_idx: u32,
    fn_idx: u32,
}

impl<'db> ModuleCodeGen<'db> {
    pub fn new(db: &'db dyn WorkspaceDataBase) -> Self {
        Self {
            db,
            type_section: Default::default(),
            fn_section: Default::default(),
            export_section: Default::default(),
            code_section: Default::default(),
            next_type_idx: 0,
            next_fn_idx: 0,
            scopes: Default::default(),
            function_indices: Default::default(),
            memory_layout: MemoryLayout::new(),
            #[cfg(feature = "debug_info")]
            debug_collector: debug_info::DebugInfoCollector::new(),
        }
    }

    /// Generate a WebAssembly module from the given program declaration.
    pub fn generate_from_program(&mut self, program: ProgramDecl<'db>) -> wasm_encoder::Module {
        // Generate code for the program scope
        self.generate_scope(program.scope_id(self.db));

        // Build the module
        let mut module = wasm_encoder::Module::new();
        module.section(&self.type_section);
        module.section(&self.fn_section);

        // Add memory section if we allocated any memory
        if self.memory_layout.total_size() > 0 {
            let memory_min_pages = (self.memory_layout.total_size() + 65535) / 65536; // Round up to pages
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

        module.section(&self.export_section);
        module.section(&self.code_section);

        // Generate and embed DWARF debug information
        #[cfg(feature = "debug_info")]
        {
            if let Ok(custom_sections) = self.generate_debug_sections() {
                for (name, data) in custom_sections {
                    let custom_section = wasm_encoder::CustomSection {
                        name: std::borrow::Cow::Borrowed(&name),
                        data: std::borrow::Cow::Borrowed(&data),
                    };
                    module.section(&custom_section);
                }
            }
        }

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

    /// Generate code for a function.
    fn generate_function(&mut self, func: Function<'db>) {
        let scope_id = func.scope_id(self.db);

        // Get function signature from type inference
        let _signature = infer_signature(self.db, scope_id);
        let def_map = scope_id.def_map(self.db);

        // Build parameter types (Input and InOut variables, in declaration order)
        let mut param_types = Vec::new();
        for (_name, var) in &def_map.local_variables {
            use hir::hir_def::pous::variable::VariableKind;

            // Only VAR_INPUT and VAR_IN_OUT become function parameters
            match var.kind(self.db) {
                VariableKind::Input => {
                    // VAR_INPUT parameters are passed by value
                    if let Ok(repr) = WasmRepr::from_type(self.db, var.spec(self.db).infer(self.db)) {
                        param_types.extend(repr.flatten());
                    }
                }
                VariableKind::InOut => {
                    // VAR_IN_OUT parameters are passed as i32 pointers
                    param_types.push(wasm_encoder::ValType::I32);
                }
                _ => {
                    // VAR, VAR_TEMP, VAR_OUTPUT, etc. are NOT function parameters
                }
            }
        }

        // Build return type
        let mut result_types = Vec::new();
        if let Some(return_spec) = func.return_type(self.db) {
            if let Ok(repr) = WasmRepr::from_type(self.db, return_spec.infer(self.db)) {
                result_types.extend(repr.flatten());
            }
        }

        // Add function type signature
        let type_idx = self.next_type_idx;
        self.type_section
            .ty()
            .function(param_types, result_types);
        self.next_type_idx += 1;

        // Declare the function
        let fn_idx = self.next_fn_idx;
        self.fn_section.function(type_idx);
        self.next_fn_idx += 1;

        // Export the function with its name
        let func_name = func.name(self.db).text(self.db);
        self.export_section.export(
            func_name.as_str(),
            wasm_encoder::ExportKind::Func,
            fn_idx,
        );

        // Store function name → index mapping
        self.function_indices.insert(func.name(self.db), fn_idx);

        // Store codegen info for memoization
        self.scopes.insert(
            scope_id,
            ScopeCodegenInfo { type_idx, fn_idx },
        );

        // Collect debug information
        #[cfg(feature = "debug_info")]
        self.collect_function_debug_info(func, scope_id, fn_idx);

        // Generate function body
        let codegen = FunctionCodegen::new(self.db, scope_id, &self.function_indices);
        let (func_body, line_mappings) = codegen
            .generate(&mut self.memory_layout)
            .expect("Failed to generate function body");
        self.code_section.function(&func_body);

        // Phase 6: Update debug info with line mappings
        #[cfg(feature = "debug_info")]
        self.debug_collector.update_function_line_mappings(fn_idx, line_mappings);
    }

    /// Generate code for a function block and its methods.
    pub fn generate_function_block(&mut self, fb: hir::hir_def::pous::function_block::FunctionBlock<'db>) {
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
                        if let Ok(repr) = WasmRepr::from_type(self.db, var.spec(self.db).infer(self.db)) {
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
            if let Some(return_spec) = method.return_type(self.db) {
                if let Ok(repr) = WasmRepr::from_type(self.db, return_spec.infer(self.db)) {
                    result_types.extend(repr.flatten());
                }
            }

            // Add method type signature
            let type_idx = self.next_type_idx;
            self.type_section
                .ty()
                .function(param_types, result_types);
            self.next_type_idx += 1;

            // Declare the method as a WASM function
            let fn_idx = self.next_fn_idx;
            self.fn_section.function(type_idx);
            self.next_fn_idx += 1;

            // Export the method with qualified name: FB_Name$Method_Name
            let fb_name = fb.name(self.db).text(self.db);
            let method_name = method.name(self.db).text(self.db);
            let qualified_name = format!("{}${}", fb_name, method_name);
            self.export_section.export(
                &qualified_name,
                wasm_encoder::ExportKind::Func,
                fn_idx,
            );

            // Store qualified method name → index mapping
            let qualified_ident = Ident::from_slice(self.db, &qualified_name);
            self.function_indices.insert(qualified_ident, fn_idx);

            // Store codegen info for memoization
            self.scopes.insert(
                scope_id,
                ScopeCodegenInfo { type_idx, fn_idx },
            );

            // Generate method body (with implicit 'this' parameter handling)
            let codegen = FunctionCodegen::new_with_fb(
                self.db,
                scope_id,
                &self.function_indices,
                fb, // Pass FB for instance variable access
            );
            let (func_body, line_mappings) = codegen
                .generate(&mut self.memory_layout)
                .expect("Failed to generate method body");
            self.code_section.function(&func_body);

            // Collect debug information for the method
            #[cfg(feature = "debug_info")]
            self.collect_method_debug_info(*method, scope_id, &qualified_name, fn_idx);

            // Phase 6: Update debug info with line mappings
            #[cfg(feature = "debug_info")]
            self.debug_collector.update_function_line_mappings(fn_idx, line_mappings);
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
                        if let Ok(repr) = WasmRepr::from_type(self.db, var.spec(self.db).infer(self.db)) {
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
            if let Some(return_spec) = method.return_type(self.db) {
                if let Ok(repr) = WasmRepr::from_type(self.db, return_spec.infer(self.db)) {
                    result_types.extend(repr.flatten());
                }
            }

            // Add method type signature
            let type_idx = self.next_type_idx;
            self.type_section
                .ty()
                .function(param_types, result_types);
            self.next_type_idx += 1;

            // Declare the method as a WASM function
            let fn_idx = self.next_fn_idx;
            self.fn_section.function(type_idx);
            self.next_fn_idx += 1;

            // Export the method with qualified name: ClassName$MethodName
            let class_name = class.name(self.db).text(self.db);
            let method_name = method.name(self.db).text(self.db);
            let qualified_name = format!("{}${}", class_name, method_name);
            self.export_section.export(
                &qualified_name,
                wasm_encoder::ExportKind::Func,
                fn_idx,
            );

            // Store qualified method name → index mapping
            let qualified_ident = Ident::from_slice(self.db, &qualified_name);
            self.function_indices.insert(qualified_ident, fn_idx);

            // Store codegen info for memoization
            self.scopes.insert(
                scope_id,
                ScopeCodegenInfo { type_idx, fn_idx },
            );

            // Generate method body (with implicit 'this' parameter handling)
            // We pass the class as a FunctionBlock-like entity for instance variable access
            // TODO: Handle inheritance by traversing parent class fields
            let codegen = FunctionCodegen::new_with_class(
                self.db,
                scope_id,
                &self.function_indices,
                class,
            );
            let (func_body, line_mappings) = codegen
                .generate(&mut self.memory_layout)
                .expect("Failed to generate method body");
            self.code_section.function(&func_body);

            // Collect debug information for the method
            #[cfg(feature = "debug_info")]
            self.collect_method_debug_info(*method, scope_id, &qualified_name, fn_idx);

            // Phase 6: Update debug info with line mappings
            #[cfg(feature = "debug_info")]
            self.debug_collector.update_function_line_mappings(fn_idx, line_mappings);
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

        // Export the program with its name
        let program_name = program.name(self.db).text(self.db);
        self.export_section.export(
            program_name,
            wasm_encoder::ExportKind::Func,
            fn_idx,
        );

        // Store program name → index mapping
        self.function_indices.insert(program.name(self.db), fn_idx);

        // Store codegen info for memoization
        self.scopes.insert(
            scope_id,
            ScopeCodegenInfo { type_idx, fn_idx },
        );

        // Generate program body
        // Note: PROGRAM variables are allocated in linear memory during build_local_map
        let codegen = FunctionCodegen::new(
            self.db,
            scope_id,
            &self.function_indices,
        );
        let (func_body, _line_mappings) = codegen
            .generate(&mut self.memory_layout)
            .expect("Failed to generate program body");
        self.code_section.function(&func_body);

        // Phase 6: Programs don't collect debug info currently (no fn_idx to reference)
        // TODO: Add program debug collection if needed
    }

    /// Generate DWARF debug sections.
    ///
    /// Collects all debug information and converts it to DWARF sections
    /// that can be embedded as WASM custom sections.
    #[cfg(feature = "debug_info")]
    pub fn generate_debug_sections(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        // Phase 3: Basic integration
        // Get collected debug information (currently empty, will be populated in Phase 4+)
        let compilation_units = self.debug_collector.compilation_units().to_vec();

        // Generate DWARF sections
        let mut generator = debug_info::DwarfGenerator::new();
        generator.generate(compilation_units)?;
        let sections = generator.finish()?;

        // Convert DWARF sections to WASM custom sections
        let custom_sections = debug_info::dwarf_gen::sections_to_wasm_custom(sections);

        Ok(custom_sections)
    }

    /// Convert HIR type to TypeDebugInfo for debug symbols.
    ///
    /// Phase 5: Convert elementary types and return Void for others.
    /// Later phases will add struct, array, pointer type support.
    #[cfg(feature = "debug_info")]
    fn hir_type_to_debug_type(&self, ty: hir::hir_ty::ty::Type<'db>) -> debug_info::collector::TypeDebugInfo {
        use debug_info::collector::TypeDebugInfo;
        use debug_info::types::{elementary_to_dwarf, elementary_type_name};
        use hir::hir_ty::ty::Type;

        match ty {
            Type::Elementary(spec) => {
                let (encoding, byte_size) = elementary_to_dwarf(spec);
                TypeDebugInfo::Elementary {
                    name: elementary_type_name(spec).to_string(),
                    byte_size,
                    encoding,
                }
            }
            // Phase 5: Only elementary types for now
            // Later: Add Struct, Array, Pointer support
            _ => TypeDebugInfo::Void,
        }
    }

    /// Collect variable debug info from a scope's def_map.
    ///
    /// Phase 5: Extract parameters and locals with types.
    /// Variable locations are not yet tracked - will be added when
    /// we integrate with actual local_map from FunctionCodegen.
    #[cfg(feature = "debug_info")]
    fn collect_variables_from_def_map(
        &self,
        scope_id: ScopeId<'db>,
    ) -> (Vec<debug_info::VariableDebugInfo>, Vec<debug_info::VariableDebugInfo>) {
        use debug_info::VariableDebugInfo;
        use debug_info::collector::VariableLocation;
        use hir::hir_def::pous::variable::VariableKind;
        use hir::HirNodeInfo;

        let def_map = scope_id.def_map(self.db);
        let mut parameters = Vec::new();
        let mut locals = Vec::new();

        // Collect variables from both local_variables and global_variables
        // local_variables contains: Input, Output, InOut parameters
        // global_variables contains: Var, Temp, Output locals
        for (name, var) in &def_map.local_variables {
            let var_type = var.spec(self.db).infer(self.db);
            let type_debug_info = self.hir_type_to_debug_type(var_type);
            let decl_span = var.get_span(self.db);
            let var_name = name.text(self.db).to_string();

            // Phase 5: Use placeholder location (Local(0))
            // Later: Get actual locations from local_map
            let location = VariableLocation::Local(0);

            let var_debug_info = VariableDebugInfo {
                name: var_name,
                var_type: type_debug_info,
                location,
                decl_span,
            };

            match var.kind(self.db) {
                VariableKind::Input | VariableKind::InOut => {
                    parameters.push(var_debug_info);
                }
                VariableKind::Var | VariableKind::Temp | VariableKind::Output => {
                    locals.push(var_debug_info);
                }
                _ => {}
            }
        }

        // Also check global_variables for VAR, TEMP, OUTPUT locals
        for (name, var) in &def_map.global_variables {
            // Skip if already in local_variables
            if def_map.local_variables.contains_key(name) {
                continue;
            }

            let var_type = var.spec(self.db).infer(self.db);
            let type_debug_info = self.hir_type_to_debug_type(var_type);
            let decl_span = var.get_span(self.db);
            let var_name = name.text(self.db).to_string();

            let location = VariableLocation::Local(0);

            let var_debug_info = VariableDebugInfo {
                name: var_name,
                var_type: type_debug_info,
                location,
                decl_span,
            };

            match var.kind(self.db) {
                VariableKind::Var | VariableKind::Temp | VariableKind::Output => {
                    locals.push(var_debug_info);
                }
                _ => {}
            }
        }

        (parameters, locals)
    }

    /// Collect debug information for a function.
    ///
    /// Extracts function metadata including name, WASM index, parameters, locals,
    /// and return type, then adds it to the debug info collector.
    #[cfg(feature = "debug_info")]
    fn collect_function_debug_info(
        &mut self,
        func: Function<'db>,
        scope_id: ScopeId<'db>,
        fn_idx: u32,
    ) {
        use debug_info::FunctionDebugInfo;
        use hir::HirNodeInfo;

        // Get the file this function is defined in
        let file = scope_id.file(self.db);
        let file_path = file.url(self.db).path().to_string();

        // Register the file as a compilation unit
        self.debug_collector.register_file(file, file_path);

        // Get function span
        let decl_span = func.get_span(self.db);

        // Get function name
        let name = func.name(self.db).text(self.db).to_string();

        // Get return type
        let return_type = func.return_type(self.db).map(|spec| {
            self.hir_type_to_debug_type(spec.infer(self.db))
        });

        // Phase 5: Collect parameters and locals from def_map
        let (parameters, locals) = self.collect_variables_from_def_map(scope_id);

        // Phase 6 will add line mappings
        let func_info = FunctionDebugInfo {
            name,
            wasm_index: fn_idx,
            decl_span,
            parameters,
            locals,
            return_type,
            line_mappings: Vec::new(), // Phase 6
        };

        self.debug_collector.add_function(file, func_info);
    }

    /// Collect debug information for a method (function block or class method).
    ///
    /// Phase 4: Basic method metadata (name, WASM index, span)
    /// Phase 5: Will add parameters, locals, and return type
    /// Phase 6: Will add line mappings
    #[cfg(feature = "debug_info")]
    fn collect_method_debug_info(
        &mut self,
        method: hir::hir_def::pous::class::MethodDecl<'db>,
        scope_id: ScopeId<'db>,
        qualified_name: &str,
        fn_idx: u32,
    ) {
        use debug_info::FunctionDebugInfo;
        use hir::HirNodeInfo;

        // Get the file this method is defined in
        let file = scope_id.file(self.db);
        let file_path = file.url(self.db).path().to_string();

        // Register the file as a compilation unit
        self.debug_collector.register_file(file, file_path);

        // Get method span from the method declaration
        let decl_span = method.get_span(self.db);

        // Get return type
        let return_type = method.return_type(self.db).map(|spec| {
            self.hir_type_to_debug_type(spec.infer(self.db))
        });

        // Phase 5: Collect parameters and locals from def_map
        let (parameters, locals) = self.collect_variables_from_def_map(scope_id);

        // Phase 6 will add line mappings
        let func_info = FunctionDebugInfo {
            name: qualified_name.to_string(),
            wasm_index: fn_idx,
            decl_span,
            parameters,
            locals,
            return_type,
            line_mappings: Vec::new(), // Phase 6
        };

        self.debug_collector.add_function(file, func_info);
    }

    /// Build the final WASM module with all generated code and debug information.
    ///
    /// This method consumes the codegen and returns the complete WASM binary as bytes.
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        // Build the module
        let mut module = wasm_encoder::Module::new();
        module.section(&self.type_section);
        module.section(&self.fn_section);

        // Add memory section
        let memory_size = self.memory_layout.total_size();
        let memory_min_pages = if memory_size > 0 {
            (memory_size + 65535) / 65536
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

        // Generate debug sections before moving export_section
        #[cfg(feature = "debug_info")]
        let debug_sections = self.generate_debug_sections().ok();

        // Export section
        self.export_section.export("memory", wasm_encoder::ExportKind::Memory, 0);
        module.section(&self.export_section);

        // Code section
        module.section(&self.code_section);

        // Add debug sections if feature is enabled
        #[cfg(feature = "debug_info")]
        {
            if let Some(custom_sections) = debug_sections {
                for (name, data) in custom_sections {
                    let custom_section = wasm_encoder::CustomSection {
                        name: std::borrow::Cow::Owned(name),
                        data: std::borrow::Cow::Owned(data),
                    };
                    module.section(&custom_section);
                }
            }
        }

        Ok(module.finish())
    }
}

