use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use auto_lsp::tree_sitter;
use db::WorkspaceDataBase;
use hir::hir_ty::body::CaseLabelValue;
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

/// L0109: duplicate CASE selector value.
struct DuplicateCase;

impl ErrorCode for DuplicateCase {
    fn code(&self) -> &'static str {
        "L0109"
    }

    fn description(&self) -> &'static str {
        "duplicate CASE selector"
    }
}

/// A resolved range with its source span and file.
struct SeenRange {
    lo: u64,
    hi: u64,
    span: tree_sitter::Range,
    file: File,
}

/// Check a single CASE statement's selectors for duplicates and overlaps.
pub fn check_case<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &hir::hir_ty::body::BodyInferenceResult<'db>,
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
                    // Key on the value HIR evaluated, so labels that are
                    // written differently but ARE the same value collide:
                    // `7`, a CONSTANT `K = 7` and `3+4` are one label. The
                    // source text is the fallback for the labels that have no
                    // integer value — enum variants and strings.
                    let written = expr.as_call_site(db).to_string(db).to_string();
                    // Key on the value, show the SOURCE TEXT. Two labels that
                    // are written differently but hold the same value are one
                    // label, and the second branch is unreachable — but the
                    // message must name what the user wrote.
                    // Both value domains key the same map: two labels collide
                    // when they denote the same thing, whether that is an
                    // integer (`1` and `INT#1`) or a string (`'a'` and
                    // `STRING#'a'`). A label with no recorded value — an enum
                    // variant — falls back to what was written.
                    let key = match body.case_label_value.get(expr) {
                        Some(CaseLabelValue::Int(v)) => format!("#{v}"),
                        Some(CaseLabelValue::Str(bytes)) => format!("${bytes:?}"),
                        None => written.clone(),
                    };

                    // Check exact duplicate
                    if let Some(first) = seen_exprs.insert(key.clone(), *selector) {
                        emit(
                            db,
                            &first,
                            expr.get_span(db),
                            expr.get_scope_id(db).file(db),
                            &format!(
                                "CASE selector '{written}' is duplicated, second branch is unreachable"
                            ),
                            diagnostics,
                        );
                        continue;
                    }

                    // Check if this integer value falls inside an existing range
                    if let Some(val) = eval_label(body, db, expr) {
                        for prev in &seen_ranges {
                            if val >= prev.lo && val <= prev.hi {
                                let mut d = diag()
                                    .message(format!(
                                        "CASE selector '{written}' is already covered by range '{}..{}'",
                                        prev.lo, prev.hi
                                    ))
                                    .desc(&DuplicateCase)
                                    .range(hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db)).unwrap_or_default())
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
                    if let (Some(lo), Some(hi)) =
                        (eval_label(body, db, lower), eval_label(body, db, upper))
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
                                .range(hir::denormalize(db, file, &span).unwrap_or_default())
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
                                    .range(hir::denormalize(db, file, &span).unwrap_or_default())
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
                        for prev_selector in seen_exprs.values() {
                            let (val, prev_written) = match prev_selector {
                                CaseKind::Expression(e) => (
                                    eval_label(body, db, e),
                                    e.as_call_site(db).to_string(db).to_string(),
                                ),
                                _ => (None, String::new()),
                            };
                            if let Some(val) = val
                                && val >= lo
                                && val <= hi
                            {
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
                                            "CASE range '{lo}..{hi}' covers already defined selector '{prev_written}'"
                                        ))
                                        .desc(&DuplicateCase)
                                        .range(hir::denormalize(db, file, &span).unwrap_or_default())
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
    span: tree_sitter::Range,
    file: File,
    message: &str,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let mut d = diag()
        .message(message.to_string())
        .desc(&DuplicateCase)
        .range(hir::denormalize(db, file, &span).unwrap_or_default())
        .severity(DiagnosticSeverity::WARNING)
        .tags(vec![DiagnosticTag::UNNECESSARY])
        .call();

    let (related_file, related_range) = match first {
        CaseKind::Expression(expr) => (expr.get_scope_id(db).file(db), expr.get_span(db)),
        CaseKind::Subrange { lower, .. } => (lower.get_scope_id(db).file(db), lower.get_span(db)),
    };
    d.with_related(Related::new(
        "CASE selector is already defined here".into(),
        related_file,
        related_range,
    ));
    diagnostics.push(d);
}

/// A label's value, as HIR evaluated it (`case_label_value`) — the same
/// answer the compiler branches on, so an overlap the lint reports is a real
/// one. The local literal walk stays only as the fallback for a label HIR did
/// not record.
fn eval_label<'db>(
    body: &hir::hir_ty::body::BodyInferenceResult<'db>,
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
) -> Option<u64> {
    match body.case_label_value.get(expr) {
        // Only integers order, so only integers enter the interval scan; a
        // string label has no place on a number line and never reaches it.
        Some(CaseLabelValue::Int(v)) => u64::try_from(*v).ok(),
        Some(CaseLabelValue::Str(_)) => None,
        None => eval_integer(db, expr),
    }
}

fn eval_integer<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> Option<u64> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(
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
            | Elementary::LWord(i),
        )) => i.as_u64(db).ok(),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => eval_integer(db, expr),
        _ => None,
    }
}
