use hir::hir_def::interned::identifier::Ident;

use crate::types::{MirElementary, MirType};

/// An expression in MIR — fully resolved, no type inference needed.
#[derive(Debug, Clone)]
pub enum MirExpr {
    /// A constant literal value.
    Constant(MirConstant),

    /// Read from a place (variable, field, array element, deref).
    Load(MirPlace, MirType),

    /// Binary operation. Both operands and the result have the same type
    /// (casts are inserted explicitly during lowering).
    BinOp {
        op: MirBinOp,
        lhs: Box<MirExpr>,
        rhs: Box<MirExpr>,
        /// The concrete type at which this operation executes.
        ty: MirElementary,
    },

    /// Unary operation.
    UnaryOp {
        op: MirUnaryOp,
        expr: Box<MirExpr>,
        ty: MirElementary,
    },

    /// Explicit type cast (inserted by lowering for implicit casts).
    Cast {
        expr: Box<MirExpr>,
        from: MirElementary,
        to: MirElementary,
    },

    /// Function or method call that produces a value.
    Call(MirCall),

    /// Take the address of a place (REF operator).
    AddrOf(MirPlace),

    /// String literal reference.
    StringLiteral {
        /// Index into MirModule::string_literals.
        id: u32,
        /// Pre-computed offset in the data section.
        offset: u32,
        /// Length in bytes.
        len: u32,
    },
}

/// A call expression (also usable as statement via MirStmt::Call).
#[derive(Debug, Clone)]
pub struct MirCall {
    /// Callee name (salsa-interned).
    pub callee: Ident,
    /// Pre-resolved function index.
    pub callee_index: u32,
    /// Arguments in parameter order.
    pub args: Vec<MirCallArg>,
    /// Return type.
    pub return_type: MirType,
}

#[derive(Debug, Clone)]
pub struct MirCallArg {
    pub value: MirExpr,
    pub kind: MirArgKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirArgKind {
    /// Pass by value.
    ByValue,
    /// Pass by reference (address of the argument).
    ByRef,
}

/// A place designates a memory location that can be read or assigned to.
#[derive(Debug, Clone)]
pub enum MirPlace {
    /// A named local variable or parameter.
    Local(Ident),

    /// Field access: base.field_name, with the field offset pre-computed.
    Field {
        base: Box<MirPlace>,
        field_name: Ident,
        field_offset: u32,
        field_type: MirType,
    },

    /// Array index: base[index], with element size pre-computed.
    Index {
        base: Box<MirPlace>,
        index: Box<MirExpr>,
        element_size: u32,
        element_type: MirType,
        /// Lower bound of the array dimension (for offset calculation).
        lower_bound: i64,
    },

    /// Pointer dereference: base^
    Deref {
        base: Box<MirPlace>,
        pointee_type: MirType,
    },

    /// Instance variable access through 'this' pointer (for methods).
    ThisField {
        field_name: Ident,
        field_offset: u32,
        field_type: MirType,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBinOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Power,
    // Boolean / Bitwise
    And,
    Or,
    Xor,
    // Comparison (result is always Bool/i32)
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirUnaryOp {
    /// Arithmetic negation.
    Neg,
    /// Boolean/bitwise not.
    Not,
}

#[derive(Debug, Clone)]
pub enum MirConstant {
    Bool(bool),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    /// Null pointer.
    Null,
}
