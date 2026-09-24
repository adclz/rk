use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::AddOperatorKind;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::expressions::expression::MultOperatorKind;
use crate::hir_def::expressions::spec::Spec;
use crate::hir_ty::body::Adjust;
use crate::hir_ty::body::Adjustment;
use crate::hir_ty::infer::table::InferSource;
use crate::hir_ty::ty::CallableType;
use crate::hir_ty::ty::Type;
use auto_lsp::lsp_types::CodeAction;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::lsp_types::WorkspaceEdit;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TypeError<'db> {
    NotAssignable {
        base_target: Type<'db>,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
        /// Whether a cast could be written where the value sits. False for
        /// an output binding: `o => d` names a destination, not an expression.
        suggest_cast: bool,
    },
    NotComparable {
        base_target: Type<'db>,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    NotAddable {
        base_target: Type<'db>,
        operator: AddOperatorKind,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    NotMultiplicable {
        base_target: Type<'db>,
        operator: MultOperatorKind,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    UnsupportedOperator {
        typ: Type<'db>,
        operator: &'static str,
        call_site: CallSite<'db>,
    },
    InferLiteralError {
        expr: Expr<'db>,
        source: Option<InferSource<'db>>,
        target: Type<'db>,
        err: InferLiteralError,
    },
    /// A `STRING[n]` whose length the compiler cannot work out. The length is
    /// part of the TYPE — it decides how many bytes the variable occupies — so
    /// one only the runtime knows leaves the layout unknowable, and silently
    /// taking the default 80 would size the storage wrongly with nothing said.
    StringLengthNotConstant {
        length: Expr<'db>,
    },
    FunctionAsType {
        expr: Spec<'db>,
        ty: Type<'db>,
    },
    DirectType {
        typ: Type<'db>,
        expr: CallSite<'db>,
    },
    AssignCallableType {
        typ: CallableType<'db>,
        access: CallSite<'db>,
    },
    /// A CLASS instance as an assignment target. It has no body, so it is not
    /// callable, and it used to be copied where a FUNCTION_BLOCK is refused.
    AssignClassInstance {
        class: Type<'db>,
        access: CallSite<'db>,
    },
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InferLiteralError {
    // Emitted by rust std library cast
    TypeMismatch(String),
    /// The literal is well-formed but outside what the type holds.
    OutOfRange {
        type_name: &'static str,
    },
    /// A minus sign on a literal for an unsigned type.
    NegativeUnsigned {
        type_name: &'static str,
    },

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

    Invalid_CHAR_Length(usize),
    Invalid_STRING_Length {
        max: u64,
        got: usize,
    },

    // Inner
    ExpectedNumber,
    InvalidNumber(String),
    /// Internal overflow when accumulating duration components in
    /// `i64` nanoseconds - only reachable for absurd input like
    /// `T#9999999d`.
    DurationOverflow,
    /// Literal's integer encoding is outside the type's representable
    /// range. Bounds are pre-formatted IEC literals (e.g.
    /// `T#-24d20h31m23s648ms`, `DT#1901-12-13-20:45:52`) hard-coded per
    /// type so the diagnostic note can show them verbatim.
    DurationOutOfRange {
        /// Source-level type name (e.g. "TIME", "DT").
        type_name: &'static str,
        /// Minimum value as an IEC literal string.
        min: &'static str,
        /// Maximum value as an IEC literal string.
        max: &'static str,
        /// True when the value exceeds `max` (overflow); false when
        /// below `min` (underflow).
        above_max: bool,
    },

    Invalid_TIME_Unit(String),
    Invalid_TIME_Components,

    Invalid_TOD_Format(String),
    Invalid_LTOD_Format(String),

    Invalid_DATE_Format(String),
    Invalid_LDATE_Format(String),

    Invalid_DT_Format(String),
    Invalid_LDT_Format(String),
}

impl<'db> ErrorCode for TypeError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::NotAssignable { .. } => "E0301",
            Self::NotComparable { .. } => "E0302",
            Self::NotAddable { .. } => "E0303",
            Self::NotMultiplicable { .. } => "E0304",
            Self::UnsupportedOperator { .. } => "E0305",
            Self::InferLiteralError { err, .. } => err.code(),
            Self::StringLengthNotConstant { .. } => "E0315",
            Self::FunctionAsType { .. } => "E0316",
            Self::DirectType { .. } => "E0317",
            Self::AssignCallableType { .. } => "E0318",
            Self::AssignClassInstance { .. } => "E0318",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::NotAssignable { .. } => "type mismatch",
            Self::NotComparable { .. } => "type mismatch",
            Self::NotAddable { .. } => "type mismatch",
            Self::NotMultiplicable { .. } => "type mismatch",
            Self::UnsupportedOperator { .. } => "type mismatch",
            Self::InferLiteralError { .. } => "invalid literal",
            Self::StringLengthNotConstant { .. } => "length is not constant",
            Self::FunctionAsType { .. } => "invalid type",
            Self::DirectType { .. } => "semantic violation",
            Self::AssignCallableType { .. } => "semantic violation",
            Self::AssignClassInstance { .. } => "semantic violation",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for TypeError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::NotAssignable {
                base_target,
                lhs: target,
                rhs: value,
                adjustment,
                expr,
                suggest_cast,
            } => {
                let message = if target.is_void() {
                    format!(
                        "'{}' is void and can not be assigned",
                        base_target.type_name(db)
                    )
                } else {
                    format!(
                        "expected '{}', got '{}'",
                        target.type_name(db),
                        adjustment_to_string(db, *value, adjustment),
                    )
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                base_target.with_location(db, &mut diag);
                if *suggest_cast {
                    explicit_cast_suggestion(db, *target, *value, *expr, &mut diag);
                }
                diag
            }
            Self::NotComparable {
                base_target,
                lhs,
                rhs,
                expr,
                adjustment,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can't compare '{}' with '{}'",
                        lhs.type_name(db),
                        adjustment_to_string(db, *rhs, adjustment),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                base_target.with_location(db, &mut diag);
                explicit_cast_suggestion(db, *lhs, *rhs, *expr, &mut diag);
                diag
            }
            Self::NotAddable {
                base_target,
                lhs,
                operator,
                rhs,
                expr,
                adjustment,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not {} '{}' with '{}'",
                        match operator {
                            AddOperatorKind::Plus => "add",
                            AddOperatorKind::Minus => "subtract",
                        },
                        lhs.type_name(db),
                        adjustment_to_string(db, *rhs, adjustment)
                    ))
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                base_target.with_location(db, &mut diag);
                explicit_cast_suggestion(db, *lhs, *rhs, *expr, &mut diag);
                diag
            }
            Self::NotMultiplicable {
                base_target,
                lhs,
                operator,
                rhs,
                expr,
                adjustment,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not {} '{}' with '{}'",
                        match operator {
                            MultOperatorKind::Mul => "multiply",
                            MultOperatorKind::Div => "divide",
                            MultOperatorKind::Mod => "modulus",
                        },
                        lhs.type_name(db),
                        adjustment_to_string(db, *rhs, adjustment)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                base_target.with_location(db, &mut diag);
                explicit_cast_suggestion(db, *lhs, *rhs, *expr, &mut diag);
                diag
            }
            Self::UnsupportedOperator {
                typ,
                operator,
                call_site,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "operator '{}' cannot be applied to type '{}'",
                        operator,
                        typ.type_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                typ.with_location(db, &mut diag);
                diag
            }
            Self::InferLiteralError {
                err,
                expr,
                source,
                target,
            } => {
                let target_name = target.type_name(db);
                let mut message = format!(
                    "cannot infer '{}' to '{}': {}",
                    expr.to_string(db),
                    target_name,
                    err
                );
                if let Some(shape) = err.shape(&target_name) {
                    message.push_str("; ");
                    message.push_str(&shape);
                }
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                if let Some(source) = source {
                    match source {
                        InferSource::Type(typ) => typ.with_location(db, &mut diag),
                        InferSource::CallSite(call) => diag.with_related(Related::new(
                            format!("'{}' is expected due to this", target.type_name(db)),
                            call.get_scope_id(db).file(db),
                            call.get_span(db),
                        )),
                    }
                }

                target.with_location(db, &mut diag);

                diag
            }
            Self::StringLengthNotConstant { length } => diag()
                .message("a STRING length must be known at compile time".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &length.get_span(db)).unwrap_or_default())
                .call(),
            Self::FunctionAsType { expr, ty } => diag()
                .message(format!(
                    "'{}' is a function and cannot be used as a variable or data type",
                    ty.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::DirectType { expr, typ } => diag()
                .message(format!(
                    "cannot use direct type '{}' here",
                    typ.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::AssignCallableType { typ, access } => diag()
                .message(format!(
                    "'{}' is a callable type and can not be assigned",
                    typ.get_name_ident(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                .call(),
            Self::AssignClassInstance { class, access } => diag()
                .message(format!(
                    "'{}' is a CLASS and can not be assigned",
                    class.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                .call(),
        }
    }
}

fn adjustment_to_string(
    db: &dyn WorkspaceDataBase,
    value: Type,
    adj: &Option<Adjustment>,
) -> String {
    match adj {
        Some(adj) => match adj.kind {
            Adjust::Ref => {
                format!("REF_TO {}", adj.target.type_name(db))
            }
            Adjust::Deref => {
                format!("DEREF {}", adj.target.type_name(db))
            }
            Adjust::Index => {
                format!("INDEX {}", adj.target.type_name(db))
            }
        },
        None => value.type_name(db),
    }
}

impl InferLiteralError {
    pub fn code(&self) -> &'static str {
        use InferLiteralError::*;
        match self {
            OutOfRange { .. } | DurationOverflow | DurationOutOfRange { .. } => "E0306",
            NegativeUnsigned { .. } => "E0307",
            TypeMismatch(_)
            | Invalid_BOOL_Literal
            | Invalid_UNSIGNED_8_BITS_Literal
            | Invalid_UNSIGNED_16_BITS_Literal
            | Invalid_UNSIGNED_32_BITS_Literal
            | Invalid_UNSIGNED_64_BITS_Literal
            | Invalid_SIGNED_8_BITS_Literal
            | Invalid_SIGNED_16_BITS_Literal
            | Invalid_SIGNED_32_BITS_Literal
            | Invalid_SIGNED_64_BITS_Literal
            | Invalid_REAL_Literal
            | Invalid_LREAL_Literal => "E0308",
            ExpectedNumber | InvalidNumber(_) | Invalid_TIME_Unit(_) | Invalid_TIME_Components => {
                "E0309"
            }
            Invalid_DATE_Format(_) | Invalid_LDATE_Format(_) => "E0310",
            Invalid_TOD_Format(_) | Invalid_LTOD_Format(_) => "E0311",
            Invalid_DT_Format(_) | Invalid_LDT_Format(_) => "E0312",
            Invalid_CHAR_Length(_) => "E0313",
            Invalid_STRING_Length { .. } => "E0314",
        }
    }

    /// What a correct literal looks like, for the end of the message. Short
    /// on purpose: the shape, not a tutorial.
    pub fn shape(&self, target: &str) -> Option<String> {
        use InferLiteralError::*;
        Some(match self {
            OutOfRange { type_name } => {
                let b = int_bounds(type_name)?;
                format!("{type_name} holds {b}")
            }
            DurationOutOfRange {
                type_name,
                min,
                max,
                ..
            } => {
                format!("{type_name} holds {min} to {max}")
            }
            DurationOverflow => return None,
            NegativeUnsigned { type_name } => match signed_twin(type_name) {
                Some(t) => format!("{type_name} is unsigned; use {t}, or drop the sign"),
                None => format!("{type_name} is unsigned; drop the sign"),
            },
            Invalid_BOOL_Literal => "BOOL is TRUE or FALSE".to_string(),
            Invalid_REAL_Literal | Invalid_LREAL_Literal => {
                format!("{target} takes a number, written like 3.14 or 1.0E3")
            }
            Invalid_UNSIGNED_8_BITS_Literal
            | Invalid_UNSIGNED_16_BITS_Literal
            | Invalid_UNSIGNED_32_BITS_Literal
            | Invalid_UNSIGNED_64_BITS_Literal
            | Invalid_SIGNED_8_BITS_Literal
            | Invalid_SIGNED_16_BITS_Literal
            | Invalid_SIGNED_32_BITS_Literal
            | Invalid_SIGNED_64_BITS_Literal => {
                format!("{target} takes a whole number, written like 42 or 16#2A")
            }
            TypeMismatch(_) => return None,
            ExpectedNumber | InvalidNumber(_) | Invalid_TIME_Unit(_) | Invalid_TIME_Components => {
                let prefix = if target.starts_with('L') { "LT" } else { "T" };
                format!("{target} is written {prefix}#1d2h3m4s5ms")
            }
            Invalid_DATE_Format(_) => "DATE is written D#2025-01-31".to_string(),
            Invalid_LDATE_Format(_) => "LDATE is written LD#2025-01-31".to_string(),
            Invalid_TOD_Format(_) => "TOD is written TOD#12:30:00.500".to_string(),
            Invalid_LTOD_Format(_) => "LTOD is written LTOD#12:30:00.500".to_string(),
            Invalid_DT_Format(_) => "DT is written DT#2025-01-31-12:30:00".to_string(),
            Invalid_LDT_Format(_) => "LDT is written LDT#2025-01-31-12:30:00".to_string(),
            Invalid_CHAR_Length(_) => "CHAR is one character, written 'a'".to_string(),
            Invalid_STRING_Length { got, .. } => {
                format!("declare it STRING[{got}], or shorten the literal")
            }
        })
    }
}

fn int_bounds(type_name: &str) -> Option<&'static str> {
    Some(match type_name {
        "SINT" => "-128 to 127",
        "INT" => "-32768 to 32767",
        "DINT" => "-2147483648 to 2147483647",
        "LINT" => "-9223372036854775808 to 9223372036854775807",
        "USINT" | "BYTE" => "0 to 255",
        "UINT" | "WORD" => "0 to 65535",
        "UDINT" | "DWORD" => "0 to 4294967295",
        "ULINT" | "LWORD" => "0 to 18446744073709551615",
        "REAL" => "whole numbers from -2147483648 to 2147483647",
        "LREAL" => "whole numbers from -9223372036854775808 to 9223372036854775807",
        _ => return None,
    })
}

fn signed_twin(type_name: &str) -> Option<&'static str> {
    Some(match type_name {
        "USINT" => "SINT",
        "UINT" => "INT",
        "UDINT" => "DINT",
        "ULINT" => "LINT",
        _ => return None,
    })
}

impl std::fmt::Display for InferLiteralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            InferLiteralError::TypeMismatch(st) => return f.write_str(st),
            InferLiteralError::OutOfRange { type_name } => {
                return write!(f, "the value does not fit in {type_name}");
            }
            InferLiteralError::NegativeUnsigned { type_name } => {
                return write!(f, "{type_name} cannot be negative");
            }

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

            InferLiteralError::Invalid_CHAR_Length(len) => {
                return write!(f, "CHAR literal must be exactly 1 character, got {len}");
            }
            InferLiteralError::Invalid_STRING_Length { max, got } => {
                return write!(
                    f,
                    "STRING literal exceeds the capacity of {max} bytes, got {got}"
                );
            }

            InferLiteralError::ExpectedNumber => "expected number",
            InferLiteralError::InvalidNumber(st) => return f.write_str(st),
            InferLiteralError::DurationOverflow => "duration overflow",
            InferLiteralError::DurationOutOfRange {
                type_name,
                above_max,
                ..
            } => {
                return if *above_max {
                    write!(f, "{type_name} value exceeds the supported maximum")
                } else {
                    write!(f, "{type_name} value is below the supported minimum")
                };
            }

            InferLiteralError::Invalid_TIME_Unit(st) => return f.write_str(st),
            InferLiteralError::Invalid_TIME_Components => "invalid TIME components",
            InferLiteralError::Invalid_TOD_Format(st) => return f.write_str(st),
            InferLiteralError::Invalid_LTOD_Format(st) => return f.write_str(st),

            InferLiteralError::Invalid_DATE_Format(st) => return f.write_str(st),
            InferLiteralError::Invalid_LDATE_Format(st) => return f.write_str(st),

            InferLiteralError::Invalid_DT_Format(st) => return f.write_str(st),
            InferLiteralError::Invalid_LDT_Format(st) => return f.write_str(st),
        };
        f.write_str(msg)
    }
}

