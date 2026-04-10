use auto_lsp::core::span::Span;
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::{
        expression::{Elementary, Expr, ExprKind, PrimaryExpr},
        statement::CaseKind,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashMap;

pub const NAME: &str = "duplicate-case";

/// L0306: duplicate CASE selector value.
struct DuplicateCase;

impl ErrorCode for DuplicateCase {
    fn code(&self) -> &'static str {
        "L0306"
    }

    fn description(&self) -> &'static str {
        "duplicate CASE selector"
    }
}

/// A resolved range with its source span and file.
struct SeenRange {
    lo: u64,
    hi: u64,
    span: Span,
    file: File,
}

/// Check a single CASE statement's selectors for duplicates and overlaps.
pub fn check_case<'db>(
    db: &'db dyn WorkspaceDataBase,
    cases: &[(
        Vec<CaseKind<'db>>,
        Vec<hir::hir_def::expressions::statement::Stmt<'db>>,
    )],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let mut seen_exprs: FxHashMap<String, CaseKind<'db>> = FxHashMap::default();
    let mut seen_ranges: Vec<SeenRange> = Vec::new();

    for (selectors, _) in cases {
        for selector in selectors {
            match selector {
                CaseKind::Expression(expr) => {
                    let key = expr.as_call_site(db).to_string(db).to_string();

                    // Check exact duplicate
                    if let Some(first) = seen_exprs.insert(key.clone(), *selector) {
                        emit(
                            db,
                            &first,
                            expr.get_span(db),
                            &format!(
                                "CASE selector '{key}' is duplicated, second branch is unreachable"
                            ),
                            diagnostics,
                        );
                        continue;
                    }

                    // Check if this integer value falls inside an existing range
                    if let Some(val) = eval_integer(db, expr) {
                        for prev in &seen_ranges {
                            if val >= prev.lo && val <= prev.hi {
                                let mut d = diag()
                                    .message(format!(
                                        "CASE selector '{key}' is already covered by range '{}..{}'",
                                        prev.lo, prev.hi
                                    ))
                                    .desc(&DuplicateCase)
                                    .range(expr.get_span(db))
                                    .severity(DiagnosticSeverity::WARNING)
                                    .tags(vec![DiagnosticTag::UNNECESSARY])
                                    .call();
                                d.with_related(Related::new(
                                    "range defined here".into(),
                                    prev.file,
                                    prev.span,
                                ));
                                diagnostics.push(d);
                                break;
                            }
                        }
                    }
                }
                CaseKind::Subrange { lower, upper } => {
                    if let (Some(lo), Some(hi)) = (eval_integer(db, lower), eval_integer(db, upper))
                    {
                        let span = lower.get_span(db);
                        let file = lower.get_scope_id(db).file(db);

                        // Check exact duplicate range
                        let exact_dup = seen_ranges.iter().find(|r| r.lo == lo && r.hi == hi);
                        if let Some(prev) = exact_dup {
                            let mut d = diag()
                                .message(format!(
                                    "CASE range '{lo}..{hi}' is duplicated, second branch is unreachable"
                                ))
                                .desc(&DuplicateCase)
                                .range(span)
                                .severity(DiagnosticSeverity::WARNING)
                                .tags(vec![DiagnosticTag::UNNECESSARY])
                                .call();
                            d.with_related(Related::new(
                                "CASE selector is already defined here".into(),
                                prev.file,
                                prev.span,
                            ));
                            diagnostics.push(d);
                            continue;
                        }

                        // Check overlap with existing ranges
                        for prev in &seen_ranges {
                            if lo <= prev.hi && hi >= prev.lo {
                                let mut d = diag()
                                    .message(format!(
                                        "CASE range '{lo}..{hi}' overlaps with '{}'",
                                        format_range(prev)
                                    ))
                                    .desc(&DuplicateCase)
                                    .range(span)
                                    .severity(DiagnosticSeverity::WARNING)
                                    .call();
                                d.with_related(Related::new(
                                    "overlapping range defined here".into(),
                                    prev.file,
                                    prev.span,
                                ));
                                diagnostics.push(d);
                                break;
                            }
                        }

                        // Check if any existing expression value falls in this new range
                        for (key, prev_selector) in &seen_exprs {
                            let val = match prev_selector {
                                CaseKind::Expression(e) => eval_integer(db, e),
                                _ => None,
                            };
                            if let Some(val) = val
                                && val >= lo && val <= hi {
                                    let prev_span = match prev_selector {
                                        CaseKind::Expression(e) => e.get_span(db),
                                        _ => continue,
                                    };
                                    let prev_file = match prev_selector {
                                        CaseKind::Expression(e) => e.get_scope_id(db).file(db),
                                        _ => continue,
                                    };
                                    let mut d = diag()
                                        .message(format!(
                                            "CASE range '{lo}..{hi}' covers already defined selector '{key}'"
                                        ))
                                        .desc(&DuplicateCase)
                                        .range(span)
                                        .severity(DiagnosticSeverity::WARNING)
                                        .call();
                                    d.with_related(Related::new(
                                        "selector defined here".into(),
                                        prev_file,
                                        prev_span,
                                    ));
                                    diagnostics.push(d);
                                    break;
                                }
                        }

                        seen_ranges.push(SeenRange { lo, hi, span, file });
                    }
                }
            }
        }
    }
}

fn format_range(r: &SeenRange) -> String {
    format!("{}..{}", r.lo, r.hi)
}

fn emit(
    db: &dyn WorkspaceDataBase,
    first: &CaseKind<'_>,
    span: Span,
    message: &str,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let mut d = diag()
        .message(message.to_string())
        .desc(&DuplicateCase)
        .range(span)
        .severity(DiagnosticSeverity::WARNING)
        .tags(vec![DiagnosticTag::UNNECESSARY])
        .call();

    let (file, range) = match first {
        CaseKind::Expression(expr) => (expr.get_scope_id(db).file(db), expr.get_span(db)),
        CaseKind::Subrange { lower, .. } => (lower.get_scope_id(db).file(db), lower.get_span(db)),
    };
    d.with_related(Related::new(
        "CASE selector is already defined here".into(),
        file,
        range,
    ));
    diagnostics.push(d);
}

fn eval_integer<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> Option<u64> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)) => match lit {
            Elementary::InferInteger(i)
            | Elementary::SInt(i)
            | Elementary::Int(i)
            | Elementary::DInt(i)
            | Elementary::LInt(i)
            | Elementary::USInt(i)
            | Elementary::UInt(i)
            | Elementary::UDInt(i)
            | Elementary::ULInt(i)
            | Elementary::Byte(i)
            | Elementary::Word(i)
            | Elementary::DWord(i)
            | Elementary::LWord(i) => i.as_u64(db).ok(),
            _ => None,
        },
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => eval_integer(db, expr),
        _ => None,
    }
}
