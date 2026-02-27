use crate::hir_def::expressions::invocation::Invocation;
use crate::hir_def::expressions::spec::Spec;
use crate::hir_def::interned::identifier::{Ident, SpanIdent};
use crate::hir_def::pous::variable::DirectVariable;
use crate::hir_def::scope::ScopeId;
use crate::{AstId, HirNodeInfo};
use db::WorkspaceDataBase;

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

impl AddOperatorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            AddOperatorKind::Plus => "+",
            AddOperatorKind::Minus => "-",
        }
    }
}

impl std::fmt::Display for AddOperatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddOperatorKind::Plus => write!(f, "+"),
            AddOperatorKind::Minus => write!(f, "-"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BooleanOperatorKind {
    And,
    Or,
    Xor,
}

impl BooleanOperatorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BooleanOperatorKind::And => "AND",
            BooleanOperatorKind::Or => "OR",
            BooleanOperatorKind::Xor => "XOR",
        }
    }
}

impl std::fmt::Display for BooleanOperatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BooleanOperatorKind::And => write!(f, "AND"),
            BooleanOperatorKind::Or => write!(f, "OR"),
            BooleanOperatorKind::Xor => write!(f, "XOR"),
        }
    }
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

impl ComparisonOperatorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ComparisonOperatorKind::Eq => "=",
            ComparisonOperatorKind::Ne => "<>",
            ComparisonOperatorKind::Lt => "<",
            ComparisonOperatorKind::Gt => ">",
            ComparisonOperatorKind::Le => "<=",
            ComparisonOperatorKind::Ge => ">=",
        }
    }
}

impl std::fmt::Display for ComparisonOperatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComparisonOperatorKind::Eq => write!(f, "="),
            ComparisonOperatorKind::Ne => write!(f, "<>"),
            ComparisonOperatorKind::Lt => write!(f, "<"),
            ComparisonOperatorKind::Gt => write!(f, ">"),
            ComparisonOperatorKind::Le => write!(f, "<="),
            ComparisonOperatorKind::Ge => write!(f, ">="),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MultOperatorKind {
    Mul, // *
    Div, // /
    Mod, // %
}

impl MultOperatorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MultOperatorKind::Mul => "*",
            MultOperatorKind::Div => "/",
            MultOperatorKind::Mod => "MOD",
        }
    }
}

impl std::fmt::Display for MultOperatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MultOperatorKind::Mul => write!(f, "*"),
            MultOperatorKind::Div => write!(f, "/"),
            MultOperatorKind::Mod => write!(f, "MOD"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOperatorKind {
    Plus,  // +
    Minus, // -
    Not,   // NOT
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FoldOperatorKind {
    // Arithmetic
    Plus,  // +
    Minus, // -
    Mul,   // *
    Div,   // /
    Mod,   // MOD
    Power, // **
    // Boolean / Bitwise
    And, // AND, &
    Or,  // OR
    Xor, // XOR
    // Comparison
    Eq, // =
    Ne, // <>
    Lt, // <
    Gt, // >
    Le, // <=
    Ge, // >=
}

impl FoldOperatorKind {
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            FoldOperatorKind::Eq
                | FoldOperatorKind::Ne
                | FoldOperatorKind::Lt
                | FoldOperatorKind::Gt
                | FoldOperatorKind::Le
                | FoldOperatorKind::Ge
        )
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            FoldOperatorKind::Plus => "+",
            FoldOperatorKind::Minus => "-",
            FoldOperatorKind::Mul => "*",
            FoldOperatorKind::Div => "/",
            FoldOperatorKind::Mod => "MOD",
            FoldOperatorKind::Power => "**",
            FoldOperatorKind::And => "AND",
            FoldOperatorKind::Or => "OR",
            FoldOperatorKind::Xor => "XOR",
            FoldOperatorKind::Eq => "=",
            FoldOperatorKind::Ne => "<>",
            FoldOperatorKind::Lt => "<",
            FoldOperatorKind::Gt => ">",
            FoldOperatorKind::Le => "<=",
            FoldOperatorKind::Ge => ">=",
        }
    }
}

