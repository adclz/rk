use std::ops::Deref;

use crate::{ident::Ident, solver::NamespacePath};
use auto_lsp::anyhow;
use auto_lsp::core::ast::AstNode;
use auto_lsp::default::db::File;
use auto_lsp::{anyhow::anyhow, default::db::BaseDatabase};

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExprKind<'db> {
    PrimaryExpr { expr: PrimaryExpr<'db> },
    BooleanOperator { left: Expr<'db>, right: Expr<'db> },
    ComparisonOperator { left: Expr<'db>, right: Expr<'db> },
    AddOperator { left: Expr<'db>, right: Expr<'db> },
    MultOperator { left: Expr<'db>, right: Expr<'db> },
    PowerOperator { left: Expr<'db>, right: Expr<'db> },
    UnaryOperator { expr: Expr<'db> },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PrimaryExpr<'db> {
    Literal(Literal),
    // Path --> Target
    Target { path: NamespacePath, target: Ident },
    EnumValue { value: Ident },
    FuncCall,
    RefValue,
    ParenthesizedExpr(Expr<'db>),
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
