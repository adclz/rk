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

impl InferLiteralError {
    pub fn to_string(&self) -> String {
        match self {
            InferLiteralError::TypeMismatch(st) => return st.to_owned(),

            InferLiteralError::Invalid_BOOL_Literal => "invalid boolean literal",
            InferLiteralError::Invalid_UNSIGNED_8_BITS_Literal => "invalid USINT literal",
            InferLiteralError::Invalid_UNSIGNED_16_BITS_Literal => "invalid UINT literal",
            InferLiteralError::Invalid_UNSIGNED_32_BITS_Literal => "invalid UDINT literal",
            InferLiteralError::Invalid_UNSIGNED_64_BITS_Literal => "invalid ULINT literal",

            InferLiteralError::Invalid_SIGNED_8_BITS_Literal => "invalid SINT literal",
            InferLiteralError::Invalid_SIGNED_16_BITS_Literal => "invalid INT literal",
            InferLiteralError::Invalid_SIGNED_32_BITS_Literal => "invalid DINT literal",
            InferLiteralError::Invalid_SIGNED_64_BITS_Literal => "invalid LINT literal",

            InferLiteralError::Invalid_REAL_Literal => "invalid REAL literal",
            InferLiteralError::Invalid_LREAL_Literal => "invalid LREAL literal",

            InferLiteralError::Invalid_TIME_Literal => "invalid TIME literal",
            InferLiteralError::Invalid_LTIME_Literal => "invalid LTIME literal",

            InferLiteralError::Invalid_DATE_Literal => "invalid DATE literal",
            InferLiteralError::Invalid_LDATE_Literal => "invalid LDATE literal",

            InferLiteralError::Invalid_TOD_Literal => "invalid TOD literal",
            InferLiteralError::Invalid_LTOD_Literal => "invalid LTOD literal",

            InferLiteralError::Invalid_DT_Literal => "invalid DT literal",
            InferLiteralError::Invalid_LDT_Literal => "invalid LDT literal",

            InferLiteralError::Invalid_STRING_Literal => "invalid STRING literal",
            InferLiteralError::Invalid_WSTRING_Literal => "invalid WSTRING literal",

            InferLiteralError::ExpectedNumber => "expected number",
            InferLiteralError::InvalidNumber(st) => return st.to_owned(),
            InferLiteralError::DurationOverflow => "duration overflow",

            InferLiteralError::Invalid_TIME_Unit(st) => return st.to_owned(),
            InferLiteralError::Invalid_TIME_Components => "invalid TIME components",

            InferLiteralError::Invalid_TOD_Format(st) => return st.to_owned(),
            InferLiteralError::Invalid_LTOD_Format(st) => return st.to_owned(),

            InferLiteralError::Invalid_DATE_Format(st) => return st.to_owned(),
            InferLiteralError::Invalid_LDATE_Format(st) => return st.to_owned(),

            InferLiteralError::Invalid_DT_Format(st) => return st.to_owned(),
            InferLiteralError::Invalid_LDT_Format(st) => return st.to_owned(),

            InferLiteralError::Incomplete_STRING_XX_Escape => {
                "incomplete STRING XX escape sequence"
            }
            InferLiteralError::Invalid_STRING_Hex_Escape => "invalid STRING hex escape sequence",
            InferLiteralError::Invalid_STRING_CHAR(st) => return st.to_owned(),

            InferLiteralError::Incomplete_WSTRING_XXXX_Escape => {
                "incomplete WSTRING XXXX escape sequence"
            }
            InferLiteralError::Invalid_WSTRING_Hex_Escape(st) => return st.to_owned(),
            InferLiteralError::Invalid_WSTRING_Unicode_Scalar(st) => return st.to_owned(),
        }
        .to_string()
    }
}