impl std::fmt::Display for FoldOperatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FoldOperatorKind::Plus => write!(f, "+"),
            FoldOperatorKind::Minus => write!(f, "-"),
            FoldOperatorKind::Mul => write!(f, "*"),
            FoldOperatorKind::Div => write!(f, "/"),
            FoldOperatorKind::Mod => write!(f, "MOD"),
            FoldOperatorKind::Power => write!(f, "**"),
            FoldOperatorKind::And => write!(f, "AND"),
            FoldOperatorKind::Or => write!(f, "OR"),
            FoldOperatorKind::Xor => write!(f, "XOR"),
            FoldOperatorKind::Eq => write!(f, "="),
            FoldOperatorKind::Ne => write!(f, "<>"),
            FoldOperatorKind::Lt => write!(f, "<"),
            FoldOperatorKind::Gt => write!(f, ">"),
            FoldOperatorKind::Le => write!(f, "<="),
            FoldOperatorKind::Ge => write!(f, ">="),
        }
    }
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
    // Fold expression: ...param<op> (variadic parameter fold)
    FoldExpr {
        param: Ident,
        #[allow(dead_code)]
        param_id: SpanIdent<'db>,
        operator: FoldOperatorKind,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PrimaryExpr<'db> {
    Literal(Elementary), // constant
    // Path --> Target
    VariableAccess(VariableAccess<'db>),
    FuncCall(FuncCall<'db>),
    EnumValue {
        name: BeginPathExpr<'db>,
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
    pub path: BeginPathExpr<'db>,

    #[returns(ref)]
    pub type_args: Vec<Spec<'db>>,

    #[returns(ref)]
    pub params: Vec<ParamAssign<'db>>,
}

#[salsa::tracked(debug)]
pub struct BeginPathExpr<'db> {
    pub invocation: Option<Invocation<'db>>,
    pub expr: Option<PathExpr<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for BeginPathExpr<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
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
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match &self.expr(db) {
            PathExprKind::Field(field_expr) => match field_expr.var {
                VarAccess::Simple(ref simple) => simple.id,
            },
            PathExprKind::Index(index_expr) => index_expr.path.get_id(db),
            PathExprKind::Deref(deref_expr) => deref_expr.path.get_id(db),
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => simple.id,
            },
        }
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> PathExpr<'db> {
    pub fn ident(&self, db: &'db dyn WorkspaceDataBase) -> SpanIdent<'db> {
        match &self.expr(db) {
            PathExprKind::Field(field_expr) => match field_expr.var {
                VarAccess::Simple(ref simple) => *simple,
            },
            PathExprKind::Index(index_expr) => index_expr.path.ident(db),
            PathExprKind::Deref(deref_expr) => deref_expr.path.ident(db),
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => *simple,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum PathExprKind<'db> {
    Field(FieldExpr<'db>),     // .
    Index(IndexExpr<'db>),     // []
    Deref(DerefExpr<'db>),     // ^
    VarAccess(VarAccess<'db>), // Variable access (e.g. "var")
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
pub struct DerefExpr<'db> {
    pub path: PathExpr<'db>,
    pub count: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MultibitsPart {
    Offset(Integer),
    // XBWDL
    AccessOffset { access: Ident, offset: Integer },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum RefValue<'db> {
    Address(BeginPathExpr<'db>),
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
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
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

// Variable : Direct_Variable | Symbolic_Variable;
// Direct_Variable : '%' ( 'I' | 'Q' | 'M' ) ( 'X' | 'B' | 'W' | 'D' | 'L' )? Unsigned_Int ( '.' Unsigned_Int )*;
// Symbolic_Variable : ( ( 'THIS' '.' ) | ( Namespace_Name '.' )+ )? ( Var_Access | Multi_Elem_Var );
// Var_Access : Identifier | Ref_Deref;

// Variable_Access : Variable Multibit_Part_Access ?;
// Multibit_Part_Access : '.' ( Unsigned_Int | '%' ( 'X' | 'B' | 'W' | 'D' | 'L' ) ? Unsigned_Int );

#[salsa::tracked(debug)]
pub struct VariableAccess<'db> {
    pub kind: VariableAccessKind<'db>,

    pub multibits: Option<MultibitsPart>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VariableAccessKind<'db> {
    Direct(DirectVariable<'db>),
    Symbolic(BeginPathExpr<'db>),
}

impl<'db> HirNodeInfo<'db> for VariableAccess<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VarAccess<'db> {
    Simple(SpanIdent<'db>),
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
    String(Ident),
    WString(Ident),
    Char(Ident),
    WChar(Ident),

    // Has to be solved later
    InferInteger(Integer),
    InferFloat(Ident),
}

impl Elementary {
    pub fn has_infer(&self) -> bool {
        matches!(
            self,
            Elementary::InferInteger(_) | Elementary::InferFloat(_)
        )
    }
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
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
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
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}
