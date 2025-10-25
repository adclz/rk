use crate::hir_def::expressions::invocation::{Invocation, InvocationKind};
use crate::hir_def::interned::identifier::{Ident, SpanIdent};
use crate::hir_def::scope::ScopeId;
use crate::{AstId, HirNodeInfo};
use auto_lsp::default::db::BaseDatabase;

#[salsa::tracked(debug)]
pub struct Expr<'db> {
    #[returns(ref)]
    pub expr: ExprKind<'db>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
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
    Plus,  // +
    Minus, // -
    Not,   // NOT
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExprKind<'db> {
    // Normal expressions
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
    Literal(Elementary), // constant
    // Path --> Target
    VariableAccess {
        variable: VariableAccess<'db>,
        multibits: Option<MultibitsPart>,
    },
    FuncCall(FuncCall<'db>),
    Invocation(Invocation<'db>),
    EnumValue {
        name: PathExpr<'db>,
        variant: SpanIdent<'db>,
    },
    RefValue {
        value: RefValue<'db>,
    },
    ParenthesizedExpr {
        expr: Expr<'db>,
    },
}

#[salsa::tracked(debug)]
pub struct FuncCall<'db> {
    pub path: PathExpr<'db>,
    pub params: Vec<ParamAssign<'db>>,
}

