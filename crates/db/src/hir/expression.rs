use std::path::Iter;

use crate::ident::Ident;
use crate::to_proto::{self_iter, IterToProto, ToProto};
use auto_enums::auto_enum;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;
use bitflags::bitflags;

#[salsa::tracked(debug)]
pub struct Expr<'db> {
    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub expr: ExprKind<'db>,

    #[no_eq]
    pub id: usize
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
    PrimaryExpr(PrimaryExpr<'db>),
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
    Literal(Literal), // constant
    // Path --> Target
    VariableAccess {
        variable: Variable<'db>,
        multibits: MultibitsPart,
    },
    FuncCall {
        path: PathExpr<'db>,
        params: Vec<ParamAssign<'db>>,
    },
    RefValue {
        value: RefValue<'db>,
    },
    ParenthesizedExpr {
        expr: Expr<'db>,
    },
}

#[salsa::tracked(debug)]
pub struct PathExpr<'db> {
    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub expr: PathExprKind<'db>,

    #[no_eq]
    pub id: usize
}

impl<'db> ToProto<'db> for PathExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> usize {
        self.id(db)
    }

    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }
}

impl<'db> IterToProto<'db> for PathExpr<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match &self.expr(db) {
            PathExprKind::Field(field) => self_iter(self),
            PathExprKind::Index(index) => self_iter(self),
            PathExprKind::VarAccess(var_access) => self_iter(self)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum PathExprKind<'db> {
    Field(FieldExpr<'db>), // .
    Index(IndexExpr<'db>), // []
    VarAccess(VarAccess),  // Variable access (e.g. "var" or "var^")
}

#[salsa::tracked(debug)]
pub struct FieldExpr<'db> {
    path: PathExprKind<'db>,
    var: VarAccess,
}

#[salsa::tracked(debug)]
pub struct IndexExpr<'db> {
    path: PathExprKind<'db>,
    index: Vec<Expr<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MultibitsPart {
    Offset(Ident),
    SizedOffset { size: SizeOperator, offset: Ident },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum RefValue<'db> {
    Address(RefAdress<'db>),
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum RefAdress<'db> {
    Symbolic(SymbolicVariable<'db>),
    Instance(Ident),
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

impl<'db> IterToProto<'db> for ParamAssign<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        std::iter::empty()
    }
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
        adress: Ident,
        partly: bool,
        offset: Option<Ident>,
    },
    Symbolic(SymbolicVariable<'db>),
}

impl<'db> ToProto<'db> for Variable<'db> {
    fn get_id(&'db self, db: &'db dyn crate::BaseDatabase) -> usize {
        todo!()    
    }

    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        todo!()
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.spanned(db)
    }
}

impl<'db> IterToProto<'db> for Variable<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self {
            Variable::Direct { adress, .. } => self_iter(self),
            Variable::Symbolic(symbolic) => self_iter(self),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SymbolicVariable<'db> {
    pub this: bool,
    pub kind: PathExprKind<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
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
            Literal::AnyNumeric(n) => format!(
                "Number {}",
                match n {
                    Numeric::Binary(ident) => ident.text(db),
                    Numeric::Hex(ident) => ident.text(db),
                    Numeric::Octal(ident) => ident.text(db),
                    Numeric::Signed(ident) => ident.text(db),
                }
            ),
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

impl<'db> ToProto<'db> for Expr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> usize {
        self.id(db)
    }

    fn spanned(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }

    fn named_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }
}

impl<'db> IterToProto<'db> for Expr<'db> {
    #[auto_enum(Iterator)]
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self.expr(db) {
            ExprKind::AddOperator {
                left,
                operator, 
                right,
            } => Box::new(self_iter(self).chain(left.iter(db)).chain(right.iter(db))) as Box<dyn Iterator<Item = _>>,
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => Box::new(self_iter(self).chain(left.iter(db)).chain(right.iter(db))) as Box<dyn Iterator<Item = _>>,
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => Box::new(self_iter(self).chain(left.iter(db)).chain(right.iter(db))) as Box<dyn Iterator<Item = _>>,
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => Box::new(self_iter(self).chain(left.iter(db)).chain(right.iter(db))) as Box<dyn Iterator<Item = _>>,
            ExprKind::PowerOperator { left, right } => {
                Box::new(self_iter(self).chain(left.iter(db)).chain(right.iter(db))) as Box<dyn Iterator<Item = _>>
            }
            ExprKind::UnaryOperator { expr, operator } => Box::new(expr.iter(db)) as Box<dyn Iterator<Item = _>>,
            ExprKind::PrimaryExpr(primary) => Box::new(primary.iter(db)) as Box<dyn Iterator<Item = _>>,
        }
    }
}

impl<'db> IterToProto<'db> for PrimaryExpr<'db> {
    #[auto_enum(Iterator)]
    fn iter(
        &'db self,
        db: &'db dyn crate::BaseDatabase,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self {
            PrimaryExpr::FuncCall { path, params } => path.iter(db)
                .chain(params.iter().flat_map(|p| p.iter(db))),
            PrimaryExpr::VariableAccess {
                variable,
                multibits,
            } => variable.iter(db),
            PrimaryExpr::ParenthesizedExpr { expr } => expr.iter(db),
            PrimaryExpr::Literal(lit) => std::iter::empty(),
            PrimaryExpr::RefValue { value } => std::iter::empty(),
        }
    }
}
