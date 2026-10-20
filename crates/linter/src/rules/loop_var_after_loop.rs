// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A FOR loop's counter read after the loop, before anything writes it
//! again. Every path is followed: a read is reported when one path reaches
//! it from a loop with the counter left as the loop left it. A loop body is
//! walked until what it leaves no longer grows, so a read at the top of an
//! outer loop's body sees what an inner loop left the pass before.

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{BeginPathExpr, VariableAccess, VariableAccessKind},
            statement::{Stmt, StmtKind},
        },
        pous::variable::VariableDecl,
    },
    hir_ty::body::ScopeInference,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::{FxHashMap, FxHashSet};

use super::accesses::{self, Place, Use};

pub const NAME: &str = "loop-var-after-loop";

/// L0130: a FOR control variable read after its loop.
struct LoopVarAfterLoop;

impl ErrorCode for LoopVarAfterLoop {
    fn code(&self) -> &'static str {
        "L0130"
    }
}

/// The counters holding what their loop left in them, each with the
/// control variable of that loop's header.
type Left<'db> = FxHashMap<VariableDecl<'db>, VariableAccess<'db>>;

/// How a list of statements ends.
#[derive(Default)]
struct Ends<'db> {
    /// What holds where it runs on, `None` when no path does: every one
    /// returns, raises, or leaves by EXIT or CONTINUE.
    through: Option<Left<'db>>,
    /// At the EXITs, for the loop around them.
    exits: Option<Left<'db>>,
    /// At the CONTINUEs.
    continues: Option<Left<'db>>,
}

/// `into` holds what either holds.
fn merge<'db>(into: &mut Option<Left<'db>>, from: Option<Left<'db>>) {
    let Some(from) = from else {
        return;
    };
    match into {
        Some(into) => {
            for (var, origin) in from {
                into.entry(var).or_insert(origin);
            }
        }
        None => *into = Some(from),
    }
}

/// A pass over a loop body leaves at most one more counter: a handful of
/// passes reach what a body leaves, and this bounds a body that keeps
/// changing.
const PASSES: usize = 8;

/// The statements a body's walk may visit. A loop's second pass starts
/// with what its first one left, so the loops inside it take one pass each
/// and a body is walked a few times per loop around it. This bounds what a
/// shape nobody foresaw could cost: a body past it is given up, and nothing
/// in it is reported.
const VISITS: usize = 100_000;

struct Walk<'db> {
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    /// The reads already reported, by where they are: a loop body is
    /// walked more than once.
    reported: FxHashSet<(usize, usize)>,
    /// The statements visited so far, against [`VISITS`].
    visits: usize,
    found: Vec<IdeDiagnostic>,
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    statements: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let mut walk = Walk {
        db,
        body,
        reported: FxHashSet::default(),
        visits: 0,
        found: Vec::new(),
    };
    walk.block(statements, Left::default());
    if walk.visits <= VISITS {
        diagnostics.extend(walk.found);
    }
}

impl<'db> Walk<'db> {
    fn block(&mut self, stmts: &[Stmt<'db>], left: Left<'db>) -> Ends<'db> {
        let mut ends = Ends {
            through: Some(left),
            ..Default::default()
        };
        for stmt in stmts {
            // What follows a RETURN, an EXIT or a raise does not run.
            let Some(left) = ends.through.take() else {
                break;
            };
            let next = self.statement(*stmt, left);
            merge(&mut ends.exits, next.exits);
            merge(&mut ends.continues, next.continues);
            ends.through = next.through;
        }
        ends
    }

