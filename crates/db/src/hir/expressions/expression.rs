use crate::completions::snippets::elem_type_names_init;
use crate::hir::interned::identifier::{Ident, SpannedIdent};
use crate::hir::scope::FileScopeId;
use crate::hir::semantic_index::SemanticIndex;
use crate::to_proto::{self_iter, AstId, IterToProto, ToProto};
use auto_enums::auto_enum;
use auto_lsp::default::db::BaseDatabase;

#[salsa::tracked(debug)]
pub struct Expr<'db> {
    #[returns(ref)]
    pub expr: ExprKind<'db>,

    pub id: AstId,

    pub scope_id: FileScopeId,
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

    pub scope_id: FileScopeId,
}

impl<'db> ToProto<'db> for PathExpr<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }
}

impl<'db> IterToProto<'db> for PathExpr<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex<'db>,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        std::iter::empty()
        /*resolved_path_expr(db, sema.file, *self)
            .clone()
            .elements
            .iter()
            .map(|element| element as _)*/
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
    pub path: PathExpr<'db>,
    pub var: VarAccess,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct IndexExpr<'db> {
    pub path: PathExpr<'db>,
    pub index: Vec<Expr<'db>>,
}

impl<'db> PathExpr<'db> {
    pub fn to_string(&self, db: &'db dyn BaseDatabase) -> SpannedIdent {
        match &self.expr(db) {
            PathExprKind::Field(field_expr) => match field_expr.var {
                VarAccess::Simple(ref simple) => simple.clone(),
                VarAccess::Deref(ref deref) => deref.clone(),
            },
            PathExprKind::Index(index_expr) => index_expr.path.to_string(db),
            PathExprKind::VarAccess(var_access) => match var_access {
                VarAccess::Simple(ref simple) => simple.clone(),
                VarAccess::Deref(ref deref) => deref.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MultibitsPart {
    Offset(Numeric),
    // XBWDL
    AccessOffset { access: Ident, offset: Numeric },
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
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
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
pub struct VariableAccess<'db> {
    pub kind: VariableAccessKind<'db>,

    pub id: AstId,

    pub scope_id: FileScopeId
}

impl<'db> ToProto<'db> for VariableAccess<'db> {
    fn get_id(&'db self, db: &'db dyn crate::BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&'db self, db: &'db dyn crate::BaseDatabase) -> FileScopeId {
        self.scope_id
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
    #[auto_enum(Iterator)]
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match &self.kind {
            VariableAccessKind::Direct { adress, .. } => self_iter(self),
            VariableAccessKind::Symbolic(symbolic) => symbolic.kind.iter(db, sema),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct SymbolicVariable<'db> {
    pub this: bool,
    pub kind: PathExpr<'db>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum VarAccess {
    Simple(SpannedIdent),
    Deref(SpannedIdent), // ^
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Elementary {
    // Bit strings
    Bool(Ident),
    Byte(Numeric),
    Word(Numeric),
    DWord(Numeric),
    LWord(Numeric),

    // Signed and Unsigned Integers
    SInt(Numeric),
    Int(Numeric),
    DInt(Numeric),
    LInt(Numeric),

    USInt(Numeric),
    UInt(Numeric),
    UDInt(Numeric),
    ULInt(Numeric),

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
    InferNumeric(Numeric),
    InferIdent(Ident),
}

#[salsa::interned(debug, no_lifetime)]
pub struct Numeric {
    pub ident: Ident,
    pub kind: NumericKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NumericKind {
    Binary,
    Hex,
    Octal,
    Signed,
}

#[salsa::tracked]
impl Ident {
    #[salsa::tracked]
    pub fn as_f32(self, db: &dyn BaseDatabase) -> Result<f32, std::num::ParseFloatError> {
        self.text(db).parse()
    }

    #[salsa::tracked]
    pub fn as_f64(self, db: &dyn BaseDatabase) -> Result<f64, std::num::ParseFloatError> {
        self.text(db).parse()
    }
}

#[salsa::tracked]
impl Numeric {
    #[salsa::tracked]
    pub fn as_bool(self, db: &dyn BaseDatabase) -> Result<bool, std::str::ParseBoolError> {
        self.ident(db).text(db).parse()
    }

    #[salsa::tracked]
    pub fn as_u8(self, db: &dyn BaseDatabase) -> Result<u8, std::num::ParseIntError> {
        match self.kind(db) {
            NumericKind::Binary => {
                u8::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            NumericKind::Octal => {
                u8::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            NumericKind::Hex => {
                u8::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            NumericKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u16(self, db: &dyn BaseDatabase) -> Result<u16, std::num::ParseIntError> {
        match self.kind(db) {
            NumericKind::Binary => {
                u16::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            NumericKind::Octal => {
                u16::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            NumericKind::Hex => {
                u16::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            NumericKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u32(self, db: &dyn BaseDatabase) -> Result<u32, std::num::ParseIntError> {
        match self.kind(db) {
            NumericKind::Binary => {
                u32::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            NumericKind::Octal => {
                u32::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            NumericKind::Hex => {
                u32::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            NumericKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_u64(self, db: &dyn BaseDatabase) -> Result<u64, std::num::ParseIntError> {
        match self.kind(db) {
            NumericKind::Binary => {
                u64::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            NumericKind::Octal => {
                u64::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            NumericKind::Hex => {
                u64::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            NumericKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_i8(self, db: &dyn BaseDatabase) -> Result<i8, std::num::ParseIntError> {
        match self.kind(db) {
            NumericKind::Binary => {
                i8::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            NumericKind::Octal => {
                i8::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            NumericKind::Hex => {
                i8::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            NumericKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_i16(self, db: &dyn BaseDatabase) -> Result<i16, std::num::ParseIntError> {
        match self.kind(db) {
            NumericKind::Binary => {
                i16::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            NumericKind::Octal => {
                i16::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            NumericKind::Hex => {
                i16::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            NumericKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_i32(self, db: &dyn BaseDatabase) -> Result<i32, std::num::ParseIntError> {
        match self.kind(db) {
            NumericKind::Binary => {
                i32::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            NumericKind::Octal => {
                i32::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            NumericKind::Hex => {
                i32::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            NumericKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    #[salsa::tracked]
    pub fn as_i64(self, db: &dyn BaseDatabase) -> Result<i64, std::num::ParseIntError> {
        match self.kind(db) {
            NumericKind::Binary => {
                i64::from_str_radix(self.ident(db).text(db).trim_start_matches("2#"), 2)
            }
            NumericKind::Octal => {
                i64::from_str_radix(self.ident(db).text(db).trim_start_matches("8#"), 8)
            }
            NumericKind::Hex => {
                i64::from_str_radix(self.ident(db).text(db).trim_start_matches("16#"), 16)
            }
            NumericKind::Signed => self.ident(db).text(db).parse(),
        }
    }

    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        match self.kind(db) {
            NumericKind::Binary => format!("[Binary] {}", self.ident(db).text(db)),
            NumericKind::Hex => format!("[Hexa] {}", self.ident(db).text(db)),
            NumericKind::Octal => format!("[Octal] {}", self.ident(db).text(db)),
            NumericKind::Signed => self.ident(db).text(db).to_string(),
        }
    }
}

impl Elementary {
    pub fn to_string(&self, db: &dyn BaseDatabase) -> String {
        match self {
            Self::InferNumeric(numeric) => numeric.to_string(db),
            Self::InferIdent(ident) => ident.text(db).to_string(),
            Self::Bool(ident) => format!("Bool: {}", ident.text(db)),
            Self::Byte(ident) => format!("Byte: {}", ident.to_string(db)),
            Self::Word(ident) => format!("Word: {}", ident.to_string(db)),
            Self::DWord(ident) => format!("DWord: {}", ident.to_string(db)),
            Self::LWord(ident) => format!("Lword: {}", ident.to_string(db)),
            Self::Real(ident) => format!("Real: {}", ident.text(db)),
            Self::LReal(ident) => format!("LReal: {}", ident.text(db)),
            Self::SInt(ident) => format!("SInt: {}", ident.to_string(db)),
            Self::Int(ident) => format!("Int: {}", ident.to_string(db)),
            Self::DInt(ident) => format!("DInt: {}", ident.to_string(db)),
            Self::LInt(ident) => format!("LInt: {}", ident.to_string(db)),
            Self::USInt(ident) => format!("USInt: {}", ident.to_string(db)),
            Self::UInt(ident) => format!("UInt: {}", ident.to_string(db)),
            Self::UDInt(ident) => format!("UDInt: {}", ident.to_string(db)),
            Self::ULInt(ident) => format!("ULInt: {}", ident.to_string(db)),
            Self::Time(ident) => format!("Time: {}", ident.text(db)),
            Self::LTime(ident) => format!("LTime: {}", ident.text(db)),
            Self::AnyString(ident) => format!("String: {}", ident.text(db)),
            Self::AnyChar(ident) => format!("Char: {}", ident.text(db)),
            Self::DateAndTime(ident) => format!("Date and Time: {}", ident.text(db)),
            Self::LDateTime(ident) => format!("Long Date and Time: {}", ident.text(db)),
            Self::Date(ident) => format!("Date: {}", ident.text(db)),
            Self::LDate(ident) => format!("Long Date: {}", ident.text(db)),
            Self::TimeOfDay(ident) => format!("Time of Day: {}", ident.text(db)),
            Self::LTod(ident) => format!("Long Time of Day: {}", ident.text(db)),
        }
    }
}

impl<'db> ToProto<'db> for Expr<'db> {
    fn get_id(&'db self, db: &'db dyn crate::BaseDatabase) -> AstId {
        self.id(db)
    }
    
    fn get_scope_id(&'db self, db: &'db dyn crate::BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }

    fn completion(
        &'db self,
        _db: &'db dyn crate::BaseDatabase,
        _sema: &'db SemanticIndex<'db>,
        _offset: usize,
    ) -> Option<Vec<auto_lsp::lsp_types::CompletionItem>> {
        Some(elem_type_names_init())
    }
}

impl<'db> IterToProto<'db> for Expr<'db> {
    #[auto_enum(Iterator)]
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self.expr(db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => Box::new(
                self_iter(self)
                    .chain(left.iter(db, sema))
                    .chain(right.iter(db, sema)),
            ) as Box<dyn Iterator<Item = _>>,
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => Box::new(
                self_iter(self)
                    .chain(left.iter(db, sema))
                    .chain(right.iter(db, sema)),
            ) as Box<dyn Iterator<Item = _>>,
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => Box::new(
                self_iter(self)
                    .chain(left.iter(db, sema))
                    .chain(right.iter(db, sema)),
            ) as Box<dyn Iterator<Item = _>>,
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => Box::new(
                self_iter(self)
                    .chain(left.iter(db, sema))
                    .chain(right.iter(db, sema)),
            ) as Box<dyn Iterator<Item = _>>,
            ExprKind::PowerOperator { left, right } => Box::new(
                self_iter(self)
                    .chain(left.iter(db, sema))
                    .chain(right.iter(db, sema)),
            ) as Box<dyn Iterator<Item = _>>,
            ExprKind::UnaryOperator { expr, operator } => {
                Box::new(self_iter(self).chain(expr.iter(db, sema))) as Box<dyn Iterator<Item = _>>
            }
            ExprKind::PrimaryExpr(primary) => {
                Box::new(self_iter(self).chain(primary.iter(db, sema)))
                    as Box<dyn Iterator<Item = _>>
            }
        }
    }
}

impl<'db> IterToProto<'db> for PrimaryExpr<'db> {
    #[auto_enum(Iterator)]
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match self {
            PrimaryExpr::FuncCall { path, params } => path
                .iter(db, sema)
                .chain(params.iter().flat_map(move |p| p.iter(db, sema))),
            PrimaryExpr::VariableAccess {
                variable,
                multibits,
            } => variable.iter(db, sema),
            PrimaryExpr::ParenthesizedExpr { expr } => expr.iter(db, sema),
            // todo: Unsure if literal and ref_value should be iterable
            PrimaryExpr::Literal(lit) => std::iter::empty(),
            PrimaryExpr::RefValue { value } => std::iter::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InitExpr<'db> {
    pub kind: InitExprKind<'db>,

    pub id: AstId,

    pub scope_id: FileScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitExprKind<'db> {
    ArrayInit {
        values: Vec<InitExpr<'db>>,
    },
    ArrayIndexedElement {
        index: Numeric,
        values: Vec<InitExpr<'db>>,
    },
    StructInit {
        values: Vec<InitExpr<'db>>,
    },
    StructElement {
        name: Ident,
        value: Box<InitExpr<'db>>,
    },
    ConstantExpr(Expr<'db>),
}

impl ToProto<'_> for InitExpr<'_> {
    fn get_id(&'_ self, db: &'_ dyn crate::BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&'_ self, db: &'_ dyn crate::BaseDatabase) -> FileScopeId {
        self.scope_id
    }
}