fn explicit_cast_suggestion(
    db: &dyn WorkspaceDataBase,
    expected: Type,
    actual: Type,
    actual_site: CallSite,
    diag: &mut IdeDiagnostic,
) {
    if let (Type::Elementary(lhs), Type::Elementary(rhs)) =
        (expected.normalize(db), actual.normalize(db))
        && lhs.explicit_cast(rhs)
    {
        let value = actual_site.to_string(db);
        diag.with_related(Related::new(
            format!(
                "consider explicitly casting with '{}_TO_{}({})'",
                rhs.type_name(),
                lhs.type_name(),
                value
            ),
            actual_site.get_scope_id(db).file(db),
            actual_site.get_span(db),
        ));

        // The edit the title promises. It used to carry an empty
        // `WorkspaceEdit`, so the action appeared, applied nothing, and left
        // the reader to write the call out themselves.
        let file = actual_site.get_scope_id(db).file(db);
        let cast = format!("{}_TO_{}({})", rhs.type_name(), lhs.type_name(), value);
        let range = crate::denormalize(db, file, &actual_site.get_span(db)).unwrap_or_default();

        diag.with_fix(CodeAction {
            title: format!("insert explicit cast '{cast}'"),
            edit: Some(WorkspaceEdit::new(HashMap::from([(
                file.url(db).clone(),
                vec![auto_lsp::lsp_types::TextEdit {
                    range,
                    new_text: cast,
                }],
            )]))),
            is_preferred: Some(true),
            ..Default::default()
        });
    }
}
