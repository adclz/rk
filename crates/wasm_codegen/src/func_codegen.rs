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

use crate::{
    body::{BodyCodegen, LocalInfo},
    wasm_repr::elementary::elementary_to_val_type,
};

/// Container for either a FunctionBlock or Class (both have instance variables and methods).
#[derive(Debug, Clone, Copy)]
pub enum InstanceType<'db> {
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
}

impl<'db> InstanceType<'db> {
    /// Get the list of instance variables (member fields).
    pub fn variables(
        self,
        db: &'db dyn WorkspaceDataBase,
    ) -> &'db [hir::hir_def::pous::variable::VariableDecl<'db>] {
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

    /// Monomorphized ANY_* extern function indices
    monomorphized_indices: &'a FxHashMap<String, u32>,
    /// ANY_* extern function info
    any_extern_functions: &'a FxHashMap<Ident, crate::AnyExternInfo<'db>>,

    /// For methods: the 'this' pointer local index (always 0 for methods).
    this_local: Option<u32>,

    /// For methods: the instance type (FunctionBlock or Class) for instance variable access.
    instance: Option<InstanceType<'db>>,

    /// Debug configuration
    config: &'a crate::debug::CodeGenConfig,

    /// Debug information
    debug_info: &'a mut crate::debug::DebugInfo,

    /// Debug global indices
    debug_enabled_global: Option<u32>,
    debug_trap_id_global: Option<u32>,
}

impl<'db, 'a> FunctionCodegen<'db, 'a> {
    pub fn new(
        db: &'db dyn WorkspaceDataBase,
        scope: ScopeId<'db>,
        function_indices: &'a FxHashMap<Ident, u32>,
        monomorphized_indices: &'a FxHashMap<String, u32>,
        any_extern_functions: &'a FxHashMap<Ident, crate::AnyExternInfo<'db>>,
        config: &'a crate::debug::CodeGenConfig,
        debug_info: &'a mut crate::debug::DebugInfo,
        debug_enabled_global: Option<u32>,
        debug_trap_id_global: Option<u32>,
    ) -> Self {
        Self {
            db,
            scope,
            local_map: FxHashMap::default(),
            return_local: None,
            function_indices,
            monomorphized_indices,
            any_extern_functions,
            this_local: None,
            instance: None,
            config,
            debug_info,
            debug_enabled_global,
            debug_trap_id_global,
        }
    }

    pub fn new_with_fb(
        db: &'db dyn WorkspaceDataBase,
        scope: ScopeId<'db>,
        function_indices: &'a FxHashMap<Ident, u32>,
        monomorphized_indices: &'a FxHashMap<String, u32>,
        any_extern_functions: &'a FxHashMap<Ident, crate::AnyExternInfo<'db>>,
        fb: FunctionBlock<'db>,
        config: &'a crate::debug::CodeGenConfig,
        debug_info: &'a mut crate::debug::DebugInfo,
        debug_enabled_global: Option<u32>,
        debug_trap_id_global: Option<u32>,
    ) -> Self {
        Self {
            db,
            scope,
            local_map: FxHashMap::default(),
            return_local: None,
            function_indices,
            monomorphized_indices,
            any_extern_functions,
            this_local: Some(0),
            instance: Some(InstanceType::FunctionBlock(fb)),
            config,
            debug_info,
            debug_enabled_global,
            debug_trap_id_global,
        }
    }

    pub fn new_with_class(
        db: &'db dyn WorkspaceDataBase,
        scope: ScopeId<'db>,
        function_indices: &'a FxHashMap<Ident, u32>,
        monomorphized_indices: &'a FxHashMap<String, u32>,
        any_extern_functions: &'a FxHashMap<Ident, crate::AnyExternInfo<'db>>,
        class: Class<'db>,
        config: &'a crate::debug::CodeGenConfig,
        debug_info: &'a mut crate::debug::DebugInfo,
        debug_enabled_global: Option<u32>,
        debug_trap_id_global: Option<u32>,
    ) -> Self {
        Self {
            db,
            scope,
            local_map: FxHashMap::default(),
            return_local: None,
            function_indices,
            monomorphized_indices,
            any_extern_functions,
            this_local: Some(0),
            instance: Some(InstanceType::Class(class)),
            config,
            debug_info,
            debug_enabled_global,
            debug_trap_id_global,
        }
    }

