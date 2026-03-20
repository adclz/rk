use crate::expr::{MirCall, MirConstant, MirExpr, MirPlace};
use crate::types::MirElementary;
use hir::hir_def::interned::identifier::Ident;

/// A statement in MIR.
#[derive(Debug, Clone)]
pub enum MirStmt {
    /// Simple assignment: place = expr.
    Assign {
        target: MirPlace,
        value: MirExpr,
    },

    /// Function/method call as statement (result discarded if any).
    Call(MirCall),

    /// Return from function.
    Return,

    /// Structured if/elsif/else.
    If {
        condition: MirExpr,
        then_body: Vec<MirStmt>,
        else_ifs: Vec<(MirExpr, Vec<MirStmt>)>,
        else_body: Option<Vec<MirStmt>>,
    },

    /// Case statement.
    Case {
        /// The case selector expression. Always integer-typed.
        selector: MirExpr,
        arms: Vec<MirCaseArm>,
        else_body: Option<Vec<MirStmt>>,
    },

    /// For loop.
    For {
        control_var: Ident,
        control_type: MirElementary,
        start: MirExpr,
        end: MirExpr,
        /// Always present; default 1 is materialized during lowering.
        step: MirExpr,
        body: Vec<MirStmt>,
    },

    /// While loop.
    While {
        condition: MirExpr,
        body: Vec<MirStmt>,
    },

    /// Repeat-until loop.
    Repeat {
        condition: MirExpr,
        body: Vec<MirStmt>,
    },

    /// Exit (break from innermost loop).
    Exit,

    /// Continue (jump to start of innermost loop).
    Continue,

    /// Direct memory store for flat initialization sequences.
    MemStore {
        offset: u32,
        value: MirConstant,
    },

    /// Debug trap point (optional, only when debug mode enabled).
    DebugTrap {
        trap_id: u32,
        location: MirSourceLocation,
    },
}

#[derive(Debug, Clone)]
pub struct MirCaseArm {
    /// Match patterns: single values or ranges.
    pub patterns: Vec<MirCasePattern>,
    pub body: Vec<MirStmt>,
}

#[derive(Debug, Clone)]
pub enum MirCasePattern {
    /// Single constant value.
    Value(MirConstant),
    /// Range: lower..=upper.
    Range {
        lower: MirConstant,
        upper: MirConstant,
    },
}

#[derive(Debug, Clone)]
pub struct MirSourceLocation {
    pub file_id: u32,
    pub line: u32,
    pub column: u32,
}
