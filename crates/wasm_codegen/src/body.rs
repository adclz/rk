//! Statement and expression code generation (function body emission).

use db::WorkspaceDataBase;
use hir::hir_def::{
    expressions::{
        expression::{Elementary, PrimaryExpr},
        spec::ElementarySpec,
    },
    interned::identifier::Ident,
};
use hir::hir_ty::{infer::Infer, ty::Type};
use rustc_hash::FxHashMap;
use wasm_encoder::{Instruction, ValType};

use crate::wasm_repr::{
    WasmRepr,
    elementary::{elementary_to_val_type, is_64bit, is_float, is_signed},
    instance::calculate_instance_field_offsets,
    strukt::calculate_field_offsets,
};

/// Information about a local variable.
#[derive(Debug, Clone, Copy)]
pub enum LocalInfo {
    /// Scalar value in WASM local (elementary types).
    Scalar {
        index: u32,
        val_type: ValType,
        spec: ElementarySpec,
    },

    /// Memory-resident value (array/struct).
    Memory { address: u32, size: u32, align: u32 },

    /// Pointer to memory (VAR_IN_OUT reference parameter).
    /// Stores the local index and the WASM representation of the pointee type.
    Pointer { index: u32, pointee_repr: WasmRepr },
}

/// Body code generation context (expressions and statements).
pub struct BodyCodegen<'db, 'a> {
    db: &'db dyn WorkspaceDataBase,
    local_map: &'a FxHashMap<Ident, LocalInfo>,
    return_local: Option<u32>,
    function_indices: &'a FxHashMap<Ident, u32>,

    /// For methods: the 'this' pointer local index.
    this_local: Option<u32>,

    /// For methods: the instance type (FunctionBlock or Class) for instance variable access.
    instance: Option<crate::func_codegen::InstanceType<'db>>,

    /// Debug configuration
    config: &'a crate::debug::CodeGenConfig,

    /// Debug information (mutable for recording traps)
    debug_info: &'a mut crate::debug::DebugInfo,

    /// Debug global indices
    debug_enabled_global: Option<u32>,
    debug_trap_id_global: Option<u32>,
}

impl<'db, 'a> BodyCodegen<'db, 'a> {
    pub fn new(
        db: &'db dyn WorkspaceDataBase,
        local_map: &'a FxHashMap<Ident, LocalInfo>,
        return_local: Option<u32>,
        function_indices: &'a FxHashMap<Ident, u32>,
        config: &'a crate::debug::CodeGenConfig,
        debug_info: &'a mut crate::debug::DebugInfo,
        debug_enabled_global: Option<u32>,
        debug_trap_id_global: Option<u32>,
    ) -> Self {
        Self {
            db,
            local_map,
            return_local,
            function_indices,
            this_local: None,
            instance: None,
            config,
            debug_info,
            debug_enabled_global,
            debug_trap_id_global,
        }
    }

    pub fn new_with_this(
        db: &'db dyn WorkspaceDataBase,
        local_map: &'a FxHashMap<Ident, LocalInfo>,
        return_local: Option<u32>,
        function_indices: &'a FxHashMap<Ident, u32>,
        this_local: u32,
        instance: crate::func_codegen::InstanceType<'db>,
        config: &'a crate::debug::CodeGenConfig,
        debug_info: &'a mut crate::debug::DebugInfo,
        debug_enabled_global: Option<u32>,
        debug_trap_id_global: Option<u32>,
    ) -> Self {
        Self {
            db,
            local_map,
            return_local,
            function_indices,
            this_local: Some(this_local),
            instance: Some(instance),
            config,
            debug_info,
            debug_enabled_global,
            debug_trap_id_global,
        }
    }

    /// Emit all statements in the function body.
    pub fn emit_statements(
        &mut self,
        func: &mut wasm_encoder::Function,
        statements: &[hir::hir_def::expressions::statement::Stmt<'db>],
    ) -> Result<(), String> {
        for stmt in statements {
            self.emit_stmt(func, *stmt)?;
        }
        Ok(())
    }

    /// Emit a statement.
    fn emit_stmt(
        &mut self,
        func: &mut wasm_encoder::Function,
        stmt: hir::hir_def::expressions::statement::Stmt<'db>,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::statement::StmtKind;

        // Inject debug trap at statement boundary
        if self.config.debug_mode != crate::debug::DebugMode::None {
            // Create a simple placeholder source location
            // TODO: Get actual file/line/column from AST node
            let location = crate::debug::SourceLocation {
                file_id: 0,                                   // Placeholder file ID
                line: self.debug_info.traps.len() as u32 + 1, // Unique line per trap
                column: 0,
            };

            // Create emitter and inject trap
            let mut emitter = crate::emitter::InstructionEmitter::new(
                func,
                self.config,
                self.debug_info,
                self.debug_enabled_global,
                self.debug_trap_id_global,
            );
            emitter.begin_statement(location);
        }

        match stmt.stmt(self.db) {
            StmtKind::Assignment { var, target } => {
                // Check if the LHS is an array index or struct field
                if let Some(path_expr) = self.get_path_expr(*var)? {
                    // Check if this is actually an indexed or field access, not just a simple variable
                    use hir::hir_def::expressions::expression::PathExprKind;
                    match path_expr.expr(self.db) {
                        PathExprKind::Index(_) | PathExprKind::Field(_) => {
                            // This is an array index or struct field assignment
                            self.emit_path_assignment(func, path_expr, *target)?;
                        }
                        PathExprKind::VarAccess(var_access) => {
                            // Check if this is a dereferenced pointer (ptr^ := value)
                            use hir::hir_def::expressions::expression::VarAccess;

                            match var_access {
                                VarAccess::Simple(_) => {
                                    // Simple variable assignment
                                    self.emit_simple_assignment(func, *var, *target)?;
                                }
                                VarAccess::Deref(span_ident, deref_count) => {
                                    // Assignment to dereferenced pointer: ptr^ := value
                                    self.emit_deref_assignment(
                                        func,
                                        span_ident,
                                        deref_count,
                                        path_expr,
                                        *target,
                                    )?;
                                }
                            }
                        }
                    }
                } else {
                    self.emit_simple_assignment(func, *var, *target)?;
                }

                Ok(())
            }

            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                // Emit the main condition
                self.emit_expr(func, *condition)?;

                // Start if block
                func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));

                // Emit then branch
                if let Some(then_stmts) = then {
                    for stmt in then_stmts {
                        self.emit_stmt(func, *stmt)?;
                    }
                }

                // Emit elsif branches
                for (elsif_cond, elsif_stmts) in else_if {
                    func.instruction(&Instruction::Else);
                    self.emit_expr(func, *elsif_cond)?;
                    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
                    for stmt in elsif_stmts {
                        self.emit_stmt(func, *stmt)?;
                    }
                }

                // Emit else branch
                if let Some(else_stmts) = else_ {
                    func.instruction(&Instruction::Else);
                    for stmt in else_stmts {
                        self.emit_stmt(func, *stmt)?;
                    }
                }

                // Close all if blocks
                // Main if + one for each elsif
                func.instruction(&Instruction::End);
                for _ in else_if {
                    func.instruction(&Instruction::End);
                }

                Ok(())
            }

            StmtKind::Return => {
                // Return statement - jump to end of function
                if let Some(ret_idx) = self.return_local {
                    func.instruction(&Instruction::LocalGet(ret_idx));
                }
                func.instruction(&Instruction::Return);
                Ok(())
            }