    /// Generate the WASM function body.
    pub fn generate(
        mut self,
        memory_layout: &mut crate::memory::MemoryLayout,
    ) -> Result<wasm_encoder::Function, String> {
        // Detect which variables have their address taken (REF operator)
        let address_taken_vars = self.find_address_taken_variables()?;

        // Build local variable map and collect non-parameter locals
        let extra_locals = self.build_local_map(memory_layout, &address_taken_vars)?;

        // Create function with extra locals
        let mut func = wasm_encoder::Function::new(extra_locals);

        // Initialize memory-allocated variables at function start
        self.initialize_memory_variables(&mut func)?;

        // Generate function body instructions
        self.generate_body(&mut func)?;

        // Push return value if function has return type
        if let Some(ret_idx) = self.return_local {
            func.instruction(&Instruction::LocalGet(ret_idx));
        }

        func.instruction(&Instruction::End);
        Ok(func)
    }

    /// Scan function body to find variables whose addresses are taken with REF().
    fn find_address_taken_variables(&self) -> Result<rustc_hash::FxHashSet<Ident>, String> {
        use hir::hir_def::expressions::{
            expression::{Expr, ExprKind, ParamAssignKind, PrimaryExpr, RefValue},
            statement::{Stmt, StmtKind},
        };
        use rustc_hash::FxHashSet;

        let mut address_taken = FxHashSet::default();

        // Get statements based on scope kind
        let statements: &[Stmt<'db>] = match get_scope(self.db, self.scope).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => f.statements(self.db),
                Pou::FunctionBlock(fb) => fb.statements(self.db),
                _ => return Ok(address_taken),
            },
            ScopeKind::MethodDecl(m) => m.stmts(self.db),
            ScopeKind::Program(program) => program.statements(self.db),
            _ => return Ok(address_taken),
        };

