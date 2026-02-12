//! Per-function code generation.

use db::WorkspaceDataBase;
use hir::{
    hir_def::{
        expressions::{spec::ElementarySpec, statement::Stmt},
        interned::identifier::Ident,
        pous::{class::Class, function_block::FunctionBlock, pou::Pou, variable::VariableKind},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body::infer_body, head::signature::infer_signature, infer::Infer},
};
use rustc_hash::FxHashMap;
use wasm_encoder::{Instruction, ValType};

use crate::{body::{BodyCodegen, LocalInfo}, wasm_repr::elementary_to_val_type};

/// Container for either a FunctionBlock or Class (both have instance variables and methods).
#[derive(Debug, Clone, Copy)]
pub enum InstanceType<'db> {
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
}

impl<'db> InstanceType<'db> {
    /// Get the list of instance variables (member fields).
    pub fn variables(self, db: &'db dyn WorkspaceDataBase) -> &'db [hir::hir_def::pous::variable::VariableDecl<'db>] {
        match self {
            InstanceType::FunctionBlock(fb) => fb.variables(db),
            InstanceType::Class(class) => class.variables(db),
        }
    }
} 

/// Per-function code generation context.
pub struct FunctionCodegen<'db, 'a> {
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,

    /// Maps variable name to WASM local index.
    local_map: FxHashMap<Ident, LocalInfo>,

    /// The local index for the return value (if function has return type).
    return_local: Option<u32>,

    /// Maps function name to WASM function index (for call resolution).
    function_indices: &'a FxHashMap<Ident, u32>,

    /// For methods: the 'this' pointer local index (always 0 for methods).
    this_local: Option<u32>,

    /// For methods: the instance type (FunctionBlock or Class) for instance variable access.
    instance: Option<InstanceType<'db>>,
}

impl<'db, 'a> FunctionCodegen<'db, 'a> {
    pub fn new(
        db: &'db dyn WorkspaceDataBase,
        scope: ScopeId<'db>,
        function_indices: &'a FxHashMap<Ident, u32>,
    ) -> Self {
        Self {
            db,
            scope,
            local_map: FxHashMap::default(),
            return_local: None,
            function_indices,
            this_local: None,
            instance: None
        }
    }

    pub fn new_with_fb(
        db: &'db dyn WorkspaceDataBase,
        scope: ScopeId<'db>,
        function_indices: &'a FxHashMap<Ident, u32>,
        fb: FunctionBlock<'db>,
    ) -> Self {
        Self {
            db,
            scope,
            local_map: FxHashMap::default(),
            return_local: None,
            function_indices,
            this_local: Some(0), // 'this' is always the first parameter (index 0)
            instance: Some(InstanceType::FunctionBlock(fb))
        }
    }

    pub fn new_with_class(
        db: &'db dyn WorkspaceDataBase,
        scope: ScopeId<'db>,
        function_indices: &'a FxHashMap<Ident, u32>,
        class: Class<'db>,
    ) -> Self {
        Self {
            db,
            scope,
            local_map: FxHashMap::default(),
            return_local: None,
            function_indices,
            this_local: Some(0), // 'this' is always the first parameter (index 0)
            instance: Some(InstanceType::Class(class))
        }
    }

    /// Generate the WASM function body.
    pub fn generate(
        mut self,
        memory_layout: &mut crate::memory::MemoryLayout,
    ) -> Result<wasm_encoder::Function, String> {
        // Build local variable map and collect non-parameter locals
        let extra_locals = self.build_local_map(memory_layout)?;

        // Create function with extra locals
        let mut func = wasm_encoder::Function::new(extra_locals);

        // Generate function body instructions
        self.generate_body(&mut func)?;

        // Push return value if function has return type
        if let Some(ret_idx) = self.return_local {
            func.instruction(&Instruction::LocalGet(ret_idx));
        }

        func.instruction(&Instruction::End);
        Ok(func)
    }