#[salsa::tracked(debug)]
pub struct PathExpr<'db> {
    pub expr: PathExprKind<'db>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for PathExpr<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        match &self.expr(db) {
            PathExprKind::Field(field_expr) => match field_expr.var {
                VarAccess::Simple(ref simple) => simple.id,
                VarAccess::Deref(ref deref) => deref.id,
            },
            PathExprKind::Index(index_expr) => index_expr.path.get_id(db),
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => simple.id,
                VarAccess::Deref(deref) => deref.id,
            },
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> PathExpr<'db> {
    pub fn ident(&self, db: &'db dyn BaseDatabase) -> SpanIdent<'db> {
        match &self.expr(db) {
            PathExprKind::Field(field_expr) => match field_expr.var {
                VarAccess::Simple(ref simple) => *simple,
                VarAccess::Deref(ref deref) => *deref,
            },
            PathExprKind::Index(index_expr) => index_expr.path.ident(db),
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => *simple,
                VarAccess::Deref(deref) => *deref,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PathExprKind<'db> {
    Field(FieldExpr<'db>),     // .
    Index(IndexExpr<'db>),     // []
    VarAccess(VarAccess<'db>), // Variable access (e.g. "var" or "var^")
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct FieldExpr<'db> {
    pub path: PathExpr<'db>,
    pub var: VarAccess<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct IndexExpr<'db> {
    pub path: PathExpr<'db>,
    pub index: Vec<Expr<'db>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MultibitsPart {
    Offset(Integer),
    // XBWDL
    AccessOffset { access: Ident, offset: Integer },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum RefValue<'db> {
    Address(SymbolicVariable<'db>),
    Null,
}

#[salsa::tracked(debug)]
pub struct ParamAssign<'db> {
    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,

    pub kind: ParamAssignKind<'db>,
}

impl<'db> HirNodeInfo<'db> for ParamAssign<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

// use local variables
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ParamAssignKind<'db> {
    NonFormal {
        value: Expr<'db>,
    },
    FormalInput {
        param: SpanIdent<'db>,
        value: Expr<'db>,
    },
    FormalOutput {
        not: bool,
        param: SpanIdent<'db>,
        variable: VariableAccess<'db>,
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

#[salsa::tracked(debug)]
pub struct VariableAccess<'db> {
    pub kind: VariableAccessKind<'db>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for VariableAccess<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SymbolicVariable<'db> {
    pub kind: PathExpr<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VarAccess<'db> {
    Simple(SpanIdent<'db>),
    Deref(SpanIdent<'db>), // ^
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Elementary {
    // Bit strings
    Bool(Ident),
    Byte(Integer),
    Word(Integer),
    DWord(Integer),
    LWord(Integer),

    // Signed and Unsigned Integers
    SInt(Integer),
    Int(Integer),
    DInt(Integer),
    LInt(Integer),

    USInt(Integer),
    UInt(Integer),
    UDInt(Integer),
    ULInt(Integer),

    // Time
    Time(Ident),
    LTime(Ident),

    // Reals
    Real(Ident),
    LReal(Ident),

    // dates
    DateAndTime(Ident),
    LDateTime(Ident),
    LDate(Ident),
    Date(Ident),
    TimeOfDay(Ident),
    LTod(Ident),

    // strings
    AnyString(Ident),
    AnyChar(Ident),

    // Has to be solved later
    InferInteger(Integer),
    InferIdent(Ident),
}

#[salsa::interned(debug, no_lifetime)]
pub struct Integer {
    pub ident: Ident,
    pub kind: IntegerKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntegerKind {
    Binary,
    Hex,
    Octal,
    Signed,
}

impl<'db> HirNodeInfo<'db> for Expr<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[salsa::tracked(debug)]
pub struct InitExpr<'db> {
    pub kind: InitExprKind<'db>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitExprKind<'db> {
    ArrayInit {
        values: Vec<InitExpr<'db>>,
    },
    ArrayIndexedElement {
        size: SpanIdent<'db>,
        values: Vec<InitExpr<'db>>,
    },
    StructInit {
        values: Vec<InitExpr<'db>>,
    },
    StructElement {
        name: SpanIdent<'db>,
        value: Box<InitExpr<'db>>,
    },
    ConstantExpr(Expr<'db>),
}

impl<'db> HirNodeInfo<'db> for InitExpr<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> InitExpr<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> &str {
        match self.kind(db) {
            InitExprKind::StructInit { .. } => "STRUCT init",
            InitExprKind::ArrayInit { .. } => "ARRAY init",
            InitExprKind::ArrayIndexedElement { size, .. } => "ARRAY element",
            InitExprKind::StructElement { name, .. } => "STRUCT field",
            InitExprKind::ConstantExpr(expr) => "<expression>",
        }
    }
}

impl<'db> Expr<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> &str {
        match self.expr(db) {
            ExprKind::PrimaryExpr(primary_expr) => primary_expr.to_string(db),
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => "<arithmetic expression>",
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => "<boolean expression>",
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => "<comparison expression>",
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => "<multiplicative expression>",
            ExprKind::PowerOperator { left, right } => "<power expression>",
            ExprKind::UnaryOperator { expr, operator } => "<unary expression>",
        }
    }
}

impl<'db> PrimaryExpr<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> &str {
        match self {
            PrimaryExpr::Literal(lit) => match lit {
                Elementary::Bool(_) => "BOOL literal",
                Elementary::Byte(_) => "BYTE literal",
                Elementary::Word(_) => "WORD literal",
                Elementary::DWord(_) => "DWORD literal",
                Elementary::LWord(_) => "LWORD literal",
                Elementary::SInt(_) => "SINT literal",
                Elementary::Int(_) => "INT literal",
                Elementary::DInt(_) => "DINT literal",
                Elementary::LInt(_) => "LINT literal",
                Elementary::USInt(_) => "USINT literal",
                Elementary::UInt(_) => "UINT literal",
                Elementary::UDInt(_) => "UDINT literal",
                Elementary::ULInt(_) => "ULINT literal",
                Elementary::Time(_) => "TIME literal",
                Elementary::LTime(_) => "LTIME literal",
                Elementary::Real(_) => "REAL literal",
                Elementary::LReal(_) => "LREAL literal",
                Elementary::DateAndTime(_) => "DATE_AND_TIME literal",
                Elementary::LDateTime(_) => "LDATE_AND_TIME literal",
                Elementary::LDate(_) => "LDATE literal",
                Elementary::Date(_) => "DATE literal",
                Elementary::TimeOfDay(_) => "TIME_OF_DAY literal",
                Elementary::LTod(_) => "LTOD literal",
                Elementary::AnyString(_) => "STRING literal",
                Elementary::AnyChar(_) => "CHAR literal",
                Elementary::InferInteger(_) => "<integer>",
                Elementary::InferIdent(_) => "<identifier>",
            },
            PrimaryExpr::VariableAccess {
                variable,
                multibits,
            } => "<variable access>",
            PrimaryExpr::FuncCall(func_call) => func_call.path(db).ident(db).text(db),
            PrimaryExpr::Invocation(invocation) => match invocation.kind(db) {
                InvocationKind::Super { path } => "<SUPER invocation>",
                InvocationKind::This { path } => "<THIS invocation>",
                InvocationKind::SuperBody { .. } => "<SUPER.BODY invocation>",
            },
            PrimaryExpr::EnumValue { name, variant } => variant.text(db),
            PrimaryExpr::RefValue { value } => match value {
                RefValue::Address(addr) => "<DEREF>",
                RefValue::Null => "NULL",
            },
            PrimaryExpr::ParenthesizedExpr { expr } => expr.to_string(db),
        }
    }
}
