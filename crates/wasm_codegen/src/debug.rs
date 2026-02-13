//! Debug instrumentation support for WASM code generation.

/// Debug configuration for code generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeGenConfig {
    pub debug_mode: DebugMode,
    pub trap_on_overflow: bool,
    pub trap_on_div_zero: bool,
}

impl Default for CodeGenConfig {
    fn default() -> Self {
        Self {
            debug_mode: DebugMode::None,
            trap_on_overflow: false,
            trap_on_div_zero: false,
        }
    }
}

/// Level of debug instrumentation to inject.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugMode {
    /// No debug instrumentation.
    None,
    /// Inject traps before each statement.
    StatementLevel,
    /// Inject traps before each expression.
    ExpressionLevel,
    /// Inject traps before every instruction (very verbose!).
    InstructionLevel,
}

/// Source location information for debug traps.
///
/// Simplified version for initial implementation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceLocation {
    /// File path (for now, just a simple ID or placeholder)
    pub file_id: u32,
    pub line: u32,
    pub column: u32,
}

/// Debug information collected during code generation.
#[derive(Debug, Default)]
pub struct DebugInfo {
    /// Maps trap ID -> source location.
    pub traps: Vec<SourceLocation>,
    /// Maps WASM function index -> IEC function name.
    pub function_names: Vec<(u32, String)>,
}

impl DebugInfo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a trap point and return its ID.
    pub fn add_trap(&mut self, location: SourceLocation) -> u32 {
        let id = self.traps.len() as u32;
        self.traps.push(location);
        id
    }

    /// Add a function name mapping.
    pub fn add_function(&mut self, func_idx: u32, name: String) {
        self.function_names.push((func_idx, name));
    }
}

/// Point at which a trap can be injected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapPoint {
    Statement,
    Expression,
    Instruction,
}
