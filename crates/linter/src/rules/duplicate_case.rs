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

/// L0127: duplicate CASE selector value.
struct DuplicateCase;

impl ErrorCode for DuplicateCase {
    fn code(&self) -> &'static str {
        "L0127"
    }

    fn description(&self) -> &'static str {
        "duplicate CASE selector"
    }
}

/// Check a single CASE statement's selectors for duplicates.
///
/// For expressions, we use the source text (`as_call_site().to_string()`) as the
/// dedup key. This naturally disambiguates `STATE#A` from `STATE2#A`.
/// For subranges, we compare evaluated integer bounds.
pub fn check_case<'db>(
    db: &'db dyn WorkspaceDataBase,
    cases: &[(
        Vec<CaseKind<'db>>,
        Vec<hir::hir_def::expressions::statement::Stmt<'db>>,
    )],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let mut seen_exprs: FxHashMap<String, CaseKind<'db>> = FxHashMap::default();
    let mut seen_ranges: FxHashMap<(u64, u64), CaseKind<'db>> = FxHashMap::default();

    for (selectors, _) in cases {
        for selector in selectors {
            match selector {
                CaseKind::Expression(expr) => {
                    let key = expr.as_call_site(db).to_string(db).to_string();
                    if let Some(first) = seen_exprs.insert(key.clone(), *selector) {
                        emit_dup(db, &first, expr, &key, diagnostics);
                    }
                }
                CaseKind::Subrange { lower, upper } => {
                    if let (Some(lo), Some(hi)) =
                        (eval_integer(db, lower), eval_integer(db, upper))
                    {
                        if let Some(first) = seen_ranges.insert((lo, hi), *selector) {
                            let display = format!("{lo}..{hi}");
                            let span = lower.get_span(db);
                            let mut d = diag()
                                .message(format!(
                                    "CASE range '{display}' is duplicated, second branch is unreachable"
                                ))
                                .desc(&DuplicateCase)
                                .range(span)
                                .severity(DiagnosticSeverity::WARNING)
                                .tags(vec![DiagnosticTag::UNNECESSARY])
                                .call();

                            let (file, range) = match first {
                                CaseKind::Expression(expr) => {
                                    (expr.get_scope_id(db).file(db), expr.get_span(db))
                                }
                                CaseKind::Subrange { lower, .. } => {
                                    (lower.get_scope_id(db).file(db), lower.get_span(db))
                                }
                            };
                            d.with_related(Related::new(
                                "CASE selector is already defined here".into(),
                                file,
                                range,
                            ));
                            diagnostics.push(d);
                        }
                    }
                }
            }
        }
    }
}

fn emit_dup(
    db: &dyn WorkspaceDataBase,
    first: &CaseKind<'_>,
    expr: &Expr<'_>,
    display: &str,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let mut d = diag()
        .message(format!(
            "CASE selector '{display}' is duplicated, second branch is unreachable"
        ))
        .desc(&DuplicateCase)
        .range(expr.get_span(db))
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
