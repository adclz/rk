use crate::ident::Ident;
use crate::to_proto::{self_iter, HirCtx, IterToProto, ProtoAndCtx, ToProto};
use auto_enums::auto_enum;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;

#[salsa::tracked(debug)]
pub struct Expr<'db> {
    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub expr: ExprKind<'db>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AddOperatorKind {
    Plus,
    Minus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BooleanOperatorKind {
    And,
    Or,
    Xor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComparisonOperatorKind {
    Eq, // ==
    Ne, // !=
    Lt, // <
    Gt, // >
    Le, // <=
    Ge, // >=
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultOperatorKind {
    Mul, // *
    Div, // /
    Mod, // %
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperatorKind {
    Plus, // +
    Minus, // -
    Not, // NOT
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExprKind<'db> {
    PrimaryExpr(PrimaryExpr<'db>),
    AddOperator {
        left: Expr<'db>,
        operator: AddOperatorKind,
        right: Expr<'db>,
    },
    BooleanOperator {
        left: Expr<'db>,
        operator: BooleanOperatorKind,
        right: Expr<'db>,
    },
    ComparisonOperator {
        left: Expr<'db>,
        operator: ComparisonOperatorKind,
        right: Expr<'db>,
    },
    MultOperator {
        left: Expr<'db>,
        operator: MultOperatorKind,
        right: Expr<'db>,
    },
    PowerOperator {
        left: Expr<'db>,
        right: Expr<'db>,
    },
    UnaryOperator {
        expr: Expr<'db>,
        operator: UnaryOperatorKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PrimaryExpr<'db> {
    Literal(Literal), // constant
    // Path --> Target
    VariableAccess {
        variable: VariableAccess<'db>,
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
    pub expr: PathExprKind<'db>
}

impl<'db> ToProto<'db> for PathExpr<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }
}

impl<'db> IterToProto<'db> for PathExpr<'db> {
    fn iter(
        &'db self,
        ctx: HirCtx<'db>,
    ) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        match &self.expr(ctx.db) {
            PathExprKind::Field(field) => self_iter(self, ctx),
            PathExprKind::Index(index) => self_iter(self, ctx),
            PathExprKind::VarAccess(var_access) => self_iter(self, ctx)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PathExprKind<'db> {
    Field(FieldExpr<'db>), // .
    Index(IndexExpr<'db>), // []
    VarAccess(VarAccess),  // Variable access (e.g. "var" or "var^")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct FieldExpr<'db> {
    pub path: Box<PathExprKind<'db>>,
    pub var: VarAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct IndexExpr<'db> {
    pub path: Box<PathExprKind<'db>>,
    pub index: Vec<Expr<'db>>,
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
        variable: VariableAccess<'db>,
    },
}

impl<'db> IterToProto<'db> for ParamAssign<'db> {
    fn iter(
        &'db self,
        ctx: HirCtx<'db>,
    ) -> impl Iterator<Item = ProtoAndCtx<'db>> {
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
pub struct VariableAccess<'db> {
    pub span: Span,

    pub kind: VariableAccessKind<'db>,
}

impl<'db> ToProto<'db> for VariableAccess<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        &self.span
    }
}


#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VariableAccessKind<'db> {
    Direct {
        adress: Ident,
        partly: bool,
        offset: Option<Ident>,
    },
    Symbolic(SymbolicVariable<'db>),
}

impl<'db> IterToProto<'db> for VariableAccess<'db> {
    fn iter(
        &'db self,
        ctx: HirCtx<'db>,
    ) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        match &self.kind {
            VariableAccessKind::Direct { adress, .. } => self_iter(self, ctx),
            VariableAccessKind::Symbolic(symbolic) => self_iter(self, ctx),
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

// todo: Should use ANY_* from the standard instead
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
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
    }
}

impl<'db> IterToProto<'db> for Expr<'db> {
    #[auto_enum(Iterator)]
    fn iter(&'db self, ctx: HirCtx<'db>) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        match self.expr(ctx.db) {
            ExprKind::AddOperator {
                left,
                operator, 
                right,
            } => Box::new(self_iter(self, ctx).chain(left.iter(ctx)).chain(right.iter(ctx))) as Box<dyn Iterator<Item = _>>,
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => Box::new(self_iter(self, ctx).chain(left.iter(ctx)).chain(right.iter(ctx))) as Box<dyn Iterator<Item = _>>,
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => Box::new(self_iter(self, ctx).chain(left.iter(ctx)).chain(right.iter(ctx))) as Box<dyn Iterator<Item = _>>,
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => Box::new(self_iter(self, ctx).chain(left.iter(ctx)).chain(right.iter(ctx))) as Box<dyn Iterator<Item = _>>,
            ExprKind::PowerOperator { left, right } => {
                Box::new(self_iter(self, ctx).chain(left.iter(ctx)).chain(right.iter(ctx))) as Box<dyn Iterator<Item = _>>
            }
            ExprKind::UnaryOperator { expr, operator } => Box::new(expr.iter(ctx)) as Box<dyn Iterator<Item = _>>,
            ExprKind::PrimaryExpr(primary) => Box::new(primary.iter(ctx)) as Box<dyn Iterator<Item = _>>,
        }
    }
}

impl<'db> IterToProto<'db> for PrimaryExpr<'db> {
    #[auto_enum(Iterator)]
    fn iter(
        &'db self,
        ctx: HirCtx<'db>,
    ) -> impl Iterator<Item = ProtoAndCtx<'db>> {
        match self {
            PrimaryExpr::FuncCall { path, params } => path.iter(ctx)
                .chain(params.iter().flat_map(move |p| p.iter(ctx))),
            PrimaryExpr::VariableAccess {
                variable,
                multibits,
            } => variable.iter(ctx),
            PrimaryExpr::ParenthesizedExpr { expr } => expr.iter(ctx),
            // todo: Unsure if literal and ref_value should be iterable
            PrimaryExpr::Literal(lit) => std::iter::empty(),
            PrimaryExpr::RefValue { value } => std::iter::empty(),
        }
    }
}