    /// Build the local variable map and return extra locals (non-parameters).
    fn build_local_map(
        &mut self,
        memory_layout: &mut crate::memory::MemoryLayout,
    ) -> Result<Vec<(u32, ValType)>, String> {
        let def_map = self.scope.def_map(self.db);
        let _signature = infer_signature(self.db, self.scope);

        // For methods, local index 0 is the 'this' pointer
        let mut next_local_idx = if self.this_local.is_some() { 1 } else { 0 };
        let mut extra_locals = Vec::new();

        // 1. Map parameters (ONLY Input and InOut) - these become WASM function parameters
        for (name, var) in &def_map.local_variables {
            match var.kind(self.db) {
                VariableKind::Input => {
                    // VAR_INPUT parameters are passed by value
                    let spec = self.extract_elementary_spec(var.spec(self.db).infer(self.db))?;
                    let val_type = elementary_to_val_type(spec)
                        .map_err(|e| format!("Failed to convert parameter type: {}", e))?;

                    self.local_map.insert(
                        *name,
                        LocalInfo::Scalar {
                            index: next_local_idx,
                            val_type,
                            spec,
                        },
                    );
                    next_local_idx += 1;
                }
                VariableKind::InOut => {
                    // VAR_IN_OUT parameters are passed as i32 pointers
                    let var_type = var.spec(self.db).infer(self.db);
                    let pointee_repr = crate::wasm_repr::WasmRepr::from_type(self.db, var_type)
                        .map_err(|e| format!("Failed to get type representation for VAR_IN_OUT parameter: {}", e))?;

                    self.local_map.insert(
                        *name,
                        LocalInfo::Pointer {
                            index: next_local_idx,
                            pointee_repr,
                        },
                    );
                    next_local_idx += 1;
                }
                _ => {
                    // VAR, VAR_TEMP, VAR_OUTPUT, etc. will be handled in step 3
                }
            }
        }

        // 2. Return value local (if scope has return type)
        // Get return type and scope name based on scope kind
        let (return_spec_opt, scope_name_opt) = match get_scope(self.db, self.scope).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => (f.return_type(self.db), Some(f.name(self.db))),
                Pou::FunctionBlock(_fb) => (None, None), // FBs don't have return values
                _ => (None, None),
            },
            ScopeKind::MethodDecl(m) => (m.return_type(self.db), Some(m.name(self.db))),
            ScopeKind::Program(p) => (None, Some(p.name(self.db))),
            _ => (None, None),
        };

        if let (Some(return_spec), Some(scope_name)) = (return_spec_opt, scope_name_opt) {
            let return_type = return_spec.infer(self.db);
            let spec = self.extract_elementary_spec(return_type)?;
            let val_type = elementary_to_val_type(spec)
                .map_err(|e| format!("Failed to convert return type: {}", e))?;

            self.return_local = Some(next_local_idx);

            // Map scope name to return local
            self.local_map.insert(
                scope_name,
                LocalInfo::Scalar {
                    index: next_local_idx,
                    val_type,
                    spec,
                },
            );

            extra_locals.push((1, val_type));
            next_local_idx += 1;
        }

        // 3. Local variables (Var, Temp, Output) from both local_variables and global_variables
        // Process local_variables first (non-parameter vars like VAR, VAR_TEMP)
        for (name, var) in &def_map.local_variables {
            // Skip parameters (already handled in step 1)
            if matches!(var.kind(self.db), VariableKind::Input | VariableKind::InOut) {
                continue;
            }

            match var.kind(self.db) {
                VariableKind::Var | VariableKind::Temp | VariableKind::Output => {
                    let var_type = var.spec(self.db).infer(self.db);
                    let repr = crate::wasm_repr::WasmRepr::from_type(self.db, var_type)
                        .map_err(|e| format!("Failed to get type representation: {}", e))?;

                    match repr {
                        crate::wasm_repr::WasmRepr::Scalar(val_type) => {
                            // Scalar variable - allocate as WASM local
                            let spec = self.extract_elementary_spec(var_type)?;

                            self.local_map.insert(
                                *name,
                                LocalInfo::Scalar {
                                    index: next_local_idx,
                                    val_type,
                                    spec,
                                },
                            );

                            extra_locals.push((1, val_type));
                            next_local_idx += 1;
                        }
                        crate::wasm_repr::WasmRepr::Memory { size, align } => {
                            // Memory-resident variable (array/struct) - allocate in linear memory
                            let address = memory_layout.allocate(*name, size, align);

                            self.local_map.insert(
                                *name,
                                LocalInfo::Memory {
                                    address,
                                    size,
                                    align,
                                },
                            );
                        }
                    }
                }
                _ => {}
            }
        }

