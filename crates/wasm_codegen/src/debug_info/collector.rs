//! Debug information collector.
//!
//! Collects debug information during WASM code generation, including:
//! - Compilation units (source files)
//! - Functions with parameters and local variables
//! - Type information (elementary, struct, array, pointer)
//! - Line number mappings (WASM offset → source location)

use auto_lsp::core::span::Span;
use auto_lsp::default::db::file::File;
use rustc_hash::FxHashMap;

// Type will be used in later phases when converting HIR types to TypeDebugInfo
// use hir::hir_ty::ty::Type;

/// Main debug information collector used during module generation.
///
/// Collects debug info incrementally as functions are generated,
/// then provides it to the DWARF generator for serialization.
#[derive(Debug, Default)]
pub struct DebugInfoCollector {
    /// Debug info organized by compilation unit (source file)
    compilation_units: Vec<CompilationUnitDebugInfo>,

    /// Map from file to compilation unit index
    file_to_cu: FxHashMap<File, usize>,
}

impl DebugInfoCollector {
    /// Create a new debug info collector.
    pub fn new() -> Self {
        Self::default()
    }
 
    /// Register a source file as a compilation unit.
    pub fn register_file(&mut self, file: File, file_path: String) {
        if self.file_to_cu.contains_key(&file) {
            return;
        }

        let cu_index = self.compilation_units.len();
        self.file_to_cu.insert(file, cu_index);
        self.compilation_units.push(CompilationUnitDebugInfo {
            file,
            file_path,
            functions: Vec::new(),
        });
    }

    /// Add function debug info to the appropriate compilation unit.
    pub fn add_function(&mut self, file: File, func_info: FunctionDebugInfo) {
        if let Some(&cu_index) = self.file_to_cu.get(&file) {
            self.compilation_units[cu_index].functions.push(func_info);
        }
    }

    /// Update line mappings for a function by WASM index.
    ///
    /// Phase 6: Add line number mappings collected during code generation.
    pub fn update_function_line_mappings(
        &mut self,
        wasm_index: u32,
        line_mappings: Vec<LineMapping>,
    ) {
        for cu in &mut self.compilation_units {
            for func in &mut cu.functions {
                if func.wasm_index == wasm_index {
                    func.line_mappings = line_mappings;
                    return;
                }
            }
        }
    }

    /// Get a reference to collected compilation units without consuming the collector.
    pub fn compilation_units(&self) -> &[CompilationUnitDebugInfo] {
        &self.compilation_units
    }

    /// Consume the collector and return all collected compilation units.
    pub fn finish(self) -> Vec<CompilationUnitDebugInfo> {
        self.compilation_units
    }
}

/// Debug information for a compilation unit (source file).
#[derive(Debug, Clone)]
pub struct CompilationUnitDebugInfo {
    /// The source file this compilation unit represents
    pub file: File,

    /// Absolute path to the source file
    pub file_path: String,

    /// All functions defined in this compilation unit
    pub functions: Vec<FunctionDebugInfo>,
}

/// Debug information for a function.
#[derive(Debug, Clone)]
pub struct FunctionDebugInfo {
    /// Function name (qualified for methods: "FBName$MethodName")
    pub name: String,

    /// WASM function index
    pub wasm_index: u32,

    /// Source span of the function declaration
    pub decl_span: Span,

    /// Function parameters with debug info
    pub parameters: Vec<VariableDebugInfo>,

    /// Local variables with debug info
    pub locals: Vec<VariableDebugInfo>,

    /// Return type (None for void functions like PROGRAM)
    pub return_type: Option<TypeDebugInfo>,

    /// Line number mappings (WASM offset → source location)
    pub line_mappings: Vec<LineMapping>,
}

/// Debug information for a variable (parameter or local).
#[derive(Debug, Clone)]
pub struct VariableDebugInfo {
    /// Variable name
    pub name: String,

    /// Variable type
    pub var_type: TypeDebugInfo,

    /// Location of the variable in WASM (local index, memory address, or pointer)
    pub location: VariableLocation,

    /// Source span of the variable declaration
    pub decl_span: Span,
}

/// Location of a variable in WASM memory model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableLocation {
    /// WASM local variable (stack-allocated scalar)
    Local(u32),

    /// Linear memory location (for large structs, arrays)
    Memory {
        /// Memory address
        address: u32,
        /// Size in bytes
        size: u32,
    },

    /// Pointer to memory (FB/CLASS instance passed as parameter)
    Pointer {
        /// Local index holding the pointer
        local_index: u32,
        /// Size of the pointed-to data
        size: u32,
    },
}

/// Type information for debug symbols.
#[derive(Debug, Clone)]
pub enum TypeDebugInfo {
    /// Elementary type (INT, REAL, BOOL, etc.)
    Elementary {
        /// IEC type name (e.g., "INT", "REAL")
        name: String,
        /// Size in bytes
        byte_size: u8,
        /// DWARF encoding
        encoding: DwarfEncoding,
    },

    /// Struct type
    Struct {
        /// Struct name
        name: String,
        /// Total size in bytes
        byte_size: u32,
        /// Fields with names, types, and byte offsets
        fields: Vec<StructField>,
    },

    /// Array type
    Array {
        /// Element type
        element_type: Box<TypeDebugInfo>,
        /// Dimensions (for multi-dimensional arrays)
        dimensions: Vec<ArrayDimension>,
        /// Total size in bytes
        byte_size: u32,
    },

    /// Pointer/reference type
    Pointer {
        /// Pointed-to type
        pointee_type: Box<TypeDebugInfo>,
    },

    /// Enum type
    Enum {
        /// Enum name
        name: String,
        /// Underlying integer type size
        byte_size: u8,
        /// Enumerators with names and values
        enumerators: Vec<(String, i64)>,
    },

    /// Void type (for procedures with no return value)
    Void,
}

/// DWARF encoding for elementary types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DwarfEncoding {
    Boolean,
    Signed,
    Unsigned,
    Float,
}

/// Struct field information.
#[derive(Debug, Clone)]
pub struct StructField {
    /// Field name
    pub name: String,
    /// Field type
    pub field_type: TypeDebugInfo,
    /// Byte offset from struct start
    pub offset: u32,
}

/// Array dimension information.
#[derive(Debug, Clone)]
pub struct ArrayDimension {
    /// Lower bound (inclusive)
    pub lower_bound: i64,
    /// Upper bound (inclusive)
    pub upper_bound: i64,
}

/// Mapping from WASM instruction offset to source location.
#[derive(Debug, Clone)]
pub struct LineMapping {
    /// WASM instruction offset (byte offset in code section)
    pub wasm_offset: u32,

    /// Source span for this instruction
    pub source_span: Span,
}

impl TypeDebugInfo {
    /// Get the byte size of this type.
    pub fn byte_size(&self) -> u32 {
        match self {
            TypeDebugInfo::Elementary { byte_size, .. } => *byte_size as u32,
            TypeDebugInfo::Struct { byte_size, .. } => *byte_size,
            TypeDebugInfo::Array { byte_size, .. } => *byte_size,
            TypeDebugInfo::Pointer { .. } => 4, // WASM32 pointer
            TypeDebugInfo::Enum { byte_size, .. } => *byte_size as u32,
            TypeDebugInfo::Void => 0,
        }
    }
}