    fn statement(&mut self, stmt: Stmt<'db>, mut left: Left<'db>) -> Ends<'db> {
        let db = self.db;
        self.visits += 1;
        if self.visits > VISITS {
            return Ends::default();
        }
        match stmt.stmt(db) {
            StmtKind::Assignment { .. }
            | StmtKind::FuncCall(_)
            | StmtKind::EmptyPathExpression(_) => {
                self.places(&mut left, |f| accesses::statements(db, &[stmt], f));
                Ends {
                    through: Some(left),
                    ..Default::default()
                }
            }
            StmtKind::Raise { message } => {
                self.places(&mut left, |f| accesses::expression(db, *message, f));
                Ends::default()
            }
            StmtKind::Return => Ends::default(),
            StmtKind::Exit => Ends {
                exits: Some(left),
                ..Default::default()
            },
            StmtKind::Continue => Ends {
                continues: Some(left),
                ..Default::default()
            },
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                let mut ends = Ends::default();
                self.places(&mut left, |f| accesses::expression(db, *condition, f));
                let branch = self.block(then.as_deref().unwrap_or_default(), left.clone());
                ends.add(branch);
                for (condition, stmts) in else_if {
                    self.places(&mut left, |f| accesses::expression(db, *condition, f));
                    let branch = self.block(stmts, left.clone());
                    ends.add(branch);
                }
                match else_ {
                    Some(stmts) => {
                        let branch = self.block(stmts, left);
                        ends.add(branch);
                    }
                    None => merge(&mut ends.through, Some(left)),
                }
                ends
            }
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                let mut ends = Ends::default();
                self.places(&mut left, |f| accesses::expression(db, *condition, f));
                for (labels, stmts) in cases {
                    self.places(&mut left, |f| {
                        for label in labels {
                            accesses::case_label(db, *label, f);
                        }
                    });
                    let branch = self.block(stmts, left.clone());
                    ends.add(branch);
                }
                match else_ {
                    Some(stmts) => {
                        let branch = self.block(stmts, left);
                        ends.add(branch);
                    }
                    None => merge(&mut ends.through, Some(left)),
                }
                ends
            }
            StmtKind::For {
                control_variable,
                start,
                end,
                step,
                body,
            } => {
                // In the order the loop starts: the start value, stored in
                // the counter, then the end and the step.
                self.places(&mut left, |f| {
                    accesses::expression(db, *start, f);
                    accesses::place(db, *control_variable, Use::Write, f);
                    accesses::expression(db, *end, f);
                    if let Some(step) = step {
                        accesses::expression(db, *step, f);
                    }
                });
                let counter = self.access_root(*control_variable);
                // Each pass starts with the counter stepped: it holds a
                // value of the loop's own.
                let (entry, ends) = self.repeat(left, |walk, mut entry| {
                    if let Some(counter) = counter {
                        entry.remove(&counter);
                    }
                    let ends = walk.block(body, entry.clone());
                    (entry, ends)
                });
                // The loop ran no pass, ran to its end, or was left by EXIT.
                let mut after = Some(entry);
                merge(&mut after, ends.through);
                merge(&mut after, ends.continues);
                merge(&mut after, ends.exits);
                let mut after = after.unwrap_or_default();
                if let Some(counter) = counter {
                    after.insert(counter, *control_variable);
                }
                Ends {
                    through: Some(after),
                    ..Default::default()
                }
            }
            StmtKind::While { condition, body } => {
                let (entry, ends) = self.repeat(left, |walk, mut entry| {
                    walk.places(&mut entry, |f| accesses::expression(db, *condition, f));
                    let ends = walk.block(body, entry.clone());
                    (entry, ends)
                });
                let mut after = Some(entry);
                merge(&mut after, ends.exits);
                Ends {
                    through: after,
                    ..Default::default()
                }
            }
            StmtKind::Repeat { condition, body } => {
                let mut tested = None;
                let (_, ends) = self.repeat(left, |walk, entry| {
                    let mut ends = walk.block(body, entry.clone());
                    let mut at_until = ends.through.take();
                    merge(&mut at_until, ends.continues.take());
                    if let Some(at_until) = at_until.as_mut() {
                        walk.places(at_until, |f| accesses::expression(db, *condition, f));
                    }
                    merge(&mut tested, at_until.clone());
                    ends.through = at_until;
                    (entry, ends)
                });
                let mut after = tested;
                merge(&mut after, ends.exits);
                Ends {
                    through: after,
                    ..Default::default()
                }
            }
            StmtKind::Try {
                body,
                catch,
                finally,
            } => {
                let entry = left.clone();
                let mut ends = Ends::default();
                let tried = self.block(body, left);
                // The handler starts wherever the body raised: before it
                // ran, or after.
                let mut handler_entry = Some(entry);
                merge(&mut handler_entry, tried.through.clone());
                ends.add(tried);
                if let Some(catch) = catch {
                    let mut handler_entry = handler_entry.unwrap_or_default();
                    if let Some(variable) = catch.variable {
                        self.places(&mut handler_entry, |f| {
                            accesses::place(db, variable, Use::Write, f)
                        });
                    }
                    let handled = self.block(&catch.body, handler_entry);
                    ends.add(handled);
                }
                if let Some(finally) = finally {
                    let entry = ends.through.take().unwrap_or_default();
                    let cleaned = self.block(finally, entry);
                    ends.through = cleaned.through;
                    merge(&mut ends.exits, cleaned.exits);
                    merge(&mut ends.continues, cleaned.continues);
                }
                ends
            }
            StmtKind::WasmPragma(_) | StmtKind::AllowPragma(_) => Ends {
                through: Some(left),
                ..Default::default()
            },
        }
    }

    /// A loop body walked until the counters it starts with stop growing:
    /// `pass` walks it once from an entry, the next entry is the last one
    /// with what the pass ended with. Returns the last entry and the last
    /// pass's ends.
    fn repeat(
        &mut self,
        mut entry: Left<'db>,
        mut pass: impl FnMut(&mut Self, Left<'db>) -> (Left<'db>, Ends<'db>),
    ) -> (Left<'db>, Ends<'db>) {
        let mut last = Ends::default();
        for _ in 0..PASSES {
            let (walked, ends) = pass(self, entry.clone());
            entry = walked;
            let mut next = Some(entry.clone());
            merge(&mut next, ends.through.clone());
            merge(&mut next, ends.continues.clone());
            let next = next.unwrap_or_default();
            last = ends;
            if next.len() == entry.len() {
                break;
            }
            entry = next;
        }
        (entry, last)
    }

    /// The places `walk` names, in order: the first read of a counter that
    /// holds what its loop left is reported, a write of the whole variable
    /// gives it a value of its own again.
    fn places(&mut self, left: &mut Left<'db>, walk: impl FnOnce(&mut dyn FnMut(Place<'db>, Use))) {
        let mut seen = Vec::new();
        walk(&mut |place, how| seen.push((place, how)));
        for (place, how) in seen {
            match (place, how) {
                (Place::Access(access), Use::Write) => {
                    if let Some(root) = self.whole_variable(access) {
                        left.remove(&root);
                    }
                }
                // The first read on a path is reported: the ones after it
                // are the same mistake.
                (Place::Access(access), Use::Read) => {
                    if let Some(root) = self.access_root(access)
                        && let Some(origin) = left.remove(&root)
                    {
                        self.report(&access, root, origin);
                    }
                }
                // `REF(i)` hands out the value the loop left.
                (Place::Referenced(begin), _) => {
                    if let Some(root) = self.path_root(begin)
                        && let Some(origin) = left.remove(&root)
                    {
                        self.report(&begin, root, origin);
                    }
                }
            }
        }
    }

    fn path_root(&self, begin: BeginPathExpr<'db>) -> Option<VariableDecl<'db>> {
        let step = begin.expr(self.db)?.flatten(self.db).first().copied()?;
        self.body.variable_for_path_expr(step.get_expr(self.db))
    }

    fn access_root(&self, access: VariableAccess<'db>) -> Option<VariableDecl<'db>> {
        match access.kind(self.db) {
            VariableAccessKind::Symbolic(begin) => self.path_root(begin),
            VariableAccessKind::Direct(_) => None,
        }
    }

    /// The variable `access` writes whole: `i`, not `a[i]`, `s.f` or a bit
    /// of `i`.
    fn whole_variable(&self, access: VariableAccess<'db>) -> Option<VariableDecl<'db>> {
        let VariableAccessKind::Symbolic(begin) = access.kind(self.db) else {
            return None;
        };
        if access.multibits(self.db).is_some() || begin.expr(self.db)?.flatten(self.db).len() != 1 {
            return None;
        }
        self.path_root(begin)
    }

    /// A read at `site`, reported once however many passes reach it.
    fn report(
        &mut self,
        site: &impl HirNodeInfo<'db>,
        counter: VariableDecl<'db>,
        origin: VariableAccess<'db>,
    ) {
        let db = self.db;
        let span = site.get_span(db);
        if !self.reported.insert((span.start_byte, span.end_byte)) {
            return;
        }
        let name = counter.get_name_with_case(db).text(db);
        let mut d = diag()
            .message(format!("'{name}' is read after the FOR loop it counts"))
            .desc(&LoopVarAfterLoop)
            .range(hir::denormalize(db, site.get_scope_id(db).file(db), &span).unwrap_or_default())
            .severity(DiagnosticSeverity::WARNING)
            .call();
        d.with_related(Related::new(
            format!("'{name}' counts this FOR loop"),
            origin.scope_id(db).file(db),
            origin.get_span(db),
        ));
        d.with_note(
            "IEC 61131-3 leaves the value a FOR loop leaves in its counter to the implementation"
                .to_string(),
        );
        d.with_help(format!(
            "copy '{name}' into a variable of its own inside the loop, or count with a WHILE loop"
        ));
        self.found.push(d);
    }
}

impl<'db> Ends<'db> {
    /// A branch's ends, added to the statement's.
    fn add(&mut self, branch: Ends<'db>) {
        merge(&mut self.through, branch.through);
        merge(&mut self.exits, branch.exits);
        merge(&mut self.continues, branch.continues);
    }
}
