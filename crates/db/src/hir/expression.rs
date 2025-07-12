use std::ops::Deref;
use std::path::Display;

use crate::solver::fq_name::FqName;
use crate::to_proto::{IterToProto, ToProto};
use crate::{ident::Ident, solver::namespace::NamespacePath};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::file::File;
use bitflags::bitflags;

#[salsa::tracked(debug)]
pub struct Expr<'db> {
    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub expr: ExprKind<'db>,
}

impl<'db> ToProto<'db> for Expr<'db> {
    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }
}

fn self_iter<'db>(s: &'db impl ToProto<'db>, db: &dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
    std::iter::once::<&'db dyn ToProto<'db>>(s)
}

impl<'db> Expr<'db> {
    pub fn new_literal(
        db: &'db dyn BaseDatabase,
        span: auto_lsp::tree_sitter::Range,
        literal: Literal,
    ) -> Expr<'db> {
        Expr::new(
            db,
            span.into(),
            ExprKind::PrimaryExpr {
                expr: PrimaryExpr::Literal(literal),
            },
        )
    }

    pub fn new_target(
        db: &'db dyn BaseDatabase,
        file: File,
        fq_name: &'db ast::generated::FqName,
    ) -> anyhow::Result<Expr<'db>> {
        Ok(Expr::new(
            db,
            fq_name.get_span().into(),
            ExprKind::PrimaryExpr {
                expr: PrimaryExpr::Target(
                    FqName::new(
                        db,
                        NamespacePath::from((
                            db,
                            fq_name
                                .fragment
                                .iter()
                                .map(|f| Ident::from_node(db, file, f.deref()))
                                .collect::<anyhow::Result<Vec<_>>>()?,
                        )),
                        Ident::from_node(db, file, fq_name.target.deref())?,
                    ),
                ),
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
            span.into(),
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
    Target(FqName),
    EnumValue {
        value: Ident,
    },
    VariableAccess {
        variable: Variable<'db>,
        multibits: MultibitsPart,
    }, 
    FuncCall {
        expr: Expr<'db>,
        params: Vec<ParamAssign<'db>>,
    },
    RefValue {
        value: RefValue<'db>,
    },
    ParenthesizedExpr {
        expr: Expr<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MultibitsPart {
    Offset { offset: Ident },
    SizedOffset { size: SizeOperator, offset: Ident },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum RefValue<'db> {
    Address { adress: RefAdress<'db> },
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum RefAdress<'db> {
    Symbolic {
        this: bool,
        kind: SymbolicVariableKind<'db>,
    },
    Instance {
        instance: Ident,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AccessOperator {
    I,
    Q,
    M,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SizeOperator {
    X,
    B,
    W,
    D,
    L,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Variable<'db> {
    Direct {
        kind: AccessOperator,
        size: Option<SizeOperator>,
        offset: Vec<Ident>,
    },
    Symbolic {
        this: bool,
        kind: SymbolicVariableKind<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum SymbolicVariableKind<'db> {
    VarAccess {
        access: VarAccess,
    },
    MultiElemVar {
        base: VarAccess,
        elements: Vec<MultiElemVarElement<'db>>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MultiElemVarElement<'db> {
    Subscript { expr: Vec<Expr<'db>> },        // []
    StructVariable { access: VarAccess },      // .
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VarAccess {
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

impl Numeric {
    pub fn to_string<'db>(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            Numeric::Binary(ident) => format!("Binary {}", ident.text(db)),
            Numeric::Hex(ident) => format!("Hex {}", ident.text(db)),
            Numeric::Octal(ident) => format!("Octal {}", ident.text(db)),
            Numeric::Signed(ident) => format!("Signed {}", ident.text(db)),
        }
    }
}

impl Literal {
    pub fn to_string<'db>(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            Literal::AnyNumeric(n) => format!("Number {}", match n {
                Numeric::Binary(ident) => ident.text(db),
                Numeric::Hex(ident) => ident.text(db),
                Numeric::Octal(ident) => ident.text(db),
                Numeric::Signed(ident) => ident.text(db),
            }),
            Literal::SInt(ident) => format!("SInt {}", ident.to_string(db)),
            Literal::Int(ident) => format!("Int {}", ident.to_string(db)),
            Literal::DInt(ident) => format!("DInt {}", ident.to_string(db)),
            Literal::LInt(ident) => format!("LInt {}", ident.to_string(db)),
            Literal::USInt(ident) => format!("USInt {}", ident.to_string(db)),
            Literal::UInt(ident) => format!("UInt {}", ident.to_string(db)),
            Literal::UDInt(ident) => format!("UDInt {}", ident.to_string(db)),
            Literal::ULInt(ident) => format!("ULInt {}", ident.to_string(db)),
            Literal::Byte(ident) => format!("Byte {}", ident.to_string(db)),
            Literal::Word(ident) => format!("Word {}", ident.to_string(db)),
            Literal::DWord(ident) => format!("DWord {}", ident.to_string(db)),
            Literal::LWord(ident) => format!("LWord {}", ident.to_string(db)),
            Literal::Real(ident) => format!("Real {}", ident.text(db)),
            Literal::LReal(ident) => format!("LReal {}", ident.text(db)),
            Literal::Bool(ident) => format!("Bool {}", ident.text(db)),
            Literal::Char(ident) => format!("Char {}", ident.text(db)),
            Literal::DChar(ident) => format!("DChar {}", ident.text(db)),
            Literal::Date(ident) => format!("Date {}", ident.text(db)),
            Literal::LDate(ident) => format!("LDate {}", ident.text(db)),
            Literal::Tod(ident) => format!("Tod {}", ident.text(db)),
            Literal::LTod(ident) => format!("LTod {}", ident.text(db)),
            Literal::Time(ident) => format!("Time {}", ident.text(db)),
            Literal::LTime(ident) => format!("LTime {}", ident.text(db)),
            Literal::DateTime(ident) => format!("DateTime {}", ident.text(db)),
            Literal::LDateTime(ident) => format!("LDateTime {}", ident.text(db)),
        }
    }
}
