use crate::{hir_def::expressions::expression::Expr, hir_ty::ty::Ty};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct LiteralError<'db> {
    pub ty: Ty<'db>,
    pub expr: Expr<'db>,
    pub kind: InferLiteralError,
}

impl<'db> LiteralError<'db> {
    pub fn new(ty: Ty<'db>, expr: Expr<'db>, kind: InferLiteralError) -> Self {
        Self { ty, expr, kind }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InferLiteralError {
    // Emitted by rust std library cast
    TypeMismatch(String),

    Invalid_BOOL_Literal,
    Invalid_UNSIGNED_8_BITS_Literal,
    Invalid_UNSIGNED_16_BITS_Literal,
    Invalid_UNSIGNED_32_BITS_Literal,
    Invalid_UNSIGNED_64_BITS_Literal,

    Invalid_SIGNED_8_BITS_Literal,
    Invalid_SIGNED_16_BITS_Literal,
    Invalid_SIGNED_32_BITS_Literal,
    Invalid_SIGNED_64_BITS_Literal,

    Invalid_REAL_Literal,
    Invalid_LREAL_Literal,

    Invalid_TIME_Literal,
    Invalid_LTIME_Literal,

    Invalid_DATE_Literal,
    Invalid_LDATE_Literal,

    Invalid_TOD_Literal,
    Invalid_LTOD_Literal,

    Invalid_DT_Literal,
    Invalid_LDT_Literal,

    Invalid_STRING_Literal,
    Invalid_WSTRING_Literal,

    // Inner
    ExpectedNumber,
    InvalidNumber(String),
    DurationOverflow,

    Invalid_TIME_Unit(String),
    Invalid_TIME_Components,

    Invalid_TOD_Format(String),
    Invalid_LTOD_Format(String),

    Invalid_DATE_Format(String),
    Invalid_LDATE_Format(String),

    Invalid_DT_Format(String),
    Invalid_LDT_Format(String),

    Incomplete_STRING_XX_Escape,
    Invalid_STRING_Hex_Escape,
    Invalid_STRING_CHAR(String),

    Incomplete_WSTRING_XXXX_Escape,
    Invalid_WSTRING_Hex_Escape(String),
    Invalid_WSTRING_Unicode_Scalar(String),
}