        // Helper to recursively scan expressions for REF(variable)
        fn scan_expr<'db>(
            db: &'db dyn WorkspaceDataBase,
            expr: Expr<'db>,
            address_taken: &mut FxHashSet<Ident>,
        ) {
            match expr.expr(db) {
                ExprKind::PrimaryExpr(PrimaryExpr::RefValue { value }) => {
                    if let RefValue::Address(begin_path) = value {
                        // Extract variable name from BeginPathExpr
                        if let Some(path_expr) = begin_path.expr(db) {
                            let var_name = path_expr.ident(db).ident;
                            address_taken.insert(var_name);
                        }
                    }
                }
                ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(func_call)) => {
                    for arg in func_call.params(db) {
                        match arg.kind(db) {
                            ParamAssignKind::NonFormal { value } => {
                                scan_expr(db, value, address_taken);
                            }
                            ParamAssignKind::FormalInput { value, .. } => {
                                scan_expr(db, value, address_taken);
                            }
                            ParamAssignKind::FormalOutput { .. } => {
                                // FormalOutput has variable access, not expression
                            }
                        }
                    }
                }
                ExprKind::AddOperator { left, right, .. }
                | ExprKind::MultOperator { left, right, .. }
                | ExprKind::ComparisonOperator { left, right, .. }
                | ExprKind::BooleanOperator { left, right, .. }
                | ExprKind::PowerOperator { left, right } => {
                    scan_expr(db, *left, address_taken);
                    scan_expr(db, *right, address_taken);
                }
                ExprKind::UnaryOperator { expr, .. } => {
                    scan_expr(db, *expr, address_taken);
                }
                _ => {}
            }
        }

        // Helper to recursively scan statements
        fn scan_stmt<'db>(
            db: &'db dyn WorkspaceDataBase,
            stmt: Stmt<'db>,
            address_taken: &mut FxHashSet<Ident>,
        ) {
            match stmt.stmt(db) {
                StmtKind::Assignment { target, .. } => {
                    scan_expr(db, *target, address_taken);
                }
                StmtKind::FuncCall(func_call) => {
                    for arg in func_call.params(db) {
                        match arg.kind(db) {
                            ParamAssignKind::NonFormal { value } => {
                                scan_expr(db, value, address_taken);
                            }
                            ParamAssignKind::FormalInput { value, .. } => {
                                scan_expr(db, value, address_taken);
                            }
                            ParamAssignKind::FormalOutput { .. } => {
                                // FormalOutput has variable access, not expression
                            }
                        }
                    }
                }
                StmtKind::If {
                    condition,
                    then,
                    else_if,
                    else_,
                } => {
                    scan_expr(db, *condition, address_taken);
                    if let Some(then_stmts) = then {
                        for s in then_stmts {
                            scan_stmt(db, *s, address_taken);
                        }
                    }
                    for (cond, block) in else_if {
                        scan_expr(db, *cond, address_taken);
                        for s in block {
                            scan_stmt(db, *s, address_taken);
                        }
                    }
                    if let Some(else_stmts) = else_ {
                        for s in else_stmts {
                            scan_stmt(db, *s, address_taken);
                        }
                    }
                }
                StmtKind::Case {
                    condition,
                    cases,
                    else_,
                } => {
                    scan_expr(db, *condition, address_taken);
                    for (_case_values, block) in cases {
                        for s in block {
                            scan_stmt(db, *s, address_taken);
                        }
                    }
                    if let Some(else_stmts) = else_ {
                        for s in else_stmts {
                            scan_stmt(db, *s, address_taken);
                        }
                    }
                }
                StmtKind::For { body, .. }
                | StmtKind::While { body, .. }
                | StmtKind::Repeat { body, .. } => {
                    for s in body {
                        scan_stmt(db, *s, address_taken);
                    }
                }
                _ => {}
            }
        }

        // Scan all statements
        for stmt in statements {
            scan_stmt(self.db, *stmt, &mut address_taken);
        }

        Ok(address_taken)
    }

    /// Build the local variable map and return extra locals (non-parameters).
    fn build_local_map(
        &mut self,
        memory_layout: &mut crate::memory::MemoryLayout,
        address_taken_vars: &rustc_hash::FxHashSet<Ident>,
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
                        .map_err(|e| {
                            format!(
                                "Failed to get type representation for VAR_IN_OUT parameter: {}",
                                e
                            )
                        })?;

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
                            // Check if this variable's address is taken with REF()
                            if address_taken_vars.contains(name) {
                                // Address-taken scalars must be allocated in linear memory
                                let size = match val_type {
                                    ValType::I32 | ValType::F32 => 4,
                                    ValType::I64 | ValType::F64 => 8,
                                    _ => {
                                        return Err(format!(
                                            "Unsupported value type for memory allocation: {:?}",
                                            val_type
                                        ));
                                    }
                                };
                                let align = size; // Natural alignment
                                let address = memory_layout.allocate(*name, size, align);

                                self.local_map.insert(
                                    *name,
                                    LocalInfo::Memory {
                                        address,
                                        size,
                                        align,
                                    },
                                );
                            } else {
                                // Normal scalar - allocate as WASM local
                                // For REF_TO types, we use INT as the elementary spec since pointers are i32
                                use hir::hir_def::expressions::spec::ElementarySpec;
                                let spec = match var_type.normalize(self.db) {
                                    hir::hir_ty::ty::Type::Elementary(s) => s,
                                    hir::hir_ty::ty::Type::RefTo(_) => ElementarySpec::Int, // Pointers are i32
                                    _ => {
                                        return Err(format!(
                                            "Unexpected scalar type that isn't elementary or RefTo: {:?}",
                                            var_type
                                        ));
                                    }
                                };

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
                            // Check if this variable's address is taken with REF()
                            if address_taken_vars.contains(name) {
                                // Address-taken scalars must be allocated in linear memory
                                let size = match val_type {
                                    ValType::I32 | ValType::F32 => 4,
                                    ValType::I64 | ValType::F64 => 8,
                                    _ => {
                                        return Err(format!(
                                            "Unsupported value type for memory allocation: {:?}",
                                            val_type
                                        ));
                                    }
                                };
                                let align = size; // Natural alignment
                                let address = memory_layout.allocate(*name, size, align);

                                self.local_map.insert(
                                    *name,
                                    LocalInfo::Memory {
                                        address,
                                        size,
                                        align,
                                    },
                                );
                            } else {
                                // Normal scalar - allocate as WASM local
                                // For REF_TO types, we use INT as the elementary spec since pointers are i32
                                use hir::hir_def::expressions::spec::ElementarySpec;
                                let spec = match var_type.normalize(self.db) {
                                    hir::hir_ty::ty::Type::Elementary(s) => s,
                                    hir::hir_ty::ty::Type::RefTo(_) => ElementarySpec::Int, // Pointers are i32
                                    _ => {
                                        return Err(format!(
                                            "Unexpected scalar type that isn't elementary or RefTo: {:?}",
                                            var_type
                                        ));
                                    }
                                };

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

    /// Initialize memory-allocated variables with their initial values.
    fn initialize_memory_variables(
        &mut self,
        func: &mut wasm_encoder::Function,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::expression::InitExprKind;

        let def_map = self.scope.def_map(self.db);

        // Iterate through local_map and initialize memory-allocated variables
        for (var_name, local_info) in &self.local_map {
            if let LocalInfo::Memory { address, .. } = local_info {
                // Check if this variable has an initializer - try both local and global
                let var_decl = def_map
                    .local_variables
                    .get(var_name)
                    .or_else(|| def_map.global_variables.get(var_name));

                if let Some(var_decl) = var_decl {
                    // Get the initialization value from variable declaration
                    if let Some(init_expr) = var_decl.init(self.db) {
                        // Extract the constant expression from InitExpr
                        let init_kind = init_expr.kind(self.db);
                        match init_kind {
                            InitExprKind::ConstantExpr(expr) => {
                                // Create a temporary BodyCodegen to emit the initialization expression
                                let body_codegen = if let (Some(this_local), Some(instance)) =
                                    (self.this_local, self.instance)
                                {
                                    BodyCodegen::new_with_this(
                                        self.db,
                                        &self.local_map,
                                        self.return_local,
                                        self.function_indices,
                                        self.monomorphized_indices,
                                        self.any_extern_functions,
                                        this_local,
                                        instance,
                                        self.config,
                                        self.debug_info,
                                        self.debug_enabled_global,
                                        self.debug_trap_id_global,
                                    )
                                } else {
                                    BodyCodegen::new(
                                        self.db,
                                        &self.local_map,
                                        self.return_local,
                                        self.function_indices,
                                        self.monomorphized_indices,
                                        self.any_extern_functions,
                                        self.config,
                                        self.debug_info,
                                        self.debug_enabled_global,
                                        self.debug_trap_id_global,
                                    )
                                };

                                // Store the value to memory
                                // Determine store instruction based on the type
                                let var_type = var_decl.spec(self.db).infer(self.db);
                                let repr = crate::wasm_repr::WasmRepr::from_type(self.db, var_type)
                                    .map_err(|e| {
                                        format!("Failed to get type representation: {}", e)
                                    })?;

                                if let crate::wasm_repr::WasmRepr::Scalar(val_type) = repr {
                                    // Push address first
                                    func.instruction(&Instruction::I32Const(*address as i32));

                                    // Then emit the initialization expression (pushes value onto stack)
                                    body_codegen.emit_expr(func, expr)?;

                                    match val_type {
                                        ValType::I32 => {
                                            func.instruction(&Instruction::I32Store(
                                                wasm_encoder::MemArg {
                                                    offset: 0,
                                                    align: 2, // 4-byte alignment
                                                    memory_index: 0,
                                                },
                                            ));
                                        }
                                        ValType::F32 => {
                                            func.instruction(&Instruction::F32Store(
                                                wasm_encoder::MemArg {
                                                    offset: 0,
                                                    align: 2, // 4-byte alignment
                                                    memory_index: 0,
                                                },
                                            ));
                                        }
                                        ValType::I64 => {
                                            func.instruction(&Instruction::I64Store(
                                                wasm_encoder::MemArg {
                                                    offset: 0,
                                                    align: 3, // 8-byte alignment
                                                    memory_index: 0,
                                                },
                                            ));
                                        }
                                        ValType::F64 => {
                                            func.instruction(&Instruction::F64Store(
                                                wasm_encoder::MemArg {
                                                    offset: 0,
                                                    align: 3, // 8-byte alignment
                                                    memory_index: 0,
                                                },
                                            ));
                                        }
                                        _ => {
                                            return Err(format!(
                                                "Unsupported value type for memory initialization: {:?}",
                                                val_type
                                            ));
                                        }
                                    }
                                }
                            }
                            _ => {
                                // TODO: Handle other initializer kinds (arrays, structs, etc.)
                                // For now, skip non-constant initializers
                            }
                        }
                    }
                }
            }
        }

        Ok(())
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
        let mut body_codegen =
            if let (Some(this_local), Some(instance)) = (self.this_local, self.instance) {
                // Method with 'this' pointer and instance context (FB or Class)
                BodyCodegen::new_with_this(
                    self.db,
                    &self.local_map,
                    self.return_local,
                    self.function_indices,
                    self.monomorphized_indices,
                    self.any_extern_functions,
                    this_local,
                    instance,
                    self.config,
                    self.debug_info,
                    self.debug_enabled_global,
                    self.debug_trap_id_global,
                )
            } else {
                // Regular function
                BodyCodegen::new(
                    self.db,
                    &self.local_map,
                    self.return_local,
                    self.function_indices,
                    self.monomorphized_indices,
                    self.any_extern_functions,
                    self.config,
                    self.debug_info,
                    self.debug_enabled_global,
                    self.debug_trap_id_global,
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
