//! Instruction emitter with optional trap injection.

use wasm_encoder::Instruction;

use crate::debug::{CodeGenConfig, DebugInfo, DebugMode, SourceLocation, TrapPoint};

/// Wraps a wasm_encoder::Function with debug trap injection capability.
pub struct InstructionEmitter<'a> {
    func: &'a mut wasm_encoder::Function,
    config: &'a CodeGenConfig,
    debug_info: &'a mut DebugInfo,
    debug_enabled_global: Option<u32>,
    debug_trap_id_global: Option<u32>,

    // Source tracking
    current_source_location: Option<SourceLocation>,
    last_trap_location: Option<SourceLocation>,
}

impl<'a> InstructionEmitter<'a> {
    pub fn new(
        func: &'a mut wasm_encoder::Function,
        config: &'a CodeGenConfig,
        debug_info: &'a mut DebugInfo,
        debug_enabled_global: Option<u32>,
        debug_trap_id_global: Option<u32>,
    ) -> Self {
        Self {
            func,
            config,
            debug_info,
            debug_enabled_global,
            debug_trap_id_global,
            current_source_location: None,
            last_trap_location: None,
        }
    }

    /// Emit an instruction, optionally with trap injection.
    pub fn emit(&mut self, instruction: Instruction) {
        self.maybe_inject_trap(TrapPoint::Instruction);
        self.func.instruction(&instruction);
    }

    /// Emit multiple instructions atomically (no trap injection between them).
    pub fn emit_block(&mut self, instructions: &[Instruction]) {
        for instr in instructions {
            self.func.instruction(instr);
        }
    }

    /// Get direct access to the underlying function for complex operations.
    pub fn raw(&mut self) -> &mut wasm_encoder::Function {
        self.func
    }

    /// Mark the start of a statement (for statement-level traps).
    pub fn begin_statement(&mut self, location: SourceLocation) {
        self.current_source_location = Some(location.clone());
        self.maybe_inject_trap(TrapPoint::Statement);
    }

    /// Mark the start of an expression (for expression-level traps).
    pub fn begin_expression(&mut self, location: SourceLocation) {
        if self.config.debug_mode == DebugMode::ExpressionLevel
            || self.config.debug_mode == DebugMode::InstructionLevel
        {
            self.current_source_location = Some(location.clone());
            self.maybe_inject_trap(TrapPoint::Expression);
        }
    }

    fn maybe_inject_trap(&mut self, trap_point: TrapPoint) {
        let should_trap = match (self.config.debug_mode, trap_point) {
            (DebugMode::None, _) => false,
            (DebugMode::StatementLevel, TrapPoint::Statement) => true,
            (DebugMode::ExpressionLevel, TrapPoint::Statement | TrapPoint::Expression) => true,
            (DebugMode::InstructionLevel, _) => true,
            _ => false,
        };

        if !should_trap {
            return;
        }

        // Don't inject duplicate traps at the same location
        if self.last_trap_location == self.current_source_location {
            return;
        }

        // Inject trap
        if let Some(loc) = self.current_source_location.clone() {
            self.inject_debug_trap(loc.clone());
            self.last_trap_location = Some(loc);
        }
    }

    fn inject_debug_trap(&mut self, loc: SourceLocation) {
        // Record this trap in debug info
        let trap_id = self.debug_info.add_trap(loc);

        // Get global indices - they must exist if we're in debug mode
        let debug_enabled_global = self
            .debug_enabled_global
            .expect("debug_enabled_global should be set in debug mode");
        let debug_trap_id_global = self
            .debug_trap_id_global
            .expect("debug_trap_id_global should be set in debug mode");

        // Emit: if (debug_enabled) { trap_id = N; unreachable; }
        self.func
            .instruction(&Instruction::GlobalGet(debug_enabled_global));
        self.func
            .instruction(&Instruction::If(wasm_encoder::BlockType::Empty));

        // Store trap ID in global for debugger inspection
        self.func
            .instruction(&Instruction::I32Const(trap_id as i32));
        self.func
            .instruction(&Instruction::GlobalSet(debug_trap_id_global));

        // Trigger trap
        self.func.instruction(&Instruction::Unreachable);
        self.func.instruction(&Instruction::End);
    }
}
