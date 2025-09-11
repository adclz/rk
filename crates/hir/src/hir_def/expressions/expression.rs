use crate::completions::snippets::elem_type_names_init;
use crate::hir_def::interned::identifier::{Ident, SpanIdent};
use crate::hir_def::scope::FileScopeId;
use crate::to_proto::{AstId, ToProto};
use auto_lsp::default::db::BaseDatabase;

#[salsa::tracked(debug)]
pub struct Expr<'db> {
    #[returns(ref)]
    pub expr: ExprKind<'db>,

    pub id: AstId,

    pub scope_id: FileScopeId<'db>,
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
    #[tracked]
    pub expr: PathExprKind<'db>,

    pub id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> ToProto<'db> for PathExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
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

impl<'db> PathExpr<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> SpanIdent<'db> {
        match &self.expr(db) {
            PathExprKind::Field(field_expr) => match field_expr.var {
                VarAccess::Simple(ref simple) => simple.clone(),
                VarAccess::Deref(ref deref) => deref.clone(),
            },
            PathExprKind::Index(index_expr) => index_expr.path.to_string(db),
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(simple) => simple.clone(),
                VarAccess::Deref(deref) => deref.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MultibitsPart {
    Offset(Integer),
    // XBWDL
    AccessOffset { access: Ident, offset: Integer },
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

    pub id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> ToProto<'db> for VariableAccess<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
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
    pub this: bool,
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

impl<'db> ToProto<'db> for Expr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }

    fn completion(
        &'db self,
        _db: &'db dyn BaseDatabase,
        _offset: usize,
    ) -> Option<Vec<auto_lsp::lsp_types::CompletionItem>> {
        Some(elem_type_names_init())
    }
}

#[salsa::tracked(debug)]
pub struct InitExpr<'db> {
    pub kind: InitExprKind<'db>,

    pub id: AstId,

    pub scope_id: FileScopeId<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitExprKind<'db> {
    ArrayInit {
        values: Vec<InitExpr<'db>>,
    },
    ArrayIndexedElement {
        index: Integer,
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

impl<'db> ToProto<'db> for InitExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}
