// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::expressions::expression::InitExpr;
use crate::hir_def::expressions::expression::PathExpr;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_ty::ty::Type;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::diag;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum ArrayError<'db> {
    // Array init
    InvalidArrayLowerValue {
        value: Expr<'db>,
    },
    InvalidArrayUpperValue {
        value: Expr<'db>,
    },
    InferiorUpperBound {
        lower: i64,
        upper: i64,
        upper_expr: Expr<'db>,
    },
    NonIntegerIndex {
        expr: Expr<'db>,
        ty: crate::hir_ty::ty::Type<'db>,
    },
    // Array access
    /// A repeat count too large to read: `[99999999999999999999(0)]`.
    InvalidIndex {
        size: SpanIdent<'db>,
    },
    IndexOutOfBounds {
        expr: Expr<'db>,
        dimension: usize,
        index: i128,
        min: i64,
        max: i64,
    },
    TooManyElements {
        expr: InitExpr<'db>,
        dimension: usize,
        max_size: usize,
    },
    IndexNonArrayTypeInitExpr {
        expr: InitExpr<'db>,
        ty: Type<'db>,
    },
    IndexNonArrayTypePathExpr {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    /// An `ARRAY[*]` where it is not allowed: anything but a parameter of a
    /// FUNCTION or METHOD, or a VAR_IN_OUT of a FUNCTION_BLOCK, and for one
    /// of any type (`any_type`) anything but a VAR_INPUT or VAR_IN_OUT of a
    /// FUNCTION. `place` says what it was declared as.
    ConformandNotAllowed {
        spec: crate::hir_def::expressions::spec::Spec<'db>,
        place: &'static str,
        any_type: bool,
    },
    /// A path ending in a subscript that leaves dimensions of a
    /// multi-dimensional array: `named` of its `rank`, the last one `expr`.
    IncompleteSubscript {
        expr: Expr<'db>,
        rank: usize,
        named: usize,
    },
    /// A subscript on an `ARRAY[*]` of any type, whose elements have none.
    ElementOfAnyType {
        expr: PathExpr<'db>,
        var: Option<crate::hir_def::pous::variable::VariableDecl<'db>>,
    },
    /// A FUNCTION_BLOCK's `ARRAY[*]` VAR_IN_OUT read where no call gives its
    /// bounds: in a method, in an initializer, or through an instance.
    ConformandOutsideBody {
        access: CallSite<'db>,
        var: crate::hir_def::pous::variable::VariableDecl<'db>,
        /// The block that declares it, as written.
        block: Ident,
    },
}

impl<'db> ErrorCode for ArrayError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidArrayLowerValue { .. } => "E0501",
            Self::InvalidArrayUpperValue { .. } => "E0502",
            Self::InferiorUpperBound { .. } => "E0503",
            Self::NonIntegerIndex { .. } => "E0504",
            Self::InvalidIndex { .. } => "E0505",
            Self::IndexOutOfBounds { .. } => "E0506",
            Self::TooManyElements { .. } => "E0507",
            Self::IndexNonArrayTypeInitExpr { .. } => "E0508",
            Self::IndexNonArrayTypePathExpr { .. } => "E0508",
            Self::ConformandNotAllowed { .. } => "E0509",
            Self::IncompleteSubscript { .. } => "E0510",
            Self::ElementOfAnyType { .. } => "E0511",
            Self::ConformandOutsideBody { .. } => "E0513",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for ArrayError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            ArrayError::InvalidArrayLowerValue { value } => diag()
                .message("the lower bound is not an integer constant".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                .call(),
            ArrayError::InvalidArrayUpperValue { value } => diag()
                .message("the upper bound is not an integer constant".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                .call(),
            ArrayError::InferiorUpperBound {
                lower,
                upper,
                upper_expr,
            } => diag()
                .message(format!(
                    "the upper bound {upper} is below the lower bound {lower}"
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &upper_expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::NonIntegerIndex { expr, ty } => diag()
                .message(format!(
                    "the index is '{}', not an integer",
                    ty.type_name(db)
                ))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::InvalidIndex { size } => diag()
                .message(format!(
                    "the repeat count '{}' is too large",
                    size.as_str(db)
                ))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &size.get_span(db)).unwrap_or_default())
                .call(),
            Self::IndexOutOfBounds {
                expr,
                dimension,
                index,
                min,
                max,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "index {index} is out of bounds (the dimension is declared {min}..{max})"
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                if *dimension > 0_usize {
                    diag.with_note(format!(
                        "this error occurred in array dimension {}",
                        dimension + 1
                    ))
                }

                diag
            }
            Self::TooManyElements {
                expr,
                dimension,
                max_size,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "the initializer has more elements than the array's {}",
                        max_size
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                if *dimension > 0_usize {
                    diag.with_note(format!(
                        "this error occurred in array dimension {}",
                        dimension + 1
                    ))
                }

                diag
            }
            Self::IndexNonArrayTypeInitExpr { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::IndexNonArrayTypePathExpr { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::ConformandNotAllowed {
                spec,
                place,
                any_type,
            } => {
                let (what, note, help) = match any_type {
                    false => (
                        "an ARRAY[*]",
                        "an ARRAY[*] takes its bounds from each call: it is a parameter of a \
                         FUNCTION or a METHOD, or a VAR_IN_OUT of a FUNCTION_BLOCK",
                        "give it bounds, as in `ARRAY[0..9] OF INT`",
                    ),
                    true => (
                        "an ARRAY[*] of any type",
                        "an ARRAY[*] of any type is a VAR_INPUT or a VAR_IN_OUT of a FUNCTION",
                        "give it an element type, as in `ARRAY[*] OF INT`",
                    ),
                };
                let mut diag = diag()
                    .message(format!("{place} cannot be {what}"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &spec.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(note.to_string());
                diag.with_help(help.to_string());
                diag
            }
            Self::IncompleteSubscript { expr, rank, named } => {
                let mut diag = diag()
                    .message(format!(
                        "this subscript names {named} of the array's {rank} dimensions"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "a multi-dimensional array is indexed in all its dimensions".to_string(),
                );
                diag.with_help(
                    "subscript every dimension, `m[i, j]` or `m[i][j]`, or declare an array of \
                     an array type for rows"
                        .to_string(),
                );
                diag
            }
            Self::ElementOfAnyType { expr, var } => {
                let message = match var {
                    Some(var) => format!(
                        "the elements of '{}' have no type",
                        var.name_with_case(db).text(db)
                    ),
                    None => "the elements of this array have no type".to_string(),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                if let Some(var) = var {
                    diag.with_related(ide_diagnostic::Related::new(
                        format!("'{}' is declared here", var.name_with_case(db).text(db)),
                        var.scope_id(db).file(db),
                        var.get_name_span(db),
                    ));
                }
                diag.with_note(
                    "an ARRAY[*] of any type is passed on or has its bounds read".to_string(),
                );
                diag.with_help("give it an element type, as in `ARRAY[*] OF INT`".to_string());
                diag
            }
            Self::ConformandOutsideBody { access, var, block } => {
                let name = var.name_with_case(db).text(db);
                let mut diag = diag()
                    .message(format!(
                        "'{name}' has no bounds outside the body of '{}'",
                        block.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(ide_diagnostic::Related::new(
                    format!("'{name}' is declared here"),
                    var.scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                diag.with_note(
                    "an ARRAY[*] VAR_IN_OUT has the bounds of what the call binds to it"
                        .to_string(),
                );
                diag.with_help(
                    "pass it from the body to an ARRAY[*] parameter of the method".to_string(),
                );
                diag
            }
        }
    }
}
