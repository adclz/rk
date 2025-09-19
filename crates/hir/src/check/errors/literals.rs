use std::{error::Error, fmt::Display, sync::Arc};

use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, diag};

use crate::{
    HirNodeInfo,
    check::errors::{
        sem_errors::{AnalysisError, ToIdeDiagnostic},
        stmt::StmtError,
        utils::get_decl_and_def_for_ty,
    },
    hir_ty::{expr_resolver::ResolvedExpr, ty::Ty},
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct LiteralError<'db> {
    pub ty: Ty<'db>,
    pub expr: ResolvedExpr<'db>,
    pub kind: LiteralErrorKind,
}

impl<'db> LiteralError<'db> {
    pub fn new(ty: Ty<'db>, expr: ResolvedExpr<'db>, kind: LiteralErrorKind) -> Self {
        Self { ty, expr, kind }
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralErrorKind {
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

impl<'db> From<LiteralError<'db>> for AnalysisError<'db> {
    fn from(err: LiteralError<'db>) -> Self {
        AnalysisError::LiteralError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for LiteralError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        let mut diag = match self.kind {
            LiteralErrorKind::Invalid_BOOL_Literal => {
                let mut diag = diag()
                    .message(format!("invalid BOOL literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A BOOL literal must be either 0, 1, TRUE or FALSE".to_string());

                diag
            }
            LiteralErrorKind::Invalid_UNSIGNED_8_BITS_Literal => {
                let mut diag = diag()
                    .message(format!("invalid USINT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A USINT literal must be an integer between 0 and 255".to_string());

                diag
            }
            LiteralErrorKind::Invalid_UNSIGNED_16_BITS_Literal => {
                let mut diag = diag()
                    .message(format!("invalid UINT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A UINT literal must be an integer between 0 and 65535".to_string());

                diag
            }
            LiteralErrorKind::Invalid_UNSIGNED_32_BITS_Literal => {
                let mut diag = diag()
                    .message(format!("invalid UDINT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A UDINT literal must be an integer between 0 and 4294967295".to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_UNSIGNED_64_BITS_Literal => {
                let mut diag = diag()
                    .message(format!("invalid ULINT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A ULINT literal must be an integer between 0 and 18446744073709551615"
                        .to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_SIGNED_8_BITS_Literal => {
                let mut diag = diag()
                    .message(format!("invalid SINT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A SINT literal must be an integer between -128 and 127".to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_SIGNED_16_BITS_Literal => {
                let mut diag = diag()
                    .message(format!("invalid INT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "An INT literal must be an integer between -32768 and 32767".to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_SIGNED_32_BITS_Literal => {
                let mut diag = diag()
                    .message(format!("invalid DINT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();
                diag.with_note(
                    "A DINT literal must be an integer between -2147483648 and 2147483647"
                        .to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_SIGNED_64_BITS_Literal => {
                let mut diag = diag()
                    .message(format!("invalid LINT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A LINT literal must be an integer between -9223372036854775808 and 9223372036854775807".to_string());
                diag
            }
            LiteralErrorKind::Invalid_REAL_Literal => {
                let mut diag = diag()
                    .message(format!("invalid REAL literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A REAL literal must be a floating point number between -3.402823E+38 and 3.402823E+38".to_string());
                diag
            }
            LiteralErrorKind::Invalid_LREAL_Literal => {
                let mut diag = diag()
                    .message(format!("invalid LREAL literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A LREAL literal must be a floating point number between -1.79769313486232E+308 and 1.79769313486232E+308".to_string());
                diag
            }
            LiteralErrorKind::Invalid_TIME_Literal => {
                let mut diag = diag()
                    .message(format!("invalid TIME literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A TIME literal must be a duration in the format 't'hh:mm:ss.sss'".to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_LTIME_Literal => {
                let mut diag = diag()
                    .message(format!("invalid LTIME literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A LTIME literal must be a duration in the format 't'hh:mm:ss.sss'".to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_DATE_Literal => {
                let mut diag = diag()
                    .message(format!("invalid DATE literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A DATE literal must be a date in the format 'd'yyyy-mm-dd'".to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_LDATE_Literal => {
                let mut diag = diag()
                    .message(format!("invalid LDATE literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A LDATE literal must be a date in the format 'd'yyyy-mm-dd'".to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_TOD_Literal => {
                let mut diag = diag()
                    .message(format!("invalid TOD literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A TOD literal must be a time of day in the format 'tod'hh:mm:ss.sss'"
                        .to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_LTOD_Literal => {
                let mut diag = diag()
                    .message(format!("invalid LTOD literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A LTOD literal must be a time of day in the format 'tod'hh:mm:ss.sss'"
                        .to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_DT_Literal => {
                let mut diag = diag()
                    .message(format!("invalid DT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A DT literal must be a date and time in the format 'dt'yyyy-mm-dd-hh:mm:ss.sss'".to_string());
                diag
            }
            LiteralErrorKind::Invalid_LDT_Literal => {
                let mut diag = diag()
                    .message(format!("invalid LDT literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A LDT literal must be a date and time in the format 'dt'yyyy-mm-dd-hh:mm:ss.sss'".to_string());
                diag
            }
            LiteralErrorKind::Invalid_STRING_Literal => {
                let mut diag = diag()
                    .message(format!("invalid STRING literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note(
                    "A STRING literal must be a sequence of characters enclosed in double quotes"
                        .to_string(),
                );
                diag
            }
            LiteralErrorKind::Invalid_WSTRING_Literal => {
                let mut diag = diag()
                    .message(format!("invalid WSTRING literal",))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag.with_note("A WSTRING literal must be a sequence of wide characters enclosed in double quotes".to_string());
                diag
            }
            LiteralErrorKind::TypeMismatch(ref msg) => {
                let mut diag = diag()
                    .message(msg.clone())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(self.expr.get_span(db))
                    .call();

                diag
            }
            _ => todo!(),
        };

        get_decl_and_def_for_ty(db, self.ty, &mut diag);

        diag
    }
}
