use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::{DiagnosticSeverity, Range};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::statement::{Stmt, StmtKind},
        pous::{pou::Pou, pragma::Pragma},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

type RuleName = compact_str::CompactString;

pub const NAME: &str = "unknown-allow";

/// L0005: an `{allow}` pragma names a rule that does not exist.
struct UnknownAllow;

impl ErrorCode for UnknownAllow {
    fn code(&self) -> &'static str {
        "L0005"
    }

    fn description(&self) -> &'static str {
        "unknown-allow"
    }
}

/// A region of the file where the named rules are silenced.
pub(crate) struct AllowRegion {
    pub range: Range,
    pub rules: Vec<RuleName>,
}

impl AllowRegion {
    /// Whether a diagnostic STARTING inside this region names one of ours.
    /// Containment by start point: a lint's range never starts before the
    /// construct that produced it.
    pub fn silences(&self, rule: &str, at: &Range) -> bool {
        let starts_inside = (self.range.start.line, self.range.start.character)
            <= (at.start.line, at.start.character)
            && (at.start.line, at.start.character)
                <= (self.range.end.line, self.range.end.character);
        starts_inside && self.rules.iter().any(|r| r == rule)
    }
}

/// The rule names a scope's POU-level `{allow}` pragmas silence. These are
/// folded into an EFFECTIVE config before any of the scope's rules run, so a
/// silenced rule is never executed at all.
pub(crate) fn scope_allows<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    report_unknown: bool,
    diagnostics: &mut Vec<IdeDiagnostic>,
) -> Vec<RuleName> {
    let file = scope.file(db);
    let pragmas = match &get_scope(db, scope).kind {
        ScopeKind::Pou(Pou::Function(f)) => f.pragmas(db).as_slice(),
        ScopeKind::Pou(Pou::FunctionBlock(fb)) => fb.pragmas(db).as_slice(),
        ScopeKind::MethodDecl(m) => m.pragmas(db).as_slice(),
        ScopeKind::Program(p) => p.pragmas(db).as_slice(),
        _ => &[],
    };
    let mut names = Vec::new();
    for pragma in pragmas {
        if let Pragma::Allow(_, allow) = pragma {
            names.extend(validate(db, file, allow, report_unknown, diagnostics));
        }
    }
    names
}

/// The statement-level `{allow}` regions of one scope's body: each covers the
/// statement FOLLOWING its pragma, nested bodies included. They are installed
/// for the duration of the scope's rules and consulted by `run_lint` when a
/// diagnostic is born.
pub(crate) fn statement_regions<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    report_unknown: bool,
    diagnostics: &mut Vec<IdeDiagnostic>,
) -> Vec<AllowRegion> {
    let file = scope.file(db);
    let statements = match &get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => match pou {
            Pou::Function(f) => f.statements(db),
            Pou::FunctionBlock(fb) => fb.statements(db),
            _ => return Vec::new(),
        },
        ScopeKind::MethodDecl(m) => m.stmts(db),
        ScopeKind::Program(p) => p.statements(db),
        _ => return Vec::new(),
    };
    let mut regions = Vec::new();
    scan_statements(
        db,
        file,
        statements,
        report_unknown,
        &mut regions,
        diagnostics,
    );
    regions
}

thread_local! {
    /// The active scope's statement regions. A thread-local, not a parameter:
    /// `run_lint` sits under 50+ call sites across the rule files, and one
    /// file's linting runs on one thread from start to finish.
    static REGIONS: std::cell::RefCell<Vec<AllowRegion>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Install a scope's statement regions; dropping the guard clears them, so
/// file-level rules outside any scope never see stale ones.
pub(crate) struct RegionGuard;

impl Drop for RegionGuard {
    fn drop(&mut self) {
        REGIONS.with_borrow_mut(|r| r.clear());
    }
}

pub(crate) fn install_regions(regions: Vec<AllowRegion>) -> RegionGuard {
    REGIONS.with_borrow_mut(|r| *r = regions);
    RegionGuard
}

/// Whether the active regions silence `rule` for a diagnostic at `at`.
pub(crate) fn silenced(rule: &str, at: &Range) -> bool {
    REGIONS.with_borrow(|regions| regions.iter().any(|r| r.silences(rule, at)))
}

fn scan_statements<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    stmts: &[Stmt<'db>],
    report_unknown: bool,
    regions: &mut Vec<AllowRegion>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (i, stmt) in stmts.iter().enumerate() {
        match stmt.stmt(db) {
            StmtKind::AllowPragma(allow) => {
                let rules = validate(db, file, allow, report_unknown, diagnostics);
                // The NEXT statement, nested bodies and all. Trailing: covers
                // nothing, but its names were still validated above.
                if let Some(next) = stmts.get(i + 1)
                    && let Some(range) = hir::denormalize(db, file, &next.get_span(db))
                {
                    regions.push(AllowRegion { range, rules });
                }
            }
            StmtKind::If {
                then,
                else_if,
                else_,
                ..
            } => {
                for body in then.iter().chain(else_.iter()) {
                    scan_statements(db, file, body, report_unknown, regions, diagnostics);
                }
                for (_, body) in else_if {
                    scan_statements(db, file, body, report_unknown, regions, diagnostics);
                }
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, body) in cases {
                    scan_statements(db, file, body, report_unknown, regions, diagnostics);
                }
                for body in else_.iter() {
                    scan_statements(db, file, body, report_unknown, regions, diagnostics);
                }
            }
            StmtKind::For { body, .. }
            | StmtKind::While { body, .. }
            | StmtKind::Repeat { body, .. } => {
                scan_statements(db, file, body, report_unknown, regions, diagnostics);
            }
            _ => {}
        }
    }
}

/// The pragma's known rule names; unknown ones become L0005 (when the rule
/// is enabled) and never silence anything — a typo must not silence the typo.
fn validate(
    db: &dyn WorkspaceDataBase,
    file: File,
    allow: &hir::hir_def::pous::pragma::AllowPragma,
    report_unknown: bool,
    diagnostics: &mut Vec<IdeDiagnostic>,
) -> Vec<RuleName> {
    let mut known = Vec::with_capacity(allow.rules.len());
    for (name, span) in &allow.rules {
        if crate::rules::ALL_RULE_NAMES.contains(&name.as_str()) {
            known.push(name.clone());
        } else if report_unknown {
            super::run_lint(NAME, diagnostics, |d| {
                d.push(
                    diag()
                        .message(format!("no lint rule is named '{name}'"))
                        .desc(&UnknownAllow)
                        .range(hir::denormalize(db, file, span).unwrap_or_default())
                        .severity(DiagnosticSeverity::WARNING)
                        .call(),
                );
            });
        }
    }
    known
}
