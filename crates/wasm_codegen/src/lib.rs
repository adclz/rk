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

        // Generate function body
        let codegen = FunctionCodegen::new(self.db, scope_id, &self.function_indices);
        let func_body = codegen
            .generate(&mut self.memory_layout)
            .expect("Failed to generate function body");
        self.code_section.function(&func_body);
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
        let func_body = codegen
            .generate(&mut self.memory_layout)
            .expect("Failed to generate program body");
        self.code_section.function(&func_body);
    }
}

