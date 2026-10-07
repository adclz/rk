// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::expressions::{
        expression::Expr,
        statement::{CaseKind, Stmt, StmtKind},
    },
    hir_ty::{
        body::{CaseLabelValue, ScopeInference},
        infer::const_eval::{integer_range, subrange_bounds},
        ty::Type,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "case-without-else";

/// L0209: CASE statement without ELSE branch.
struct CaseWithoutElse;

impl ErrorCode for CaseWithoutElse {
    fn code(&self) -> &'static str {
        "L0209"
    }
}

/// What the labels of a CASE with no ELSE leave out.
enum Coverage {
    /// Every value the selector can hold has a branch.
    Complete,
    /// The variants of the selector's enum that no label names.
    Variants(Vec<String>),
    /// Values with no branch, or labels the lint cannot read.
    Partial,
}

type Branches<'db> = [(Vec<CaseKind<'db>>, Vec<Stmt<'db>>)];

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in body.case_without_else() {
        let StmtKind::Case {
            condition, cases, ..
        } = stmt.stmt(db)
        else {
            continue;
        };
        let (message, help) = match coverage(db, body, condition, cases) {
            Coverage::Complete => continue,
            Coverage::Variants(missing) => (
                format!("CASE statement does not handle {}", missing.join(", ")),
                Some(match missing.len() {
                    1 => "add a branch for it, or an ELSE branch",
                    _ => "add a branch for each, or an ELSE branch",
                }),
            ),
            Coverage::Partial => ("CASE statement has no ELSE branch".to_string(), None),
        };
        let mut d = diag()
            .message(message)
            .desc(&CaseWithoutElse)
            .range(
                hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::INFORMATION)
            .call();
        if let Some(help) = help {
            d.with_help(help.to_string());
        }
        diagnostics.push(d);
    }
}

fn coverage<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    selector: &Expr<'db>,
    cases: &Branches<'db>,
) -> Coverage {
    let declared = body.type_of_expr_adjusted(*selector);
    match declared.normalize(db) {
        Type::Enum(enm) => {
            // A label names its variant, `Color#Red`: the variant is what
            // is covered, whatever value it was given.
            let mut named = FxHashSet::default();
            let mut owner = None;
            for label in cases.iter().flat_map(|(labels, _)| labels) {
                let CaseKind::Expression(expr) = label else {
                    return Coverage::Partial;
                };
                let Type::EnumVariant(data_type, variant) = body.type_of_expr(*expr) else {
                    return Coverage::Partial;
                };
                owner.get_or_insert(data_type);
                named.insert(variant);
            }
            let Some(owner) = owner else {
                return Coverage::Partial;
            };
            let owner = owner.get_name_with_case(db).text(db);
            let missing: Vec<String> = enm
                .variants(db)
                .iter()
                .filter(|variant| !named.contains(&variant.name.ident(db)))
                .map(|variant| format!("'{owner}#{}'", variant.name.with_case.text(db)))
                .collect();
            match missing.is_empty() {
                true => Coverage::Complete,
                false => Coverage::Variants(missing),
            }
        }
        Type::Elementary(spec) => {
            // A subrange holds its bounds, any other integer its type's range.
            let range = match declared.as_subrange(db) {
                Some(sub) => match subrange_bounds(db, sub) {
                    (Some(low), Some(high)) => Some((i128::from(low), i128::from(high))),
                    _ => None,
                },
                None => integer_range(spec),
            };
            match range {
                Some(range) if covers(body, cases, range) => Coverage::Complete,
                _ => Coverage::Partial,
            }
        }
        _ => Coverage::Partial,
    }
}

/// Whether the labels hold every value of `(low, high)`. A label inference
/// recorded no value for leaves the CASE uncovered.
fn covers<'db>(
    body: ScopeInference<'db>,
    cases: &Branches<'db>,
    (low, high): (i128, i128),
) -> bool {
    let value = |expr: &Expr<'db>| match body.case_label_value(*expr) {
        Some(CaseLabelValue::Int(value)) => Some(*value),
        _ => None,
    };
    let mut intervals = Vec::new();
    for label in cases.iter().flat_map(|(labels, _)| labels) {
        let interval = match label {
            CaseKind::Expression(expr) => value(expr).map(|v| (v, v)),
            CaseKind::Subrange { lower, upper } => value(lower).zip(value(upper)),
        };
        match interval {
            Some(interval) => intervals.push(interval),
            None => return false,
        }
    }
    intervals.sort_unstable();
    // The first value no label has reached yet.
    let mut next = low;
    for (start, end) in intervals {
        if start > next {
            return false;
        }
        if end >= next {
            next = end + 1;
        }
        if next > high {
            return true;
        }
    }
    false
}