        // Also process global_variables (excluding those already in local_variables)
        for (name, var) in &def_map.global_variables {
            // Skip if already processed
            if def_map.local_variables.contains_key(name) {
                continue;
            }

            match var.kind(self.db) {
                VariableKind::Var | VariableKind::Temp | VariableKind::Output => {
                    let var_type = var.spec(self.db).infer(self.db);
                    let repr = crate::wasm_repr::WasmRepr::from_type(self.db, var_type)
                        .map_err(|e| format!("Failed to get type representation: {}", e))?;

                    match repr {
                        crate::wasm_repr::WasmRepr::Scalar(val_type) => {
                            // Scalar variable - allocate as WASM local
                            let spec = self.extract_elementary_spec(var_type)?;

                            self.local_map.insert(
                                *name,
                                LocalInfo::Scalar {
                                    index: next_local_idx,
                                    val_type,
                                    spec,
                                },
                            );

                            extra_locals.push((1, val_type));
                            next_local_idx += 1;
                        }
                        crate::wasm_repr::WasmRepr::Memory { size, align } => {
                            // Memory-resident variable (array/struct) - allocate in linear memory
                            let address = memory_layout.allocate(*name, size, align);

                            self.local_map.insert(
                                *name,
                                LocalInfo::Memory {
                                    address,
                                    size,
                                    align,
                                },
                            );
                        }
                    }
                }
                _ => {
                    // External/Global not yet supported
                }
            }
        }

        Ok(extra_locals)
    }

    /// Generate the function body statements.
    fn generate_body(&mut self, func: &mut wasm_encoder::Function) -> Result<(), String> {
        let _body_result = infer_body(self.db, self.scope);

        // Get statements based on scope kind (similar to infer_body pattern)
        let statements: &[Stmt<'db>] = match get_scope(self.db, self.scope).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => f.statements(self.db),
                Pou::FunctionBlock(fb) => fb.statements(self.db),
                _ => return Ok(()), // Other POUs don't have statements yet
            },
            ScopeKind::MethodDecl(m) => m.stmts(self.db),
            ScopeKind::Program(program) => program.statements(self.db),
            _ => return Ok(()), // Other scopes don't have statements
        };

        // Create body codegen and emit statements
        let mut body_codegen = if let (Some(this_local), Some(instance)) = (self.this_local, self.instance) {
            // Method with 'this' pointer and instance context (FB or Class)
            BodyCodegen::new_with_this(
                self.db,
                &self.local_map,
                self.return_local,
                self.function_indices,
                this_local,
                instance,
            )
        } else {
            // Regular function
            BodyCodegen::new(
                self.db,
                &self.local_map,
                self.return_local,
                self.function_indices,
            )
        };
        body_codegen.emit_statements(func, statements)?;

        Ok(())
    }


    /// Extract elementary spec from a type (unwrap Type::Elementary).
    fn extract_elementary_spec(
        &self,
        ty: hir::hir_ty::ty::Type<'db>,
    ) -> Result<ElementarySpec, String> {
        let normalized = ty.normalize(self.db);

        match normalized {
            hir::hir_ty::ty::Type::Elementary(spec) => Ok(spec),
            _ => Err(format!("Expected elementary type, got: {:?}", normalized)),
        }
    }
}
