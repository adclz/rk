use std::collections::HashMap;

use auto_lsp::lsp_types::{CodeAction, DiagnosticSeverity, WorkspaceEdit};
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo, check::errors::analysis_error::ToIdeDiagnostic, hir_def::{
        expressions::{
            expression::{AddOperatorKind, Expr, FoldOperatorKind, MultOperatorKind},
            spec::Spec,
        }, interned::identifier::Ident, pous::{generics::{AnyGeneric, GenericParam}, variable::VariableDecl}
    }, hir_ty::{
        body::{Adjust, Adjustment},
        infer::{Infer, table::InferSource},
        ty::Type,
    }
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TypeError<'db> {
    NotAssignable {
        base_target: Type<'db>,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
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
    NotPowerable {
        base_target: Type<'db>,
        lhs: Type<'db>,
        rhs: Type<'db>,
        adjustment: Option<Adjustment<'db>>,
        expr: CallSite<'db>,
    },
    NotABoolean {
        typ: Type<'db>,
        expr: Expr<'db>,
    },
    Other {
        message: String,
        expr: Expr<'db>,
    },
    InferLiteralError {
        expr: Expr<'db>,
        source: Option<InferSource<'db>>,
        target: Type<'db>,
        err: InferLiteralError,
    },
    InvalidGenericType {
        param: GenericParam<'db>,
    },
    UnknownGenericConstraint {
        param: GenericParam<'db>,
        constraint: Spec<'db>,
    },
    InvalidGenericConstraint {
        param: GenericParam<'db>,
        constraint: Spec<'db>,
    },
    MissingTypeArguments {
        func_name: Ident,
        call_site: CallSite<'db>,
    },
    WrongTypeArgumentArity {
        func_name: Ident,
        expected: usize,
        actual: usize,
        call_site: CallSite<'db>,
    },
    TypeArgumentConstraintMismatch {
        concrete_type: Type<'db>,
        param_name: Ident,
        constraint: AnyGeneric,
        call_site: CallSite<'db>,
    },
    TypeArgumentIntoConstraintMismatch {
        type_arg: Type<'db>,
        into_target: Type<'db>,
        param_name: Ident,
        call_site: CallSite<'db>,
    },
    NonVariadicFoldParameter {
        var: VariableDecl<'db>,
        call_site: CallSite<'db>,
    },
    NonNumericFoldParameter {
        typ: Type<'db>,
        operator: FoldOperatorKind,
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
            Self::NotPowerable { .. } => "E0305",
            Self::NotABoolean { .. } => "E0306",
            Self::InferLiteralError { .. } => "E0309",
            Self::InvalidGenericType { .. } => "E0310",
            Self::UnknownGenericConstraint { .. } => "E0311",
            Self::InvalidGenericConstraint { .. } => "E0312",
            Self::MissingTypeArguments { .. } => "E0313",
            Self::WrongTypeArgumentArity { .. } => "E0314",
            Self::TypeArgumentConstraintMismatch { .. } => "E0315",
            Self::TypeArgumentIntoConstraintMismatch { .. } => "E0316",
            Self::NonVariadicFoldParameter { .. } => "E0317",
            Self::NonNumericFoldParameter { .. } => "E0318",
            Self::Other { .. } => "E0350",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::InferLiteralError { .. } => "invalid literal",
            Self::MissingTypeArguments { .. } => "missing type arguments",
            Self::WrongTypeArgumentArity { .. } => "wrong number of type arguments",
            Self::TypeArgumentConstraintMismatch { .. } => "type argument constraint mismatch",
            Self::TypeArgumentIntoConstraintMismatch { .. } => {
                "type argument INTO constraint mismatch"
            }
            _ => "type mismatch",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for TypeError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::NotAssignable {
                base_target,
                lhs: target,
                rhs: value,
                adjustment,
                expr,
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
                    .range(expr.get_span(db))
                    .call();

                base_target.with_location(db, &mut diag);
                explicit_cast_suggestion(db, *target, *value, *expr, &mut diag);
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
                    .range(expr.get_span(db))
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
                    .range(expr.get_span(db))
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
                        rhs.type_name(db),
                        adjustment_to_string(db, *rhs, adjustment)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(expr.get_span(db))
                    .call();

                base_target.with_location(db, &mut diag);
                explicit_cast_suggestion(db, *lhs, *rhs, *expr, &mut diag);
                diag
            }
            Self::NotPowerable {
                base_target,
                lhs,
                rhs,
                adjustment,
                expr,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "can not power '{}' with '{}'",
                        lhs.type_name(db),
                        adjustment_to_string(db, *rhs, adjustment)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(expr.get_span(db))
                    .call();

                base_target.with_location(db, &mut diag);
                explicit_cast_suggestion(db, *lhs, *rhs, *expr, &mut diag);
                diag
            }
            Self::NotABoolean { typ, expr } => diag()
                .message(format!("expected a boolean, got {}", typ.type_name(db)))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(expr.get_span(db))
                .call(),
            Self::InvalidGenericType { param } => diag()
                .message(format!(
                    "generic '{}' has invalid type '{}'",
                    param.name(db).text(db),
                    param.generic_contraint(db).value.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(param.generic_contraint(db).value.get_span(db))
                .call(),
            Self::UnknownGenericConstraint { param, constraint } => diag()
                .message(format!(
                    "generic '{}' has unknown constraint '{}'",
                    param.name(db).text(db),
                    constraint.as_call_site(db).to_string(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(constraint.as_call_site(db).get_span(db))
                .call(),
            Self::InvalidGenericConstraint { param, constraint } => diag()
                .message(format!(
                    "generic '{}' has invalid constraint '{}'",
                    param.name(db).text(db),
                    constraint.as_call_site(db).to_string(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(constraint.as_call_site(db).get_span(db))
                .call(),
            Self::MissingTypeArguments {
                func_name,
                call_site,
            } => diag()
                .message(format!(
                    "generic function '{}' requires explicit type arguments",
                    func_name.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(call_site.get_span(db))
                .call(),
            Self::WrongTypeArgumentArity {
                func_name,
                expected,
                actual,
                call_site,
            } => diag()
                .message(format!(
                    "expected {} type argument(s), got {}",
                    expected, actual
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(call_site.get_span(db))
                .call(),
            Self::TypeArgumentConstraintMismatch {
                concrete_type,
                param_name,
                constraint,
                call_site,
            } => diag()
                .message(format!(
                    "type '{}' does not satisfy constraint '{}' (on generic parameter '{}')",
                    concrete_type.type_name(db),
                    constraint,
                    param_name.text(db),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(call_site.get_span(db))
                .call(),
            Self::TypeArgumentIntoConstraintMismatch {
                type_arg,
                into_target,
                param_name,
                call_site,
            } => diag()
                .message(format!(
                    "'{}' cannot be implicitly cast into '{}' (INTO constraint on '{}')",
                    type_arg.type_name(db),
                    into_target.type_name(db),
                    param_name.text(db),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(call_site.get_span(db))
                .call(),
            Self::NonVariadicFoldParameter { var, call_site } => {
            let mut diag = diag()
                .message(format!(
                    "variable '{}' is not variadic",
                    var.get_name_ident(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(call_site.get_span(db))
                .call();
                
                diag.with_note("... can only be used on VAR_INPUT variables that are declared variadic with the same operator (e.g: INT...)".into());
                diag
            },
            Self::NonNumericFoldParameter { typ, operator, call_site } => {
                let mut diag = diag()
                    .message(format!(
                        "math operators can not be applied to type '{}'",
                        typ.type_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(call_site.get_span(db))
                    .call();

                typ.with_location(db, &mut diag);

                diag.with_note("only numeric types can be used with fold operators".into());
                diag
            },
            Self::Other { message, expr } => diag()
                .message(message.clone())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(expr.get_span(db))
                .call(),
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
                        err.to_string()
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(expr.get_span(db))
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
                format!("REF TO {}", adj.target.type_name(db))
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
    Invalid_WSTRING_Literal,
    Invalid_CHAR_Length(usize),
    Invalid_WCHAR_Length(usize),

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
            InferLiteralError::Invalid_CHAR_Length(len) => {
                return format!("CHAR literal must be exactly 1 character, got {len}");
            }
            InferLiteralError::Invalid_WCHAR_Length(len) => {
                return format!("WCHAR literal must be exactly 1 character, got {len}");
            }

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

fn explicit_cast_suggestion(
    db: &dyn WorkspaceDataBase,
    expected: Type,
    actual: Type,
    actual_site: CallSite,
    diag: &mut IdeDiagnostic,
) {
    match (expected.normalize(db), actual.normalize(db)) {
        (Type::Elementary(lhs), Type::Elementary(rhs)) => {

            if lhs.explicit_cast(rhs) {
                diag.with_related(Related::new(
                    format!(
                        "consider explicitly casting with '{}_TO_{}({})'",
                        rhs.type_name(),
                        lhs.type_name(),
                        actual_site.to_string(db)
                    ),
                    actual_site.get_scope_id(db).file(db),
                    actual_site.get_span(db),
                ));

                diag.with_fix(CodeAction {
                    title: format!(
                        "insert explicit cast '{}_TO_{}({})'",
                        lhs.type_name(),
                        rhs.type_name(),
                        actual_site.to_string(db)
                    ),
                    edit: Some(WorkspaceEdit::new(HashMap::new())),
                    is_preferred: Some(true),
                    ..Default::default()
                });
            }
        }
        _ => {}
    }
}
