use crate::completions::snippets::elem_type_names_init;
use crate::hir::interned::identifier::Ident;
use crate::hir::scopes::scope::{ScopeId};
use crate::hir::semantic_index::SemanticIndex;
use crate::to_proto::{self_iter, IterToProto, ToProto};
use auto_enums::auto_enum;
use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;

#[salsa::tracked(debug)]
pub struct Expr<'db> {
    #[returns(ref)]
    pub span: Span,

    #[returns(ref)]
    pub expr: ExprKind<'db>,

    pub scope_id: ScopeId,
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
    Literal(AnyElementary), // constant
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct PathExpr<'db> {
    pub span: Span,

    pub expr: PathExprKind<'db>,
}

impl<'db> ToProto<'db> for PathExpr<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        &self.span
    }
}

pub struct PathExprIterator<'a> {
    current: Option<&'a PathExprKind<'a>>,
}

impl<'db> IntoIterator for &'db PathExpr<'db> {
    type Item = &'db PathExprKind<'db>;
    type IntoIter = PathExprIterator<'db>;

    fn into_iter(self) -> Self::IntoIter {
        PathExprIterator {
            current: Some(&self.expr),
        }
    }
}

impl<'a> Iterator for PathExprIterator<'a> {
    type Item = &'a PathExprKind<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = self.current.take() {
            match &current {
                PathExprKind::Field(field) => {
                    self.current = Some(&field.path);
                }
                PathExprKind::Index(index) => {
                    self.current = Some(&index.path);
                }
                PathExprKind::VarAccess(var_access) => {
                    self.current = None;
                }
            }
        }
        self.current
    }
}

