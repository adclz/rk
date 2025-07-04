use std::ops::Deref;

use crate::{ident::Ident, solver::NamespacePath};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::File;
use bitflags::bitflags;

#[salsa::tracked(debug)]
pub struct Expr<'db> {
    span: auto_lsp::tree_sitter::Range,

    #[return_ref]
    pub expr: ExprKind<'db>,
}

impl<'db> Expr<'db> {
    pub fn new_literal(
        db: &'db dyn BaseDatabase,
        span: auto_lsp::tree_sitter::Range,
        literal: Literal,
    ) -> Expr<'db> {
        Expr::new(
            db,
            span,
            ExprKind::PrimaryExpr {
                expr: PrimaryExpr::Literal(literal),
            },
        )
    }

    pub fn new_target(
        db: &'db dyn BaseDatabase,
        file: File,
        fq_name: &ast::generated::FqName,
    ) -> anyhow::Result<Expr<'db>> {
        Ok(Expr::new(
            db,
            *fq_name.get_range(),
            ExprKind::PrimaryExpr {
                expr: PrimaryExpr::Target {
                    target: Ident::from_node(db, file, fq_name.target.deref())?,
                    path: NamespacePath::from((
                        db,
                        fq_name
                            .fragment
                            .iter()
                            .map(|f| Ident::from_node(db, file, f.deref()))
                            .collect::<anyhow::Result<Vec<_>>>()?,
                    )),
                },
            },
        ))
    }

    pub fn new_enum_value(
        db: &'db dyn BaseDatabase,
        span: auto_lsp::tree_sitter::Range,
        value: Ident,
    ) -> Expr<'db> {
        Expr::new(
            db,
            span,
            ExprKind::PrimaryExpr {
                expr: PrimaryExpr::EnumValue { value },
            },
        )
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct Operator: u16 {
        const Plus = 1 << 0;
        const Minus = 1 << 1;
        const Div = 1 << 2;
        const Mul = 1 << 3;
        const Mod = 1 << 4;
        const Power = 1 << 5;
        const Not = 1 << 6;

        const And = 1 << 7;
        const Or = 1 << 8;
        const Xor = 1 << 9;

        const Eq = 1 << 10;
        const Ne = 1 << 11;
        const Lt = 1 << 12;
        const Gt = 1 << 13;
        const Le = 1 << 14;
        const Ge = 1 << 15;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExprKind<'db> {
    PrimaryExpr {
        expr: PrimaryExpr<'db>,
    },
    BooleanOperator {
        left: Expr<'db>,
        operator: Operator,
        right: Expr<'db>,
    },
    ComparisonOperator {
        left: Expr<'db>,
        operator: Operator,
        right: Expr<'db>,
    },
    AddOperator {
        left: Expr<'db>,
        operator: Operator,
        right: Expr<'db>,
    },
    MultOperator {
        left: Expr<'db>,
        operator: Operator,
        right: Expr<'db>,
    },
    PowerOperator {
        left: Expr<'db>,
        right: Expr<'db>,
    },
    UnaryOperator {
        expr: Expr<'db>,
        operator: Operator,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PrimaryExpr<'db> {
    Literal(Literal),
    // Path --> Target
    Target {
        path: NamespacePath,
        target: Ident,
    },
    EnumValue {
        value: Ident,
    },
    VariableAccess, // todo
    FuncCall {
        expr: Expr<'db>,
        params: Vec<ParamAssign<'db>>,
    },
    RefValue, // todo
    ParenthesizedExpr {
        expr: Expr<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ParamAssign<'db> {
    ParamAssignInput {
        param: Option<Ident>,
        value: Expr<'db>,
    },
    ParamAssignOutput {
        not: bool,
        param: Ident,
        variable: Variable<'db>,
    },
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct AccessOperator: u16 {
        const I = 1 << 0;
        const Q = 1 << 1;
        const M = 1 << 2;

        const X = 1 << 3;
        const B = 1 << 4;
        const W = 1 << 5;
        const D = 1 << 6;
        const L = 1 << 7;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Variable<'db> {
    Direct {
        kind: AccessOperator,
        size: Option<AccessOperator>,
        offset: Vec<Ident>,
    },
    Symbolic {
        this: bool,
        kind: SymbolicVariableKind<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SymbolicVariableKind<'db> {
    VariableAccess {
        access: VariableAccess,
    },
    MultiElemVar {
        base: VariableAccess,
        elements: Vec<MultiElemVarElement<'db>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MultiElemVarElement<'db> {
    Subscript { expr: Expr<'db> },             // []
    StructVariable { access: VariableAccess }, // .
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VariableAccess {
    Simple(Ident),
    Deref(Ident), // ^
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Literal {
    // Any numeric type (non floating point)
    AnyNumeric(Numeric),

    // Signed
    SInt(Numeric),
    Int(Numeric),
    DInt(Numeric),
    LInt(Numeric),

    // Unsigned
    USInt(Numeric),
    UInt(Numeric),
    UDInt(Numeric),
    ULInt(Numeric),

    // Bit string
    Byte(Numeric),
    Word(Numeric),
    DWord(Numeric),
    LWord(Numeric),

    Real(Ident),
    LReal(Ident),

    Bool(Ident),

    Char(Ident),
    DChar(Ident),

    Date(Ident),
    LDate(Ident),
    Tod(Ident),
    LTod(Ident),
    Time(Ident),
    LTime(Ident),
    DateTime(Ident),
    LDateTime(Ident),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Numeric {
    Binary(Ident),
    Hex(Ident),
    Octal(Ident),
    Signed(Ident),
}