            StmtKind::While { condition, body } => {
                // WHILE loop pattern:
                // block         // outer block for EXIT
                //   loop        // inner loop for CONTINUE
                //     <condition>
                //     i32.eqz
                //     br_if 1   // exit if condition false
                //     <body>
                //     br 0      // continue loop
                //   end
                // end

                func.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
                func.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));

                // Emit condition and invert (exit if false)
                self.emit_expr(func, *condition)?;
                func.instruction(&Instruction::I32Eqz); // Invert condition
                func.instruction(&Instruction::BrIf(1)); // Exit to outer block if false

                // Emit loop body
                for stmt in body {
                    self.emit_stmt(func, *stmt)?;
                }

                // Continue loop
                func.instruction(&Instruction::Br(0));

                func.instruction(&Instruction::End); // end loop
                func.instruction(&Instruction::End); // end block

                Ok(())
            }

            StmtKind::Repeat { condition, body } => {
                // REPEAT...UNTIL loop pattern:
                // block         // outer block for EXIT
                //   loop        // inner loop for CONTINUE
                //     <body>
                //     <condition>
                //     br_if 1   // exit if condition true
                //     br 0      // continue loop
                //   end
                // end

                func.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
                func.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));

                // Emit loop body
                for stmt in body {
                    self.emit_stmt(func, *stmt)?;
                }

                // Emit condition (exit if true)
                self.emit_expr(func, *condition)?;
                func.instruction(&Instruction::BrIf(1)); // Exit to outer block if true

                // Continue loop
                func.instruction(&Instruction::Br(0));

                func.instruction(&Instruction::End); // end loop
                func.instruction(&Instruction::End); // end block

                Ok(())
            }

            StmtKind::For {
                control_variable,
                start,
                end,
                step,
                body,
            } => {
                // FOR loop pattern:
                // <start> -> local.set $ctrl
                // block         // outer block for EXIT
                //   loop        // inner loop for CONTINUE
                //     local.get $ctrl
                //     <end>
                //     i32.gt_s  // ctrl > end (assuming step is positive)
                //     br_if 1   // exit if ctrl > end
                //     <body>
                //     local.get $ctrl
                //     <step>    // or i32.const 1 if no step
                //     i32.add
                //     local.set $ctrl
                //     br 0      // continue loop
                //   end
                // end

                // Get control variable
                let ctrl_name = self.resolve_variable_name(*control_variable)?;
                let ctrl_info = self
                    .local_map
                    .get(&ctrl_name)
                    .ok_or_else(|| format!("Undefined control variable: {:?}", ctrl_name))?;

                // Control variable must be a scalar
                let (ctrl_index, ctrl_spec) = match ctrl_info {
                    LocalInfo::Scalar { index, spec, .. } => (*index, *spec),
                    _ => return Err("FOR loop control variable must be scalar".to_string()),
                };

                // Initialize control variable
                self.emit_expr(func, *start)?;
                func.instruction(&Instruction::LocalSet(ctrl_index));

                func.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
                func.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));

                // Check loop condition: ctrl > end (for positive step) or ctrl < end (for negative step)
                func.instruction(&Instruction::LocalGet(ctrl_index));
                self.emit_expr(func, *end)?;

                // TODO: Handle negative step properly
                // For now, assume step is always positive
                if is_signed(ctrl_spec) {
                    func.instruction(&Instruction::I32GtS);
                } else {
                    func.instruction(&Instruction::I32GtU);
                }
                func.instruction(&Instruction::BrIf(1)); // Exit if ctrl > end

                // Emit loop body
                for stmt in body {
                    self.emit_stmt(func, *stmt)?;
                }

                // Increment control variable
                func.instruction(&Instruction::LocalGet(ctrl_index));
                if let Some(step_expr) = step {
                    self.emit_expr(func, *step_expr)?;
                } else {
                    // Default step is 1
                    func.instruction(&Instruction::I32Const(1));
                }
                func.instruction(&Instruction::I32Add);
                func.instruction(&Instruction::LocalSet(ctrl_index));

                // Continue loop
                func.instruction(&Instruction::Br(0));

                func.instruction(&Instruction::End); // end loop
                func.instruction(&Instruction::End); // end block

                Ok(())
            }

            StmtKind::Exit => {
                // EXIT breaks out of the innermost loop (br 1 - exits the block surrounding the loop)
                func.instruction(&Instruction::Br(1));
                Ok(())
            }

            StmtKind::Continue => {
                // CONTINUE jumps back to loop start (br 0 - continues the loop)
                func.instruction(&Instruction::Br(0));
                Ok(())
            }

            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                // CASE statement pattern using nested if-else chain:
                // Evaluate condition once and store in a temporary local
                // For each case, compare and branch

                // First, we need to find an unused local for the temporary
                // For now, we'll just emit the condition multiple times (suboptimal but correct)

                // Build nested if-else chain
                let mut first = true;
                for (case_labels, case_stmts) in cases {
                    if !first {
                        func.instruction(&Instruction::Else);
                    }

                    // Build condition: check if any case label matches
                    let mut label_iter = case_labels.iter();
                    if let Some(first_label) = label_iter.next() {
                        self.emit_case_label_condition(func, *condition, first_label)?;

                        // OR with remaining labels
                        for label in label_iter {
                            self.emit_case_label_condition(func, *condition, label)?;
                            func.instruction(&Instruction::I32Or);
                        }
                    } else {
                        // No labels? Skip this case
                        continue;
                    }

                    func.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
                    first = false;

                    // Emit case body
                    for stmt in case_stmts {
                        self.emit_stmt(func, *stmt)?;
                    }
                }

                // Emit else branch if present
                if let Some(else_stmts) = else_ {
                    func.instruction(&Instruction::Else);
                    for stmt in else_stmts {
                        self.emit_stmt(func, *stmt)?;
                    }
                }

                // Close all if blocks
                for _ in 0..cases.len() {
                    func.instruction(&Instruction::End);
                }

                Ok(())
            }

            _ => Err(format!(
                "Unsupported statement kind: {:?}",
                stmt.stmt(self.db)
            )),
        }
    }

    /// Emit an expression (leaves one value on the stack).
    pub fn emit_expr(
        &self,
        func: &mut wasm_encoder::Function,
        expr: hir::hir_def::expressions::expression::Expr<'db>,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::expression::{
            AddOperatorKind, BooleanOperatorKind, ComparisonOperatorKind, ExprKind,
            MultOperatorKind, UnaryOperatorKind,
        };

        match expr.expr(self.db) {
            ExprKind::PrimaryExpr(primary) => self.emit_primary_expr(func, primary),

            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                // Get the result type (the target type for the operation)
                let result_type = expr.infer(self.db).normalize(self.db);
                let result_spec = self.extract_elementary_spec(result_type)?;

                // Emit left operand and cast if needed
                self.emit_expr(func, *left)?;
                let left_type = left.infer(self.db).normalize(self.db);
                if let Type::Elementary(left_spec) = left_type {
                    let cast_instructions = crate::cast::emit_cast(left_spec, result_spec);
                    for instr in cast_instructions {
                        func.instruction(&instr);
                    }
                }

                // Emit right operand and cast if needed
                self.emit_expr(func, *right)?;
                let right_type = right.infer(self.db).normalize(self.db);
                if let Type::Elementary(right_spec) = right_type {
                    let cast_instructions = crate::cast::emit_cast(right_spec, result_spec);
                    for instr in cast_instructions {
                        func.instruction(&instr);
                    }
                }

                match operator {
                    AddOperatorKind::Plus => self.emit_add_instruction(func, result_spec),
                    AddOperatorKind::Minus => self.emit_sub_instruction(func, result_spec),
                }
            }

            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                // Get the result type (the target type for the operation)
                let result_type = expr.infer(self.db).normalize(self.db);
                let result_spec = self.extract_elementary_spec(result_type)?;

                // Emit left operand and cast if needed
                self.emit_expr(func, *left)?;
                let left_type = left.infer(self.db).normalize(self.db);
                if let Type::Elementary(left_spec) = left_type {
                    let cast_instructions = crate::cast::emit_cast(left_spec, result_spec);
                    for instr in cast_instructions {
                        func.instruction(&instr);
                    }
                }

                // Emit right operand and cast if needed
                self.emit_expr(func, *right)?;
                let right_type = right.infer(self.db).normalize(self.db);
                if let Type::Elementary(right_spec) = right_type {
                    let cast_instructions = crate::cast::emit_cast(right_spec, result_spec);
                    for instr in cast_instructions {
                        func.instruction(&instr);
                    }
                }

                match operator {
                    MultOperatorKind::Mul => self.emit_mul_instruction(func, result_spec),
                    MultOperatorKind::Div => self.emit_div_instruction(func, result_spec),
                    MultOperatorKind::Mod => self.emit_mod_instruction(func, result_spec),
                }
            }

            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                // For comparison, we need to cast both operands to a common type
                // Get both operand types
                let left_type = left.infer(self.db).normalize(self.db);
                let right_type = right.infer(self.db).normalize(self.db);

                // Determine the common type (wider type for comparison)
                // Use the left type as base, but if right is wider, use that
                let left_spec = self.extract_elementary_spec(left_type)?;
                let right_spec = self.extract_elementary_spec(right_type)?;

                // Determine common type (simple rule: use the wider/float type)
                let common_spec = if is_float(left_spec) || is_float(right_spec) {
                    // If either is float, promote to float
                    if is_64bit(left_spec) || is_64bit(right_spec) {
                        ElementarySpec::LReal // f64
                    } else {
                        ElementarySpec::Real // f32
                    }
                } else if is_64bit(left_spec) || is_64bit(right_spec) {
                    // If either is 64-bit int, promote to i64
                    if is_signed(left_spec) || is_signed(right_spec) {
                        ElementarySpec::LInt
                    } else {
                        ElementarySpec::ULInt
                    }
                } else {
                    // Both are 32-bit or smaller, use left type
                    left_spec
                };

                // Emit left operand and cast if needed
                self.emit_expr(func, *left)?;
                let cast_instructions = crate::cast::emit_cast(left_spec, common_spec);
                for instr in cast_instructions {
                    func.instruction(&instr);
                }

                // Emit right operand and cast if needed
                self.emit_expr(func, *right)?;
                let cast_instructions = crate::cast::emit_cast(right_spec, common_spec);
                for instr in cast_instructions {
                    func.instruction(&instr);
                }

                match operator {
                    ComparisonOperatorKind::Eq => self.emit_eq_instruction(func, common_spec),
                    ComparisonOperatorKind::Ne => self.emit_ne_instruction(func, common_spec),
                    ComparisonOperatorKind::Lt => self.emit_lt_instruction(func, common_spec),
                    ComparisonOperatorKind::Le => self.emit_le_instruction(func, common_spec),
                    ComparisonOperatorKind::Gt => self.emit_gt_instruction(func, common_spec),
                    ComparisonOperatorKind::Ge => self.emit_ge_instruction(func, common_spec),
                }
            }

            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                self.emit_expr(func, *left)?;
                self.emit_expr(func, *right)?;

                // Determine operand type (i32 or i64)
                let left_type = left.infer(self.db).normalize(self.db);
                let spec = self.extract_elementary_spec(left_type)?;
                let val_type = elementary_to_val_type(spec).map_err(|e| e.to_string())?;

                match (operator, val_type) {
                    (BooleanOperatorKind::And, ValType::I32) => {
                        func.instruction(&Instruction::I32And);
                        Ok(())
                    }
                    (BooleanOperatorKind::And, ValType::I64) => {
                        func.instruction(&Instruction::I64And);
                        Ok(())
                    }
                    (BooleanOperatorKind::Or, ValType::I32) => {
                        func.instruction(&Instruction::I32Or);
                        Ok(())
                    }
                    (BooleanOperatorKind::Or, ValType::I64) => {
                        func.instruction(&Instruction::I64Or);
                        Ok(())
                    }
                    (BooleanOperatorKind::Xor, ValType::I32) => {
                        func.instruction(&Instruction::I32Xor);
                        Ok(())
                    }
                    (BooleanOperatorKind::Xor, ValType::I64) => {
                        func.instruction(&Instruction::I64Xor);
                        Ok(())
                    }
                    _ => Err(format!(
                        "Unsupported boolean operator on type: {:?}",
                        val_type
                    )),
                }
            }

            ExprKind::UnaryOperator { expr, operator } => {
                match operator {
                    UnaryOperatorKind::Plus => {
                        // Unary plus is a no-op
                        self.emit_expr(func, *expr)
                    }
                    UnaryOperatorKind::Minus => {
                        // Negate: push 0, push value, subtract
                        let expr_type = expr.infer(self.db).normalize(self.db);
                        let spec = self.extract_elementary_spec(expr_type)?;

                        if is_float(spec) {
                            // For floats, use neg instruction
                            self.emit_expr(func, *expr)?;
                            match spec {
                                ElementarySpec::Real => {
                                    func.instruction(&Instruction::F32Neg);
                                }
                                ElementarySpec::LReal => {
                                    func.instruction(&Instruction::F64Neg);
                                }
                                _ => unreachable!(),
                            }
                            Ok(())
                        } else {
                            // For integers: 0 - value
                            if is_64bit(spec) {
                                func.instruction(&Instruction::I64Const(0));
                            } else {
                                func.instruction(&Instruction::I32Const(0));
                            }
                            self.emit_expr(func, *expr)?;
                            self.emit_sub_instruction(func, spec)
                        }
                    }
                    UnaryOperatorKind::Not => {
                        let expr_type = expr.infer(self.db).normalize(self.db);
                        let spec = self.extract_elementary_spec(expr_type)?;

                        self.emit_expr(func, *expr)?;

                        // Distinguish between boolean NOT and bitwise NOT
                        match spec {
                            ElementarySpec::Bool => {
                                // Boolean NOT: value == 0 (TRUE becomes FALSE, FALSE becomes TRUE)
                                func.instruction(&Instruction::I32Eqz);
                                Ok(())
                            }
                            // Bit strings: flip all bits using XOR with all 1s
                            ElementarySpec::Byte | ElementarySpec::Word | ElementarySpec::DWord => {
                                // NOT x = x XOR 0xFFFFFFFF
                                func.instruction(&Instruction::I32Const(-1));
                                func.instruction(&Instruction::I32Xor);
                                Ok(())
                            }
                            ElementarySpec::LWord => {
                                // NOT x = x XOR 0xFFFFFFFFFFFFFFFF
                                func.instruction(&Instruction::I64Const(-1));
                                func.instruction(&Instruction::I64Xor);
                                Ok(())
                            }
                            _ => Err(format!("NOT operator not supported for type: {:?}", spec)),
                        }
                    }
                }
            }

            ExprKind::PowerOperator { .. } => Err("Power operator not yet supported".to_string()),
            ExprKind::FoldExpr { .. } => {
                Err("Fold expression not yet supported in codegen".to_string())
            }
        }
    }

    /// Emit a primary expression.
    fn emit_primary_expr(
        &self,
        func: &mut wasm_encoder::Function,
        primary: &PrimaryExpr<'db>,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::expression::PrimaryExpr;

        match primary {
            PrimaryExpr::Literal(elem) => self.emit_literal(func, elem),

            PrimaryExpr::VariableAccess(var_access) => {
                // Check if this is an array index or field access
                if let Some(path_expr) = self.get_path_expr(*var_access)? {
                    use hir::hir_def::expressions::expression::PathExprKind;
                    match path_expr.expr(self.db) {
                        PathExprKind::Index(_) | PathExprKind::Field(_) => {
                            // This is an indexed or field access
                            self.emit_path_access(func, path_expr)?;
                        }
                        PathExprKind::VarAccess(var_acc) => {
                            // Check if this is a dereferenced variable (ptr^)
                            use hir::hir_def::expressions::expression::VarAccess;

                            match var_acc {
                                VarAccess::Simple(_) => {
                                    // Simple variable access (no deref)
                                    let var_name = self.resolve_variable_name(*var_access)?;

                                    // Try to get from local_map first
                                    if let Some(local_info) = self.local_map.get(&var_name) {
                                        match local_info {
                                            LocalInfo::Scalar { index, .. } => {
                                                func.instruction(&Instruction::LocalGet(*index));
                                            }
                                            LocalInfo::Memory { address, size, .. } => {
                                                // Load from memory location
                                                func.instruction(&Instruction::I32Const(
                                                    *address as i32,
                                                ));

                                                // Determine load instruction based on size
                                                // Memory variables are scalars that were allocated due to address-taken
                                                match size {
                                                    4 => {
                                                        // Could be i32 or f32, assume i32 for now
                                                        // TODO: Get actual type to distinguish i32/f32
                                                        func.instruction(&Instruction::I32Load(
                                                            wasm_encoder::MemArg {
                                                                offset: 0,
                                                                align: 2, // 4-byte alignment
                                                                memory_index: 0,
                                                            },
                                                        ));
                                                    }
                                                    8 => {
                                                        // Could be i64 or f64, assume i64 for now
                                                        func.instruction(&Instruction::I64Load(
                                                            wasm_encoder::MemArg {
                                                                offset: 0,
                                                                align: 3, // 8-byte alignment
                                                                memory_index: 0,
                                                            },
                                                        ));
                                                    }
                                                    _ => {
                                                        return Err(format!(
                                                            "Unsupported memory variable size: {}",
                                                            size
                                                        ));
                                                    }
                                                }
                                            }
                                            LocalInfo::Pointer {
                                                index,
                                                pointee_repr,
                                            } => {
                                                // VAR_IN_OUT parameter - pointer to value
                                                // Load the pointer value (address)
                                                func.instruction(&Instruction::LocalGet(*index));

                                                // Emit load instruction to dereference the pointer
                                                self.emit_load_instruction(func, *pointee_repr)?;
                                            }
                                        }
                                    } else {
                                        // Variable not in local_map - check if it's an FB instance variable
                                        self.emit_instance_var_load(func, var_name)?;
                                    }
                                }

                                VarAccess::Deref(span_ident, deref_count) => {
                                    // Dereferenced pointer variable (ptr^)
                                    let var_name = span_ident.ident;

                                    // Load the pointer variable
                                    if let Some(local_info) = self.local_map.get(&var_name) {
                                        match local_info {
                                            LocalInfo::Scalar {
                                                index,
                                                val_type: _,
                                                spec: _,
                                            } => {
                                                // Load the pointer value (should be i32 address)
                                                func.instruction(&Instruction::LocalGet(*index));

                                                // Dereference the pointer
                                                // For each ^ operator, emit a load instruction
                                                for _ in 0..deref_count {
                                                    // Get the pointee type from the HIR type system
                                                    // Use the path expression to infer the type
                                                    let path_type = path_expr.infer(self.db);
                                                    let pointee_repr =
                                                        self.get_deref_pointee_type(path_type)?;
                                                    self.emit_load_instruction(func, pointee_repr)?;
                                                }
                                            }
                                            LocalInfo::Pointer {
                                                index,
                                                pointee_repr,
                                            } => {
                                                // This is already a pointer (VAR_IN_OUT)
                                                func.instruction(&Instruction::LocalGet(*index));

                                                // Dereference
                                                for _ in 0..deref_count {
                                                    self.emit_load_instruction(
                                                        func,
                                                        *pointee_repr,
                                                    )?;
                                                }
                                            }
                                            LocalInfo::Memory { .. } => {
                                                return Err(
                                                    "Cannot dereference memory-resident variable"
                                                        .to_string(),
                                                );
                                            }
                                        }
                                    } else {
                                        return Err(format!(
                                            "Dereferenced variable '{}' not found in scope",
                                            var_name.text(self.db)
                                        ));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // No path expression - simple variable
                    let var_name = self.resolve_variable_name(*var_access)?;

                    if let Some(local_info) = self.local_map.get(&var_name) {
                        match local_info {
                            LocalInfo::Scalar { index, .. } => {
                                func.instruction(&Instruction::LocalGet(*index));
                            }
                            LocalInfo::Memory { address, size, .. } => {
                                // Load from memory location
                                func.instruction(&Instruction::I32Const(*address as i32));

                                // Determine load instruction based on size
                                match size {
                                    4 => {
                                        // 4-byte value (i32 or f32)
                                        func.instruction(&Instruction::I32Load(
                                            wasm_encoder::MemArg {
                                                offset: 0,
                                                align: 2, // 4-byte alignment
                                                memory_index: 0,
                                            },
                                        ));
                                    }
                                    8 => {
                                        // 8-byte value (i64 or f64)
                                        func.instruction(&Instruction::I64Load(
                                            wasm_encoder::MemArg {
                                                offset: 0,
                                                align: 3, // 8-byte alignment
                                                memory_index: 0,
                                            },
                                        ));
                                    }
                                    _ => {
                                        return Err(format!(
                                            "Unsupported memory variable size: {}",
                                            size
                                        ));
                                    }
                                }
                            }
                            LocalInfo::Pointer {
                                index,
                                pointee_repr,
                            } => {
                                // VAR_IN_OUT parameter - pointer to value
                                // Load the pointer value (address)
                                func.instruction(&Instruction::LocalGet(*index));

                                // Emit load instruction to dereference the pointer
                                self.emit_load_instruction(func, *pointee_repr)?;
                            }
                        }
                    } else {
                        // Variable not in local_map - check if it's an FB instance variable
                        self.emit_instance_var_load(func, var_name)?;
                    }
                }
                Ok(())
            }

            PrimaryExpr::ParenthesizedExpr { expr } => self.emit_expr(func, *expr),

            PrimaryExpr::FuncCall(call) => {
                let path = call.path(self.db);

                // Check if this is a method call (instance.method())
                if let Some(path_expr) = path.expr(self.db) {
                    use hir::hir_def::expressions::expression::PathExprKind;

                    if let PathExprKind::Field(field_expr) = path_expr.expr(self.db) {
                        // This is a method call: instance.method()
                        // Resolve the instance variable (base of the field access)
                        let instance_path = field_expr.path;

                        // Get the method name from the VarAccess
                        use hir::hir_def::expressions::expression::VarAccess;
                        let method_name = match field_expr.var {
                            VarAccess::Simple(span_ident) => span_ident.ident,
                            VarAccess::Deref(span_ident, _) => span_ident.ident,
                        };

                        // Get the type of the instance
                        let instance_type = instance_path.infer(self.db).normalize(self.db);

                        // Determine the FB/Class name
                        let fb_class_name = match instance_type {
                            Type::FunctionBlock(fb) => fb.name(self.db),
                            Type::Class(class) => class.name(self.db),
                            _ => {
                                return Err(format!(
                                    "Method call on non-instance type: {:?}",
                                    instance_type
                                ));
                            }
                        };

                        // Construct the method function name: FBName$MethodName
                        let method_func_name = Ident::from_slice(
                            self.db,
                            &format!(
                                "{}${}",
                                fb_class_name.text(self.db),
                                method_name.text(self.db)
                            ),
                        );

                        // Look up method index
                        let func_idx =
                            self.function_indices
                                .get(&method_func_name)
                                .ok_or_else(|| {
                                    format!(
                                        "Undefined method: {}.{}",
                                        fb_class_name.text(self.db),
                                        method_name.text(self.db)
                                    )
                                })?;

                        // Emit instance address as first parameter
                        self.emit_instance_address(func, instance_path)?;

                        // Emit method parameters
                        for param in call.params(self.db) {
                            self.emit_param(func, *param)?;
                        }

                        // Emit call instruction
                        func.instruction(&Instruction::Call(*func_idx));

                        return Ok(());
                    }
                }

                // Regular function call (not a method)
                let func_name = self.resolve_function_name(*call)?;

                // Look up function index
                let func_idx = self
                    .function_indices
                    .get(&func_name)
                    .ok_or_else(|| format!("Undefined function: {:?}", func_name))?;

                // Emit parameters
                for param in call.params(self.db) {
                    self.emit_param(func, *param)?;
                }

                // Emit call instruction
                func.instruction(&Instruction::Call(*func_idx));

                Ok(())
            }

            PrimaryExpr::RefValue { value } => {
                use hir::hir_def::expressions::expression::RefValue;

                match value {
                    RefValue::Address(begin_path) => {
                        // REF(variable) - get address of variable
                        // For now, we'll use a simple approach:
                        // - Variables in memory: use their memory address
                        // - Variables in WASM locals: allocate them in memory first (TODO)

                        // Extract variable name from the path expression
                        let var_name = if let Some(path_expr) = begin_path.expr(self.db) {
                            path_expr.ident(self.db).ident
                        } else {
                            return Err(
                                "REF operator requires a simple variable reference".to_string()
                            );
                        };

                        // Check if variable is in local_map
                        if let Some(local_info) = self.local_map.get(&var_name) {
                            match local_info {
                                LocalInfo::Memory { address, .. } => {
                                    // Variable is already in memory - return its address
                                    func.instruction(&Instruction::I32Const(*address as i32));
                                }
                                LocalInfo::Scalar { .. } => {
                                    // TODO: For scalar locals stored in WASM locals,
                                    // we need to allocate them in memory to get an address.
                                    // For now, return an error.
                                    return Err(format!(
                                        "REF operator on WASM local variables not yet supported. \
                                         Variable '{}' must be allocated in linear memory.",
                                        var_name.text(self.db)
                                    ));
                                }
                                LocalInfo::Pointer { index, .. } => {
                                    // This is a VAR_IN_OUT parameter - it's already a pointer
                                    // Just return the pointer value itself
                                    func.instruction(&Instruction::LocalGet(*index));
                                }
                            }
                        } else {
                            return Err(format!(
                                "Cannot take reference of variable '{}' - not found in scope",
                                var_name.text(self.db)
                            ));
                        }

                        Ok(())
                    }

                    RefValue::Null => {
                        // NULL pointer - represented as 0 in WASM
                        func.instruction(&Instruction::I32Const(0));
                        Ok(())
                    }
                }
            }

            _ => Err(format!("Unsupported primary expression: {:?}", primary)),
        }
    }

    /// Emit a literal constant.
    fn emit_literal(
        &self,
        func: &mut wasm_encoder::Function,
        elem: &Elementary,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::expression::Elementary;

        match elem {
            Elementary::Bool(ident) => {
                let val = ident
                    .as_bool(self.db)
                    .map_err(|e| format!("Invalid BOOL literal: {}", e))?;
                func.instruction(&Instruction::I32Const(if val { 1 } else { 0 }));
                Ok(())
            }

            // Signed integers
            Elementary::SInt(n) => {
                let val = n
                    .as_i8(self.db)
                    .map_err(|e| format!("Invalid SINT literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val as i32));
                Ok(())
            }
            Elementary::Int(n) => {
                let val = n
                    .as_i16(self.db)
                    .map_err(|e| format!("Invalid INT literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val as i32));
                Ok(())
            }
            Elementary::DInt(n) => {
                let val = n
                    .as_i32(self.db)
                    .map_err(|e| format!("Invalid DINT literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val));
                Ok(())
            }
            Elementary::LInt(n) => {
                let val = n
                    .as_i64(self.db)
                    .map_err(|e| format!("Invalid LINT literal: {}", e))?;
                func.instruction(&Instruction::I64Const(val));
                Ok(())
            }

            // Unsigned integers
            Elementary::USInt(n) => {
                let val = n
                    .as_u8(self.db)
                    .map_err(|e| format!("Invalid USINT literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val as i32));
                Ok(())
            }
            Elementary::UInt(n) => {
                let val = n
                    .as_u16(self.db)
                    .map_err(|e| format!("Invalid UINT literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val as i32));
                Ok(())
            }
            Elementary::UDInt(n) => {
                let val = n
                    .as_u32(self.db)
                    .map_err(|e| format!("Invalid UDINT literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val as i32));
                Ok(())
            }
            Elementary::ULInt(n) => {
                let val = n
                    .as_u64(self.db)
                    .map_err(|e| format!("Invalid ULINT literal: {}", e))?;
                func.instruction(&Instruction::I64Const(val as i64));
                Ok(())
            }

            // Floats
            Elementary::Real(ident) => {
                let val = ident
                    .as_f32(self.db)
                    .map_err(|e| format!("Invalid REAL literal: {}", e))?;
                func.instruction(&Instruction::F32Const(val.into()));
                Ok(())
            }
            Elementary::LReal(ident) => {
                let val = ident
                    .as_f64(self.db)
                    .map_err(|e| format!("Invalid LREAL literal: {}", e))?;
                func.instruction(&Instruction::F64Const(val.into()));
                Ok(())
            }

            // Infer types - need to determine actual type from context
            Elementary::InferInteger(n) => {
                // Default to i32 for now
                // TODO: Use actual inferred type from type system
                let val = n
                    .as_i32(self.db)
                    .map_err(|e| format!("Invalid integer literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val));
                Ok(())
            }
            Elementary::InferFloat(ident) => {
                // Default to f32 for now
                // TODO: Use actual inferred type from type system
                let val = ident
                    .as_f32(self.db)
                    .map_err(|e| format!("Invalid float literal: {}", e))?;
                func.instruction(&Instruction::F32Const(val.into()));
                Ok(())
            }

            // Bit strings
            Elementary::Byte(n) => {
                let val = n
                    .as_u8(self.db)
                    .map_err(|e| format!("Invalid BYTE literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val as i32));
                Ok(())
            }
            Elementary::Word(n) => {
                let val = n
                    .as_u16(self.db)
                    .map_err(|e| format!("Invalid WORD literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val as i32));
                Ok(())
            }
            Elementary::DWord(n) => {
                let val = n
                    .as_u32(self.db)
                    .map_err(|e| format!("Invalid DWORD literal: {}", e))?;
                func.instruction(&Instruction::I32Const(val as i32));
                Ok(())
            }
            Elementary::LWord(n) => {
                let val = n
                    .as_u64(self.db)
                    .map_err(|e| format!("Invalid LWORD literal: {}", e))?;
                func.instruction(&Instruction::I64Const(val as i64));
                Ok(())
            }

            _ => Err(format!("Unsupported literal type: {:?}", elem)),
        }
    }

    /// Resolve a variable access to its identifier.
    fn resolve_variable_name(
        &self,
        var_access: hir::hir_def::expressions::expression::VariableAccess<'db>,
    ) -> Result<Ident, String> {
        use hir::hir_def::expressions::expression::VariableAccessKind;

        match var_access.kind(self.db) {
            VariableAccessKind::Symbolic(begin) => {
                if let Some(path_expr) = begin.expr(self.db) {
                    let span_ident = path_expr.ident(self.db);
                    Ok(span_ident.ident)
                } else {
                    Err("Variable access has no path expression".to_string())
                }
            }
            VariableAccessKind::Direct(_) => {
                Err("Direct variable access not yet supported".to_string())
            }
        }
    }

    /// Resolve a function call to its function name identifier.
    fn resolve_function_name(
        &self,
        call: hir::hir_def::expressions::expression::FuncCall<'db>,
    ) -> Result<Ident, String> {
        let path = call.path(self.db);
        if let Some(path_expr) = path.expr(self.db) {
            let span_ident = path_expr.ident(self.db);
            Ok(span_ident.ident)
        } else {
            Err("Function call has no path expression".to_string())
        }
    }

    /// Emit the address of an instance variable (for method calls).
    /// The instance should be a FB or CLASS instance stored in memory.
    fn emit_instance_address(
        &self,
        func: &mut wasm_encoder::Function,
        instance_path: hir::hir_def::expressions::expression::PathExpr<'db>,
    ) -> Result<(), String> {
        // Get the identifier from the path
        let span_ident = instance_path.ident(self.db);
        let ident = span_ident.ident;

        // Look up the variable in local_map
        let local_info = self
            .local_map
            .get(&ident)
            .ok_or_else(|| format!("Undefined instance variable: {:?}", ident))?;

        // The instance should be in memory
        match local_info {
            LocalInfo::Memory { address, .. } => {
                // Emit the memory address as an i32 constant
                func.instruction(&Instruction::I32Const(*address as i32));
                Ok(())
            }
            LocalInfo::Pointer { index, .. } => {
                // If it's a pointer (VAR_IN_OUT), load the pointer value
                func.instruction(&Instruction::LocalGet(*index));
                Ok(())
            }
            LocalInfo::Scalar { .. } => Err(format!(
                "Cannot call method on scalar variable: {:?}",
                ident
            )),
        }
    }

    /// Emit a parameter value for a function call.
    fn emit_param(
        &self,
        func: &mut wasm_encoder::Function,
        param: hir::hir_def::expressions::expression::ParamAssign<'db>,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::expression::ParamAssignKind;

        match param.kind(self.db) {
            ParamAssignKind::NonFormal { value } => {
                // Positional parameter - just emit the expression
                self.emit_expr(func, value)
            }
            ParamAssignKind::FormalInput { param: _, value } => {
                // Named input parameter - emit the value (ignore the name for now)
                self.emit_expr(func, value)
            }
            ParamAssignKind::FormalOutput { .. } => {
                Err("Output parameters not yet supported".to_string())
            }
        }
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

    // Arithmetic instruction helpers
    fn emit_add_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Add);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Add);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            func.instruction(&Instruction::I64Add);
        } else {
            func.instruction(&Instruction::I32Add);
        }
        Ok(())
    }

    fn emit_sub_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Sub);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Sub);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            func.instruction(&Instruction::I64Sub);
        } else {
            func.instruction(&Instruction::I32Sub);
        }
        Ok(())
    }

    fn emit_mul_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Mul);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Mul);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            func.instruction(&Instruction::I64Mul);
        } else {
            func.instruction(&Instruction::I32Mul);
        }
        Ok(())
    }

    fn emit_div_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float, is_signed};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Div);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Div);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            if is_signed(spec) {
                func.instruction(&Instruction::I64DivS);
            } else {
                func.instruction(&Instruction::I64DivU);
            }
        } else if is_signed(spec) {
            func.instruction(&Instruction::I32DivS);
        } else {
            func.instruction(&Instruction::I32DivU);
        }
        Ok(())
    }

    fn emit_mod_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_signed};

        if is_64bit(spec) {
            if is_signed(spec) {
                func.instruction(&Instruction::I64RemS);
            } else {
                func.instruction(&Instruction::I64RemU);
            }
        } else if is_signed(spec) {
            func.instruction(&Instruction::I32RemS);
        } else {
            func.instruction(&Instruction::I32RemU);
        }
        Ok(())
    }

    /// Emit condition check for a case label (expression or subrange).
    fn emit_case_label_condition(
        &self,
        func: &mut wasm_encoder::Function,
        condition: hir::hir_def::expressions::expression::Expr<'db>,
        case_kind: &hir::hir_def::expressions::statement::CaseKind<'db>,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::statement::CaseKind;

        match case_kind {
            CaseKind::Expression(expr) => {
                // Simple equality check: condition = expr
                self.emit_expr(func, condition)?;
                self.emit_expr(func, *expr)?;

                // Get type for comparison instruction
                let expr_type = condition.infer(self.db).normalize(self.db);
                let spec = self.extract_elementary_spec(expr_type)?;
                self.emit_eq_instruction(func, spec)?;

                Ok(())
            }
            CaseKind::Subrange { lower, upper } => {
                // Range check: (condition >= lower) AND (condition <= upper)
                // Emit: (condition >= lower)
                self.emit_expr(func, condition)?;
                self.emit_expr(func, *lower)?;

                let expr_type = condition.infer(self.db).normalize(self.db);
                let spec = self.extract_elementary_spec(expr_type)?;
                self.emit_ge_instruction(func, spec)?;

                // Emit: (condition <= upper)
                self.emit_expr(func, condition)?;
                self.emit_expr(func, *upper)?;
                self.emit_le_instruction(func, spec)?;

                // AND the two conditions
                func.instruction(&Instruction::I32And);

                Ok(())
            }
        }
    }

    // Comparison instruction helpers
    fn emit_eq_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Eq);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Eq);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            func.instruction(&Instruction::I64Eq);
        } else {
            func.instruction(&Instruction::I32Eq);
        }
        Ok(())
    }

    fn emit_ne_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Ne);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Ne);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            func.instruction(&Instruction::I64Ne);
        } else {
            func.instruction(&Instruction::I32Ne);
        }
        Ok(())
    }

    fn emit_lt_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float, is_signed};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Lt);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Lt);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            if is_signed(spec) {
                func.instruction(&Instruction::I64LtS);
            } else {
                func.instruction(&Instruction::I64LtU);
            }
        } else if is_signed(spec) {
            func.instruction(&Instruction::I32LtS);
        } else {
            func.instruction(&Instruction::I32LtU);
        }
        Ok(())
    }

    fn emit_le_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float, is_signed};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Le);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Le);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            if is_signed(spec) {
                func.instruction(&Instruction::I64LeS);
            } else {
                func.instruction(&Instruction::I64LeU);
            }
        } else if is_signed(spec) {
            func.instruction(&Instruction::I32LeS);
        } else {
            func.instruction(&Instruction::I32LeU);
        }
        Ok(())
    }

    fn emit_gt_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float, is_signed};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Gt);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Gt);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            if is_signed(spec) {
                func.instruction(&Instruction::I64GtS);
            } else {
                func.instruction(&Instruction::I64GtU);
            }
        } else if is_signed(spec) {
            func.instruction(&Instruction::I32GtS);
        } else {
            func.instruction(&Instruction::I32GtU);
        }
        Ok(())
    }

    fn emit_ge_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        spec: ElementarySpec,
    ) -> Result<(), String> {
        use {is_64bit, is_float, is_signed};

        if is_float(spec) {
            match spec {
                ElementarySpec::Real => {
                    func.instruction(&Instruction::F32Ge);
                }
                ElementarySpec::LReal => {
                    func.instruction(&Instruction::F64Ge);
                }
                _ => unreachable!(),
            }
        } else if is_64bit(spec) {
            if is_signed(spec) {
                func.instruction(&Instruction::I64GeS);
            } else {
                func.instruction(&Instruction::I64GeU);
            }
        } else if is_signed(spec) {
            func.instruction(&Instruction::I32GeS);
        } else {
            func.instruction(&Instruction::I32GeU);
        }
        Ok(())
    }

    /// Extract PathExpr from a VariableAccess if it exists.
    fn get_path_expr(
        &self,
        var_access: hir::hir_def::expressions::expression::VariableAccess<'db>,
    ) -> Result<Option<hir::hir_def::expressions::expression::PathExpr<'db>>, String> {
        use hir::hir_def::expressions::expression::VariableAccessKind;

        match var_access.kind(self.db) {
            VariableAccessKind::Symbolic(begin) => Ok(begin.expr(self.db)),
            VariableAccessKind::Direct(_) => {
                Err("Direct variable access not yet supported".to_string())
            }
        }
    }

    /// Emit code to access a path expression (array index or struct field).
    fn emit_path_access(
        &self,
        func: &mut wasm_encoder::Function,
        path_expr: hir::hir_def::expressions::expression::PathExpr<'db>,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::expression::PathExprKind;

        match path_expr.expr(self.db) {
            PathExprKind::Index(index_expr) => {
                self.emit_array_index(func, &index_expr)?;
            }
            PathExprKind::Field(field_expr) => {
                self.emit_struct_field(func, &field_expr)?;
            }
            PathExprKind::VarAccess(_) => {
                // This shouldn't happen if we check properly, but treat as simple variable
                return Err("Unexpected VarAccess in PathExpr".to_string());
            }
        }
        Ok(())
    }

    /// Emit code for simple variable assignment (not array index or field).
    fn emit_simple_assignment(
        &self,
        func: &mut wasm_encoder::Function,
        var: hir::hir_def::expressions::expression::VariableAccess<'db>,
        target: hir::hir_def::expressions::expression::Expr<'db>,
    ) -> Result<(), String> {
        use hir::hir_ty::ty::Type;

        // Get the variable name
        let var_name = self.resolve_variable_name(var)?;

        // Check if we need to emit a cast
        let rhs_type = target.infer(self.db);
        let rhs_normalized = rhs_type.normalize(self.db);

        // Try to get from local_map first
        if let Some(local_info) = self.local_map.get(&var_name) {
            // Store based on LocalInfo variant
            match local_info {
                LocalInfo::Scalar { index, spec, .. } => {
                    // Emit the RHS expression
                    self.emit_expr(func, target)?;

                    // Get the type of the LHS variable
                    let lhs_spec = *spec;

                    // If RHS is also an elementary type, check if cast is needed
                    if let Type::Elementary(rhs_spec) = rhs_normalized {
                        // Emit cast instructions if types differ
                        let cast_instructions = crate::cast::emit_cast(rhs_spec, lhs_spec);
                        for instr in cast_instructions {
                            func.instruction(&instr);
                        }
                    }

                    // Store to local
                    func.instruction(&Instruction::LocalSet(*index));
                    Ok(())
                }
                LocalInfo::Memory { address, size, .. } => {
                    // Direct assignment to memory-resident scalar variable
                    // Stack order for store: [address, value]

                    // 1. Load memory address (constant)
                    func.instruction(&Instruction::I32Const(*address as i32));

                    // 2. Emit RHS value
                    self.emit_expr(func, target)?;

                    // 3. Emit store instruction based on size
                    match size {
                        4 => {
                            // 4-byte value (i32 or f32)
                            func.instruction(&Instruction::I32Store(wasm_encoder::MemArg {
                                offset: 0,
                                align: 2, // 4-byte alignment
                                memory_index: 0,
                            }));
                        }
                        8 => {
                            // 8-byte value (i64 or f64)
                            func.instruction(&Instruction::I64Store(wasm_encoder::MemArg {
                                offset: 0,
                                align: 3, // 8-byte alignment
                                memory_index: 0,
                            }));
                        }
                        _ => {
                            return Err(format!(
                                "Unsupported memory variable size for store: {}",
                                size
                            ));
                        }
                    }
                    Ok(())
                }
                LocalInfo::Pointer {
                    index,
                    pointee_repr,
                } => {
                    // VAR_IN_OUT parameter - store through pointer
                    // Stack order for store: [address, value]

                    // 1. Load pointer address
                    func.instruction(&Instruction::LocalGet(*index));

                    // 2. Emit RHS value
                    self.emit_expr(func, target)?;

                    // 3. Emit store instruction
                    self.emit_store_instruction(func, *pointee_repr)?;
                    Ok(())
                }
            }
        } else {
            // Variable not in local_map - check if it's an FB instance variable
            self.emit_instance_var_store(func, var_name, target, rhs_type)
        }
    }

    /// Emit code to assign through a dereferenced pointer (ptr^ := value).
    fn emit_deref_assignment(
        &self,
        func: &mut wasm_encoder::Function,
        ptr_ident: hir::hir_def::interned::identifier::SpanIdent<'db>,
        deref_count: u16,
        path_expr: hir::hir_def::expressions::expression::PathExpr<'db>,
        rhs: hir::hir_def::expressions::expression::Expr<'db>,
    ) -> Result<(), String> {
        use hir::hir_ty::infer::Infer;

        // Get the pointer variable
        let var_name = ptr_ident.ident;

        // Load the pointer variable
        if let Some(local_info) = self.local_map.get(&var_name) {
            match local_info {
                LocalInfo::Scalar { index, .. } => {
                    // Load the pointer value (i32 address)
                    func.instruction(&Instruction::LocalGet(*index));

                    // For multiple derefs (ptr^^ := value), we need to load intermediate pointers
                    // For all but the last deref, emit load instructions
                    if deref_count > 1 {
                        // Get the type from the path expression
                        let ptr_type = path_expr.infer(self.db);
                        let mut current_type = ptr_type;

                        for _ in 0..(deref_count - 1) {
                            let pointee_repr = self.get_deref_pointee_type(current_type)?;
                            self.emit_load_instruction(func, pointee_repr)?;

                            // Update current_type for next iteration
                            current_type = match current_type.normalize(self.db) {
                                hir::hir_ty::ty::Type::RefTo(spec) => spec.infer(self.db),
                                _ => {
                                    return Err(
                                        "Expected RefTo type for multiple dereference".to_string()
                                    );
                                }
                            };
                        }
                    }

                    // Now we have the final address on the stack
                    // Emit RHS value
                    self.emit_expr(func, rhs)?;

                    // Get the pointee type for the store instruction from the path expression
                    let ptr_type = path_expr.infer(self.db);
                    let store_repr = self.get_final_deref_type(ptr_type, deref_count)?;

                    // Emit store instruction
                    self.emit_store_instruction(func, store_repr)?;
                    Ok(())
                }
                LocalInfo::Pointer {
                    index,
                    pointee_repr,
                } => {
                    // This is a VAR_IN_OUT parameter (already a pointer)
                    func.instruction(&Instruction::LocalGet(*index));

                    // Handle multiple derefs
                    for _ in 0..(deref_count - 1) {
                        self.emit_load_instruction(func, *pointee_repr)?;
                    }

                    // Emit RHS value
                    self.emit_expr(func, rhs)?;

                    // Store through pointer
                    self.emit_store_instruction(func, *pointee_repr)?;
                    Ok(())
                }
                LocalInfo::Memory { .. } => {
                    Err("Cannot dereference memory-resident variable for assignment".to_string())
                }
            }
        } else {
            Err(format!(
                "Pointer variable '{}' not found in scope",
                var_name.text(self.db)
            ))
        }
    }

    /// Get the final type after applying deref_count dereferenceoperations.
    /// For ptr : REF_TO REF_TO INT with deref_count=2, returns WasmRepr for INT.
    fn get_final_deref_type(
        &self,
        ref_type: hir::hir_ty::ty::Type<'db>,
        deref_count: u16,
    ) -> Result<WasmRepr, String> {
        use hir::hir_ty::infer::Infer;
        use hir::hir_ty::ty::Type;

        let mut current_type = ref_type.normalize(self.db);

        for _ in 0..deref_count {
            match current_type {
                Type::RefTo(spec) => {
                    current_type = spec.infer(self.db).normalize(self.db);
                }
                _ => {
                    return Err(format!(
                        "Cannot dereference non-pointer type: {:?}",
                        current_type
                    ));
                }
            }
        }

        WasmRepr::from_type(self.db, current_type).map_err(|e| {
            format!(
                "Failed to get WASM representation for dereferenced type: {}",
                e
            )
        })
    }

    /// Emit code to assign to a path expression (array element or struct field).
    fn emit_path_assignment(
        &self,
        func: &mut wasm_encoder::Function,
        path_expr: hir::hir_def::expressions::expression::PathExpr<'db>,
        rhs: hir::hir_def::expressions::expression::Expr<'db>,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::expression::PathExprKind;
        use hir::hir_ty::infer::Infer;

        match path_expr.expr(self.db) {
            PathExprKind::Index(index_expr) => {
                // For array assignment: arr[i] := value
                // 1. Calculate address of arr[i]
                // 2. Emit RHS value
                // 3. Store to that address

                // Get base array variable
                let base_ident = index_expr.path.ident(self.db).ident;
                let local_info = self
                    .local_map
                    .get(&base_ident)
                    .ok_or_else(|| format!("Undefined array variable: {:?}", base_ident))?;

                let base_address = match local_info {
                    LocalInfo::Memory { address, .. } => *address,
                    LocalInfo::Pointer { index, .. } => {
                        func.instruction(&Instruction::LocalGet(*index));
                        return Err(
                            "VAR_IN_OUT array assignment not fully implemented yet".to_string()
                        );
                    }
                    LocalInfo::Scalar { .. } => {
                        return Err(format!(
                            "Cannot index into scalar variable: {:?}",
                            base_ident
                        ));
                    }
                };

                // Get array type and element info (same logic as in emit_array_index)
                let array_type = index_expr.path.infer(self.db).normalize(self.db);
                let array_spec = match array_type {
                    hir::hir_ty::ty::Type::Array(arr) => arr,
                    _ => return Err(format!("Expected array type, got: {:?}", array_type)),
                };

                let element_type = array_spec.of_type(self.db).infer(self.db);
                let element_repr = WasmRepr::from_type(self.db, element_type)
                    .map_err(|e| format!("Failed to get element representation: {}", e))?;
                let element_size = element_repr.size_bytes();

                // Calculate address (emit address calculation code)
                let subranges = array_spec.subranges(self.db);
                let indices = &index_expr.index;

                if indices.len() != subranges.len() {
                    return Err(format!(
                        "Index dimension mismatch: expected {}, got {}",
                        subranges.len(),
                        indices.len()
                    ));
                }

                let mut dim_sizes = Vec::new();
                for (start_expr, end_expr) in subranges {
                    let start = self.extract_const_integer(start_expr)?;
                    let end = self.extract_const_integer(end_expr)?;
                    let size = (end - start + 1).max(0);
                    dim_sizes.push((start, size));
                }

                // Calculate linear offset
                func.instruction(&Instruction::I32Const(0));

                for (dim_idx, index) in indices.iter().enumerate() {
                    let (start, _size) = dim_sizes[dim_idx];

                    self.emit_expr(func, *index)?;
                    let index_type = index.infer(self.db).normalize(self.db);
                    if let hir::hir_ty::ty::Type::Elementary(spec) = index_type
                        && is_64bit(spec)
                    {
                        func.instruction(&Instruction::I32WrapI64);
                    }

                    func.instruction(&Instruction::I32Const(start));
                    func.instruction(&Instruction::I32Sub);

                    let mut multiplier = 1;
                    for j in (dim_idx + 1)..dim_sizes.len() {
                        multiplier *= dim_sizes[j].1;
                    }
                    if multiplier > 1 {
                        func.instruction(&Instruction::I32Const(multiplier));
                        func.instruction(&Instruction::I32Mul);
                    }

                    func.instruction(&Instruction::I32Add);
                }

                // Multiply by element size
                if element_size > 1 {
                    func.instruction(&Instruction::I32Const(element_size as i32));
                    func.instruction(&Instruction::I32Mul);
                }

                // Add base address - address is now on stack
                func.instruction(&Instruction::I32Const(base_address as i32));
                func.instruction(&Instruction::I32Add);

                // Emit RHS value
                self.emit_expr(func, rhs)?;

                // Store to the calculated address
                self.emit_store_instruction(func, element_repr)?;

                Ok(())
            }
            PathExprKind::Field(field_expr) => {
                // For struct field assignment: myStruct.fieldA := value
                // 1. Calculate address of the field
                // 2. Emit RHS value
                // 3. Store to that address

                use hir::hir_def::expressions::expression::VarAccess;

                // Calculate the offset by recursively walking the path
                let (base_ident, total_offset) =
                    self.calculate_field_offset(&field_expr.path, &field_expr.var)?;

                let local_info = self
                    .local_map
                    .get(&base_ident)
                    .ok_or_else(|| format!("Undefined struct variable: {:?}", base_ident))?;

                // Get base address
                let base_address = match local_info {
                    LocalInfo::Memory { address, .. } => *address,
                    LocalInfo::Pointer { index, .. } => {
                        func.instruction(&Instruction::LocalGet(*index));
                        return Err(
                            "VAR_IN_OUT struct assignment not fully implemented yet".to_string()
                        );
                    }
                    LocalInfo::Scalar { .. } => {
                        return Err(format!(
                            "Cannot access field of scalar variable: {:?}",
                            base_ident
                        ));
                    }
                };

                // Calculate final address (on stack)
                func.instruction(&Instruction::I32Const((base_address + total_offset) as i32));

                // Emit RHS value
                self.emit_expr(func, rhs)?;

                // Get the field type to determine store instruction
                let struct_type = field_expr.path.infer(self.db).normalize(self.db);
                let field_name = match &field_expr.var {
                    VarAccess::Simple(simple) => simple.ident,
                    VarAccess::Deref(deref, _) => deref.ident,
                };

                let field_type = match struct_type {
                    hir::hir_ty::ty::Type::Struct(s) => {
                        // Find the field and get its type
                        s.elements(self.db)
                            .iter()
                            .find(|elem| elem.name(self.db) == field_name)
                            .map(|elem| elem.spec(self.db).infer(self.db))
                            .ok_or_else(|| format!("Field {:?} not found in struct", field_name))?
                    }
                    _ => return Err(format!("Expected struct type, got: {:?}", struct_type)),
                };

                let field_repr = WasmRepr::from_type(self.db, field_type)
                    .map_err(|e| format!("Failed to get field representation: {}", e))?;

                // Store to the field
                self.emit_store_instruction(func, field_repr)?;

                Ok(())
            }
            PathExprKind::VarAccess(_) => {
                Err("Unexpected VarAccess in PathExpr for assignment".to_string())
            }
        }
    }

    /// Emit code to calculate address and load an array element.
    fn emit_array_index(
        &self,
        func: &mut wasm_encoder::Function,
        index_expr: &hir::hir_def::expressions::expression::IndexExpr<'db>,
    ) -> Result<(), String> {
        use hir::hir_ty::infer::Infer;

        // Get the base array variable name
        let base_ident = index_expr.path.ident(self.db).ident;
        let local_info = self
            .local_map
            .get(&base_ident)
            .ok_or_else(|| format!("Undefined array variable: {:?}", base_ident))?;

        // Get base address from local_info
        let base_address = match local_info {
            LocalInfo::Memory { address, .. } => *address,
            LocalInfo::Pointer { index, .. } => {
                // For VAR_IN_OUT arrays, the base address is in a local
                func.instruction(&Instruction::LocalGet(*index));
                // Address is now on the stack
                return Err("VAR_IN_OUT array access not fully implemented yet".to_string());
            }
            LocalInfo::Scalar { .. } => {
                return Err(format!(
                    "Cannot index into scalar variable: {:?}",
                    base_ident
                ));
            }
        };

        // Get the array type to determine element size and dimensions
        // We need to infer the type of the path expression
        let array_type = index_expr.path.infer(self.db).normalize(self.db);

        // Extract array spec from Type::Array
        let array_spec = match array_type {
            hir::hir_ty::ty::Type::Array(arr) => arr,
            _ => return Err(format!("Expected array type, got: {:?}", array_type)),
        };

        // Get element type and size
        let element_type = array_spec.of_type(self.db).infer(self.db);
        let element_repr = WasmRepr::from_type(self.db, element_type)
            .map_err(|e| format!("Failed to get element representation: {}", e))?;
        let element_size = element_repr.size_bytes();

        // Calculate linear offset from multi-dimensional indices
        // Formula: offset = ((i0 - start0) * dim1_size * dim2_size * ...) + ((i1 - start1) * dim2_size * ...) + ...

        let subranges = array_spec.subranges(self.db);
        let indices = &index_expr.index;

        if indices.len() != subranges.len() {
            return Err(format!(
                "Index dimension mismatch: expected {}, got {}",
                subranges.len(),
                indices.len()
            ));
        }

        // Calculate dimension sizes
        let mut dim_sizes = Vec::new();
        for (start_expr, end_expr) in subranges {
            let start = self.extract_const_integer(start_expr)?;
            let end = self.extract_const_integer(end_expr)?;
            let size = (end - start + 1).max(0);
            dim_sizes.push((start, size));
        }

        // Emit calculation for linear offset
        // Start with 0
        func.instruction(&Instruction::I32Const(0));

        for (dim_idx, index) in indices.iter().enumerate() {
            let (start, _size) = dim_sizes[dim_idx];

            // Calculate (index - start)
            self.emit_expr(func, *index)?;
            // Cast index to i32 if needed
            let index_type = index.infer(self.db).normalize(self.db);
            if let hir::hir_ty::ty::Type::Elementary(spec) = index_type
                && is_64bit(spec)
            {
                // Need to cast from i64 to i32
                func.instruction(&Instruction::I32WrapI64);
            }

            func.instruction(&Instruction::I32Const(start));
            func.instruction(&Instruction::I32Sub);

            // Multiply by product of remaining dimension sizes
            let mut multiplier = 1;
            for j in (dim_idx + 1)..dim_sizes.len() {
                multiplier *= dim_sizes[j].1;
            }
            if multiplier > 1 {
                func.instruction(&Instruction::I32Const(multiplier));
                func.instruction(&Instruction::I32Mul);
            }

            // Add to running total
            func.instruction(&Instruction::I32Add);
        }

        // Multiply by element size
        if element_size > 1 {
            func.instruction(&Instruction::I32Const(element_size as i32));
            func.instruction(&Instruction::I32Mul);
        }

        // Add base address
        func.instruction(&Instruction::I32Const(base_address as i32));
        func.instruction(&Instruction::I32Add);

        // Now we have the element address on the stack
        // Load the element based on its type
        self.emit_load_instruction(func, element_repr)?;

        Ok(())
    }

    /// Emit code to access a struct field.
    fn emit_struct_field(
        &self,
        func: &mut wasm_encoder::Function,
        field_expr: &hir::hir_def::expressions::expression::FieldExpr<'db>,
    ) -> Result<(), String> {
        use hir::hir_def::expressions::expression::VarAccess;
        use hir::hir_ty::infer::Infer;

        // Calculate the offset by recursively walking the path
        let (base_ident, total_offset) =
            self.calculate_field_offset(&field_expr.path, &field_expr.var)?;

        let local_info = self
            .local_map
            .get(&base_ident)
            .ok_or_else(|| format!("Undefined struct variable: {:?}", base_ident))?;

        // Get base address
        let base_address = match local_info {
            LocalInfo::Memory { address, .. } => *address,
            LocalInfo::Pointer { index, .. } => {
                // For VAR_IN_OUT structs, get pointer from local
                func.instruction(&Instruction::LocalGet(*index));
                return Err("VAR_IN_OUT struct access not fully implemented yet".to_string());
            }
            LocalInfo::Scalar { .. } => {
                return Err(format!(
                    "Cannot access field of scalar variable: {:?}",
                    base_ident
                ));
            }
        };

        // Calculate final address
        func.instruction(&Instruction::I32Const((base_address + total_offset) as i32));

        // Get the field type to determine load instruction
        // We need to get the struct type and find the field within it
        let struct_type = field_expr.path.infer(self.db).normalize(self.db);
        let field_name = match &field_expr.var {
            VarAccess::Simple(simple) => simple.ident,
            VarAccess::Deref(deref, _) => deref.ident,
        };

        let field_type = match struct_type {
            hir::hir_ty::ty::Type::Struct(s) => {
                // Find the field and get its type
                s.elements(self.db)
                    .iter()
                    .find(|elem| elem.name(self.db) == field_name)
                    .map(|elem| elem.spec(self.db).infer(self.db))
                    .ok_or_else(|| format!("Field {:?} not found in struct", field_name))?
            }
            _ => return Err(format!("Expected struct type, got: {:?}", struct_type)),
        };

        let field_repr = WasmRepr::from_type(self.db, field_type)
            .map_err(|e| format!("Failed to get field representation: {}", e))?;

        // Load the field value
        self.emit_load_instruction(func, field_repr)?;

        Ok(())
    }

    /// Recursively calculate the field offset for a struct field access.
    /// Returns (base_ident, total_offset).
    fn calculate_field_offset(
        &self,
        path: &hir::hir_def::expressions::expression::PathExpr<'db>,
        field_var: &hir::hir_def::expressions::expression::VarAccess<'db>,
    ) -> Result<(Ident, u32), String> {
        use hir::hir_def::expressions::expression::{PathExprKind, VarAccess};
        use hir::hir_ty::infer::Infer;

        // First, get the base and offset for the path
        let (base_ident, path_offset) = match path.expr(self.db) {
            PathExprKind::VarAccess(var_access) => {
                // Base case: path is just a variable access
                let base_ident = match var_access {
                    VarAccess::Simple(simple) => simple.ident,
                    VarAccess::Deref(deref, _) => deref.ident,
                };
                (base_ident, 0u32)
            }
            PathExprKind::Field(nested_field) => {
                // Recursive case: path contains nested field access
                self.calculate_field_offset(&nested_field.path, &nested_field.var)?
            }
            _ => return Err("Unexpected path expression kind in struct field access".to_string()),
        };

        // Now add the offset for the final field (field_var)
        let struct_type = path.infer(self.db).normalize(self.db);

        match struct_type {
            hir::hir_ty::ty::Type::Struct(s) => {
                // Get field name
                let field_name = match field_var {
                    VarAccess::Simple(simple) => simple.ident,
                    VarAccess::Deref(deref, _) => deref.ident,
                };

                // Calculate field offsets for this struct
                let field_offsets = calculate_field_offsets(self.db, s)
                    .map_err(|e| format!("Failed to calculate field offsets: {}", e))?;

                // Find the offset for this field
                let field_offset = field_offsets
                    .iter()
                    .find(|(name, _)| *name == field_name)
                    .map(|(_, offset)| *offset)
                    .ok_or_else(|| format!("Field {:?} not found in struct", field_name))?;

                Ok((base_ident, path_offset + field_offset))
            }
            _ => Err(format!(
                "Expected struct type for field access, got: {:?}",
                struct_type
            )),
        }
    }

    /// Emit a load instruction based on the type representation.
    fn emit_load_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        repr: WasmRepr,
    ) -> Result<(), String> {
        use wasm_encoder::MemArg;

        match repr {
            WasmRepr::Scalar(val_type) => {
                // MemArg alignment is log2 of the actual alignment
                let align = (repr.alignment() as f32).log2() as u32;

                let mem_arg = MemArg {
                    offset: 0,
                    align,
                    memory_index: 0,
                };

                match val_type {
                    wasm_encoder::ValType::I32 => {
                        func.instruction(&Instruction::I32Load(mem_arg));
                    }
                    wasm_encoder::ValType::I64 => {
                        func.instruction(&Instruction::I64Load(mem_arg));
                    }
                    wasm_encoder::ValType::F32 => {
                        func.instruction(&Instruction::F32Load(mem_arg));
                    }
                    wasm_encoder::ValType::F64 => {
                        func.instruction(&Instruction::F64Load(mem_arg));
                    }
                    _ => return Err(format!("Unsupported value type for load: {:?}", val_type)),
                }
                Ok(())
            }
            WasmRepr::Memory { .. } => {
                // For memory types (structs), the address itself is the value
                // Don't load, just leave the address on the stack
                Ok(())
            }
        }
    }

    /// Emit a store instruction based on the type representation.
    /// Stack before: [address, value]
    /// Stack after: []
    fn emit_store_instruction(
        &self,
        func: &mut wasm_encoder::Function,
        repr: WasmRepr,
    ) -> Result<(), String> {
        use wasm_encoder::MemArg;

        match repr {
            WasmRepr::Scalar(val_type) => {
                // MemArg alignment is log2 of the actual alignment
                let align = (repr.alignment() as f32).log2() as u32;

                let mem_arg = MemArg {
                    offset: 0,
                    align,
                    memory_index: 0,
                };

                match val_type {
                    wasm_encoder::ValType::I32 => {
                        func.instruction(&Instruction::I32Store(mem_arg));
                    }
                    wasm_encoder::ValType::I64 => {
                        func.instruction(&Instruction::I64Store(mem_arg));
                    }
                    wasm_encoder::ValType::F32 => {
                        func.instruction(&Instruction::F32Store(mem_arg));
                    }
                    wasm_encoder::ValType::F64 => {
                        func.instruction(&Instruction::F64Store(mem_arg));
                    }
                    _ => return Err(format!("Unsupported value type for store: {:?}", val_type)),
                }
                Ok(())
            }
            WasmRepr::Memory { .. } => {
                // For memory types (structs), can't store entire struct at once
                Err("Direct store of memory-resident types not supported".to_string())
            }
        }
    }

    /// Extract a constant integer from an expression (for array bounds).
    fn extract_const_integer(
        &self,
        expr: hir::hir_def::expressions::expression::Expr<'db>,
    ) -> Result<i32, String> {
        use hir::hir_def::expressions::expression::{Elementary, ExprKind, PrimaryExpr};

        match expr.expr(self.db) {
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(int))) => int
                .as_i32(self.db)
                .map_err(|e| format!("Failed to parse array bound: {}", e)),
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::Int(int))) => int
                .as_i32(self.db)
                .map_err(|e| format!("Failed to parse array bound: {}", e)),
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::DInt(int))) => int
                .as_i32(self.db)
                .map_err(|e| format!("Failed to parse array bound: {}", e)),
            _ => Err("Array bounds must be constant integers".to_string()),
        }
    }

    /// Get the pointee type from a RefTo type.
    /// For REF_TO INT, returns INT type.
    fn get_deref_pointee_type(
        &self,
        ref_type: hir::hir_ty::ty::Type<'db>,
    ) -> Result<WasmRepr, String> {
        use hir::hir_ty::infer::Infer;
        use hir::hir_ty::ty::Type;

        // Normalize the type first
        let normalized = ref_type.normalize(self.db);

        match normalized {
            Type::RefTo(spec) => {
                // Infer the pointee type from the spec
                let pointee_type = spec.infer(self.db);
                WasmRepr::from_type(self.db, pointee_type).map_err(|e| {
                    format!("Failed to get WASM representation for pointee type: {}", e)
                })
            }
            _ => Err(format!(
                "Expected REF_TO type for dereference, got: {:?}",
                normalized
            )),
        }
    }

    /// Emit a load instruction for dereferencing a pointer.
    /// Stack before: [address]
    /// Stack after: [value]
    fn emit_load_from_pointer(
        &self,
        func: &mut wasm_encoder::Function,
        ty: hir::hir_ty::ty::Type<'db>,
    ) -> Result<(), String> {
        // Convert type to WASM representation
        let repr = WasmRepr::from_type(self.db, ty).map_err(|e| {
            format!(
                "Failed to get type representation for pointer dereference: {}",
                e
            )
        })?;

        // Emit the appropriate load instruction
        self.emit_load_instruction(func, repr)
    }

    /// Emit a load for an FB instance variable accessed via 'this' pointer.
    /// Stack before: []
    /// Stack after: [value]
    fn emit_instance_var_load(
        &self,
        func: &mut wasm_encoder::Function,
        var_name: hir::hir_def::interned::identifier::Ident,
    ) -> Result<(), String> {
        use hir::hir_ty::infer::Infer;

        // Check if we're in a method with instance context (FB or Class)
        let (this_local, instance) = match (self.this_local, self.instance) {
            (Some(this_local), Some(instance)) => (this_local, instance),
            _ => return Err(format!("Undefined variable: {:?}", var_name)),
        };

        // Find the variable in the instance's variables
        let instance_var = instance
            .variables(self.db)
            .iter()
            .find(|v| v.name(self.db) == var_name)
            .ok_or_else(|| format!("Instance variable {:?} not found", var_name))?;

        // Get variable offset in the instance layout
        let field_offsets = calculate_instance_field_offsets(self.db, instance)
            .map_err(|e| format!("Failed to calculate instance field offsets: {}", e))?;

        let offset = field_offsets
            .iter()
            .find(|(name, _)| *name == var_name)
            .map(|(_, offset)| *offset)
            .ok_or_else(|| format!("Field offset not found for {:?}", var_name))?;

        // Load 'this' pointer
        func.instruction(&Instruction::LocalGet(this_local));

        // If offset is non-zero, add it
        if offset > 0 {
            func.instruction(&Instruction::I32Const(offset as i32));
            func.instruction(&Instruction::I32Add);
        }

        // Get variable type and emit load instruction
        let var_type = instance_var.spec(self.db).infer(self.db);
        let var_repr = WasmRepr::from_type(self.db, var_type)
            .map_err(|e| format!("Failed to get type representation: {}", e))?;

        self.emit_load_instruction(func, var_repr)
    }

    /// Emit a store for an FB instance variable accessed via 'this' pointer.
    /// Stack before: []
    /// Stack after: []
    fn emit_instance_var_store(
        &self,
        func: &mut wasm_encoder::Function,
        var_name: hir::hir_def::interned::identifier::Ident,
        value_expr: hir::hir_def::expressions::expression::Expr<'db>,
        _value_type: hir::hir_ty::ty::Type<'db>,
    ) -> Result<(), String> {
        use hir::hir_ty::infer::Infer;

        // Check if we're in a method with instance context (FB or Class)
        let (this_local, instance) = match (self.this_local, self.instance) {
            (Some(this_local), Some(instance)) => (this_local, instance),
            _ => return Err(format!("Undefined variable: {:?}", var_name)),
        };

        // Find the variable in the instance's variables
        let instance_var = instance
            .variables(self.db)
            .iter()
            .find(|v| v.name(self.db) == var_name)
            .ok_or_else(|| format!("Instance variable {:?} not found", var_name))?;

        // Get variable offset in the instance layout
        let field_offsets = calculate_instance_field_offsets(self.db, instance)
            .map_err(|e| format!("Failed to calculate instance field offsets: {}", e))?;

        let offset = field_offsets
            .iter()
            .find(|(name, _)| *name == var_name)
            .map(|(_, offset)| *offset)
            .ok_or_else(|| format!("Field offset not found for {:?}", var_name))?;

        // Stack order for store: [address, value]

        // 1. Load 'this' pointer
        func.instruction(&Instruction::LocalGet(this_local));

        // 2. If offset is non-zero, add it
        if offset > 0 {
            func.instruction(&Instruction::I32Const(offset as i32));
            func.instruction(&Instruction::I32Add);
        }

        // 3. Emit RHS value
        self.emit_expr(func, value_expr)?;

        // 4. Get variable type and emit store instruction
        let var_type = instance_var.spec(self.db).infer(self.db);
        let var_repr = WasmRepr::from_type(self.db, var_type)
            .map_err(|e| format!("Failed to get type representation: {}", e))?;

        self.emit_store_instruction(func, var_repr)
    }
}
