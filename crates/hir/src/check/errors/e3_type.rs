use std::collections::HashMap;

use auto_lsp::lsp_types::{CodeAction, DiagnosticSeverity, WorkspaceEdit};
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::hir_def::expressions::expression::{ExprKind, PrimaryExpr};
use crate::hir_def::{pous::pou::Pou, scope::ScopeKind, semantic_index::get_scope};
use crate::hir_ty::infer::const_eval;
use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::expression::{AddOperatorKind, Expr, MultOperatorKind},
        pous::variable::VariableDecl,
    },
    hir_ty::{
        body::{Adjust, Adjustment},
        infer::table::InferSource,
        ty::Type,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TypeError<'db> {
    /// A `STRING[n]` whose length the compiler cannot work out. The length is
    /// part of the TYPE — it decides how many bytes the variable occupies — so
    /// one only the runtime knows leaves the layout unknowable, and silently
    /// taking the default 80 would size the storage wrongly with nothing said.
    StringLengthNotConstant { length: Expr<'db> },
    /// A once-per-type initializer (a TYPE default, an FB/CLASS member
    /// default, a static PROGRAM field or config global) referenced something
    /// with no compile-time value. Before this code the leaf was silently
    /// DROPPED: the slot read zero from source the check called clean.
    InitNotConstant { value: Expr<'db> },
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
    NotMultiplicable {
        base_target: Type<'db>,
        operator: MultOperatorKind,
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
    InferLiteralError {
        expr: Expr<'db>,
        source: Option<InferSource<'db>>,
        target: Type<'db>,
        err: InferLiteralError,
    },
    NonVariadicFoldParameter {
        var: VariableDecl<'db>,
        call_site: CallSite<'db>,
    },
    UnsupportedOperator {
        typ: Type<'db>,
        operator: &'static str,
        call_site: CallSite<'db>,
    },
}

impl<'db> ErrorCode for TypeError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::NotAssignable { .. } => "E0301",
            Self::NotComparable { .. } => "E0302",
            Self::NotAddable { .. } => "E0303",
            Self::NotMultiplicable { .. } => "E0304",
            Self::InferLiteralError { .. } => "E0309",
            Self::NonVariadicFoldParameter { .. } => "E0317",
            Self::UnsupportedOperator { .. } => "E0318",
            Self::StringLengthNotConstant { .. } => "E0319",
            Self::InitNotConstant { .. } => "E0320",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InferLiteralError { .. } => "invalid literal",
            Self::StringLengthNotConstant { .. } => "length is not constant",
            Self::InitNotConstant { .. } => "initial value is not constant",
            _ => "type mismatch",
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
            Self::StringLengthNotConstant { length } => diag()
                .message("a STRING length must be known at compile time".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &length.get_span(db)).unwrap_or_default())
                .call(),
            Self::InitNotConstant { value } => {
                let mut d = diag()
                    .message(
                        "this initial value must be a constant: it is fixed before the program runs"
                            .to_string(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                    .call();
                // Say WHY when the refused thing is a bare name — especially
                // when it IS a constant, just not one this scope can fold.

                if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = value.expr(db) {
                    use crate::Qualifier;
                    match const_eval::spec_name_binding(db, *va) {
                        Some(decl) if decl.qualifier(db).contains(Qualifier::CONSTANT) => {
                            // Three reasons a CONSTANT still refuses, told apart
                            // so the advice is not a catch-all.
                            if decl.init(db).is_none() {
                                d.with_note(format!(
                                    "'{}' is CONSTANT but declares no initial value, \
                                     so there is nothing to fold",
                                    decl.name(db).text(db)
                                ));
                            } else {
                                d.with_note(format!(
                                    "'{}' is CONSTANT, but its own value does not fold \
                                     (a reference cycle, or a non-constant initializer)",
                                    decl.name(db).text(db)
                                ));
                            }
                        }
                        Some(decl) => {
                            d.with_note(format!(
                                "'{}' is an ordinary variable; declare it CONSTANT \
                                 if its value never changes",
                                decl.name(db).text(db)
                            ));
                        }
                        None => {
                            if let Some(ident) = const_eval::bare_access_name(db, *va)
                                && let Some(global) =
                                    crate::hir_ty::index_graphs::external_var_lookup(db, ident)
                            {
                                if !global.qualifier(db).contains(Qualifier::CONSTANT) {
                                    d.with_note(format!(
                                        "'{}' is an ordinary variable; declare it CONSTANT \
                                         if its value never changes",
                                        ident.text(db)
                                    ));
                                    return d;
                                }
                                let in_type = matches!(
                                    get_scope(db, value.get_scope_id(db)).kind,
                                    ScopeKind::Pou(Pou::DataType(_))
                                );
                                if in_type {
                                    d.with_note(format!(
                                        "'{}' IS a CONSTANT, but a TYPE declaration cannot \
                                         see it: a TYPE default folds only literals, \
                                         arithmetic, and constants in its own scope",
                                        ident.text(db)
                                    ));
                                } else {
                                    d.with_note(format!(
                                        "'{}' IS a CONSTANT: declare it in this POU as \
                                         `VAR_EXTERNAL CONSTANT` and the reference folds",
                                        ident.text(db)
                                    ));
                                }
                            }
                        }
                    }
                }
                d
            }
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
            Self::NonVariadicFoldParameter { var, call_site } => {
                let mut diag = diag()
                    .message(format!(
                        "variable '{}' is not variadic",
                        var.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_note("... can only be used on VAR_INPUT variables that are declared variadic with the same operator (e.g: INT...)".into());
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
                let mut diag = diag()
                    .message(format!(
                        "cannot infer '{}' to '{}': {}",
                        expr.to_string(db),
                        target.type_name(db),
                        err
                    ))
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

                // For range-overflow errors, attach a note showing the
                // valid integer-encoding range so the user knows why a
                // literal was rejected and where the cutoff sits.
                if let InferLiteralError::DurationOutOfRange {
                    type_name,
                    min,
                    max,
                    ..
                } = err
                {
                    diag.with_note(format!("valid range for {type_name}: {min} to {max}"));
                }

                target.with_location(db, &mut diag);

                diag
            }
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

    Incomplete_STRING_XX_Escape,
    Invalid_STRING_Hex_Escape,
}

impl std::fmt::Display for InferLiteralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            InferLiteralError::TypeMismatch(st) => return f.write_str(st),

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

            InferLiteralError::Incomplete_STRING_XX_Escape => {
                "incomplete STRING XX escape sequence"
            }
            InferLiteralError::Invalid_STRING_Hex_Escape => "invalid STRING hex escape sequence",
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