impl<'db> IterToProto<'db> for PathExpr<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match &self.expr {
            PathExprKind::Field(field) => self_iter(self),
            PathExprKind::Index(index) => self_iter(self),
            PathExprKind::VarAccess(var_access) => self_iter(self),
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
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        match &self.kind {
            VariableAccessKind::Direct { adress, .. } => self_iter(self),
            VariableAccessKind::Symbolic(symbolic) => self_iter(self),
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

// Generic data types Generic data types
// Groups of elementary data types
// ANY
// |_ ANY_DERIVED
// |_ ANY_ELEMENTARY
//    |_ ANY_MAGNITUDE
//       |_ ANY_NUM
//          |_ ANY_REAL -  REAL, LREAL
//          |_ ANY_INT
//              |_ ANY_UNSIGNED - USINT, UINT, UDINT, ULINT
//              |_ ANY_SIGNED - SINT, INT, DINT, LINT
//       |_ ANY_DURATION - TIME, LTIME
//    |_ ANY_BIT - BOOL, BYTE, WORD, DWORD, LWORD
//    |_ ANY_CHARS
//       |_ ANY_STRING - STRING, WSTRING
//       |_ ANY_CHAR - CHAR, WCHAR
//    |_ ANY_DATE - DATE_AND_TIME, LDT, DATE, TIME_OF_DAY, LTOD, LDATE*
//
// Notes:
//
// - All ANY_UNSIGNED, ANY_INT and ANY_BIT (except for bool) can be represented as octal, decimal or hexadecimal - hence the Numeric enum.
// - ANY_INT and ANY_REAL have an "Infer" variant to represent unspecified types which have to be inferred later.
// * (LDATE is not present in the spec ?)

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyElementary {
    AnyMagnitude(AnyMagnitude),
    AnyBit(AnyBit),
    AnyChars(AnyChars),
    AnyDate(AnyDate),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyMagnitude {
    AnyNum(AnyNum),
    AnyDuration(AnyDuration),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyNum {
    AnyReal(AnyReal),
    AnyInt(AnyInt),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyReal {
    Real(Ident),
    LReal(Ident),
    Infer(Ident), // Represents an unspecified real type
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyInt {
    AnySigned(AnySigned),
    AnyUnsigned(AnyUnsigned),
    Infer(Numeric), // Represents an unspecified numeric type
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnySigned {
    SInt(Numeric),
    Int(Numeric),
    DInt(Numeric),
    LInt(Numeric),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyUnsigned {
    USInt(Numeric),
    UInt(Numeric),
    UDInt(Numeric),
    ULInt(Numeric),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyDuration {
    Time(Ident),
    LTime(Ident),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyBit {
    Bool(Ident),
    Byte(Numeric),
    Word(Numeric),
    DWord(Numeric),
    LWord(Numeric),
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

    pub fn to_string<'db>(&self, db: &'db dyn BaseDatabase) -> String {
        match self.kind(db) {
            NumericKind::Binary => format!("[Binary] {}", self.ident(db).text(db)),
            NumericKind::Hex => format!("[Hexa] {}", self.ident(db).text(db)),
            NumericKind::Octal => format!("[Octal] {}", self.ident(db).text(db)),
            NumericKind::Signed => self.ident(db).text(db).to_string(),
        }
    }
}

// todo: improve support for string and char
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyChars {
    AnyString(Ident),
    AnyChar(Ident),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum AnyDate {
    DateAndTime(Ident),
    LDateTime(Ident),
    LDate(Ident),
    Date(Ident),
    TimeOfDay(Ident),
    LTod(Ident),
}

impl AnyElementary {
    pub fn to_string<'db>(&self, db: &'db dyn BaseDatabase) -> String {
        match self {
            Self::AnyBit(n) => match n {
                AnyBit::Bool(ident) => format!("Bool: {}", ident.text(db)),
                AnyBit::Byte(ident) => format!("Byte: {}", ident.to_string(db)),
                AnyBit::Word(ident) => format!("Word: {}", ident.to_string(db)),
                AnyBit::DWord(ident) => format!("DWord: {}", ident.to_string(db)),
                AnyBit::LWord(ident) => format!("Lword: {}", ident.to_string(db)),
            },
            Self::AnyMagnitude(n) => match n {
                AnyMagnitude::AnyNum(n) => match n {
                    AnyNum::AnyReal(n) => match n {
                        AnyReal::Real(ident) => format!("Real: {}", ident.text(db)),
                        AnyReal::LReal(ident) => format!("LReal: {}", ident.text(db)),
                        AnyReal::Infer(ident) => format!("{}", ident.text(db)),
                    },
                    AnyNum::AnyInt(n) => match n {
                        AnyInt::AnySigned(n) => match n {
                            AnySigned::SInt(ident) => format!("SInt: {}", ident.to_string(db)),
                            AnySigned::Int(ident) => format!("Int: {}", ident.to_string(db)),
                            AnySigned::DInt(ident) => format!("DInt: {}", ident.to_string(db)),
                            AnySigned::LInt(ident) => format!("LInt: {}", ident.to_string(db)),
                        },
                        AnyInt::AnyUnsigned(ident) => match ident {
                            AnyUnsigned::USInt(ident) => format!("USInt: {}", ident.to_string(db)),
                            AnyUnsigned::UInt(ident) => format!("UInt: {}", ident.to_string(db)),
                            AnyUnsigned::UDInt(ident) => format!("UDInt: {}", ident.to_string(db)),
                            AnyUnsigned::ULInt(ident) => format!("ULInt: {}", ident.to_string(db)),
                        },
                        AnyInt::Infer(ident) => format!("{}", ident.to_string(db)),
                    },
                },
                AnyMagnitude::AnyDuration(n) => match n {
                    AnyDuration::Time(ident) => format!("Time: {}", ident.text(db)),
                    AnyDuration::LTime(ident) => format!("LTime: {}", ident.text(db)),
                },
            },
            Self::AnyChars(n) => match n {
                AnyChars::AnyString(ident) => format!("String: {}", ident.text(db)),
                AnyChars::AnyChar(ident) => format!("Char: {}", ident.text(db)),
            },
            Self::AnyDate(n) => match n {
                AnyDate::DateAndTime(ident) => format!("Date and Time: {}", ident.text(db)),
                AnyDate::LDateTime(ident) => format!("Long Date and Time: {}", ident.text(db)),
                AnyDate::Date(ident) => format!("Date: {}", ident.text(db)),
                AnyDate::LDate(ident) => format!("Long Date: {}", ident.text(db)),
                AnyDate::TimeOfDay(ident) => format!("Time of Day: {}", ident.text(db)),
                AnyDate::LTod(ident) => format!("Long Time of Day: {}", ident.text(db)),
            },
        }
    }
}

impl<'db> ToProto<'db> for Expr<'db> {
    fn get_span(&'db self, db: &'db dyn BaseDatabase) -> &'db Span {
        self.span(db)
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
