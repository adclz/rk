// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A VAR of a FUNCTION_BLOCK or PROGRAM that every run of the body writes
//! before it reads it: the value it keeps between runs is never read, so it
//! could be a VAR_TEMP. Every path through one run of the body is followed,
//! and a write counts on a path only when it writes the whole variable.

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HasQualifiers, HirNodeInfo, Qualifier,
    hir_def::{
        expressions::{
            expression::{PathExprKind, VariableAccess, VariableAccessKind},
            statement::{Stmt, StmtKind},
        },
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::ScopeId,
        semantic_index::semantic_index,
    },
    hir_ty::{body::ScopeInference, expr_store::PathExprWalkStep},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashSet;

use super::accesses::{self, Place, Use};

pub const NAME: &str = "var-could-be-temp";

/// L0317: a VAR whose value between runs is never read.
struct VarCouldBeTemp;

impl ErrorCode for VarCouldBeTemp {
    fn code(&self) -> &'static str {
        "L0317"
    }
}

/// The variables written whole on every path so far.
type Written<'db> = FxHashSet<VariableDecl<'db>>;

/// `into` holds what both hold: a variable is written after a join only
/// when every path into it wrote it.
fn meet<'db>(into: &mut Option<Written<'db>>, from: Option<Written<'db>>) {
    let Some(from) = from else {
        return;
    };
    match into {
        Some(into) => into.retain(|var| from.contains(var)),
        None => *into = Some(from),
    }
}

#[derive(Default)]
struct Ends<'db> {
    through: Option<Written<'db>>,
    exits: Option<Written<'db>>,
    continues: Option<Written<'db>>,
}

impl<'db> Ends<'db> {
    fn add(&mut self, branch: Ends<'db>) {
        meet(&mut self.through, branch.through);
        meet(&mut self.exits, branch.exits);
        meet(&mut self.continues, branch.continues);
    }
}

struct Walk<'db> {
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    candidates: FxHashSet<VariableDecl<'db>>,
    /// Read before every path wrote it, or handed out by `REF()`.
    stale: FxHashSet<VariableDecl<'db>>,
    /// Written whole somewhere.
    written: FxHashSet<VariableDecl<'db>>,
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let sema = semantic_index(db, file);
    let mut blocks: Vec<(ScopeId<'db>, &'db [VariableDecl<'db>], &'db [Stmt<'db>])> = Vec::new();
    let pous = sema.global_pous.iter().copied().chain(
        sema.namespaces
            .iter()
            .flat_map(|ns| ns.pous(db).iter().copied()),
    );
    for pou in pous {
        if let Pou::FunctionBlock(fb) = pou {
            blocks.push((pou.get_scope_id(db), fb.variables(db), fb.statements(db)));
        }
    }
    for program in sema.programs.iter() {
        blocks.push((
            program.scope_id(db),
            program.variables(db),
            program.statements(db),
        ));
    }
    if blocks.is_empty() {
        return;
    }
    let outside = named_through_a_path(db);
    for (scope, variables, statements) in blocks {
        // A `{wasm}` statement names variables the walk does not see.
        if contains_wasm(db, statements) {
            continue;
        }
        let body = scope.inference(db);
        let mut walk = Walk {
            db,
            body,
            candidates: variables
                .iter()
                .copied()
                .filter(|var| {
                    var.kind(db) == VariableKind::Var
                        && var.location(db).is_none()
                        && !var
                            .get_qualifiers(db)
                            .intersects(Qualifier::RETAIN | Qualifier::CONSTANT)
                })
                .collect(),
            stale: FxHashSet::default(),
            written: FxHashSet::default(),
        };
        walk.block(statements, Written::default());
        let mut methods_use = FxHashSet::default();
        if let Some(methods) = scope.method_declarations(db) {
            for method in methods {
                methods_use.extend(method.get_scope_id(db).inference(db).variables_used());
            }
        }
        for var in variables {
            if !walk.candidates.contains(var)
                || !walk.written.contains(var)
                || walk.stale.contains(var)
                || methods_use.contains(var)
                || outside.contains(var)
            {
                continue;
            }
            let name = var.get_name_with_case(db).text(db);
            let mut d = diag()
                .message(format!(
                    "'{name}' is written before it is read each time the body runs"
                ))
                .desc(&VarCouldBeTemp)
                .range(
                    hir::denormalize(db, var.get_scope_id(db).file(db), &var.get_name_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::HINT)
                .call();
            d.with_note(format!(
                "the value '{name}' keeps from one call to the next is never read"
            ));
            d.with_help(format!("declare '{name}' in a VAR_TEMP section"));
            diagnostics.push(d);
        }
    }
}

/// Whether a `{wasm}` statement is anywhere in `stmts`.
fn contains_wasm<'db>(db: &'db dyn WorkspaceDataBase, stmts: &[Stmt<'db>]) -> bool {
    let mut found = false;
    let mut visit = |stmts: &[Stmt<'db>], stack: &mut Vec<Vec<Stmt<'db>>>| {
        for stmt in stmts {
            match stmt.stmt(db) {
                StmtKind::WasmPragma(_) => found = true,
                StmtKind::If {
                    then,
                    else_if,
                    else_,
                    ..
                } => {
                    stack.extend(then.iter().cloned());
                    stack.extend(else_if.iter().map(|(_, s)| s.clone()));
                    stack.extend(else_.iter().cloned());
                }
                StmtKind::Case { cases, else_, .. } => {
                    stack.extend(cases.iter().map(|(_, s)| s.clone()));
                    stack.extend(else_.iter().cloned());
                }
                StmtKind::For { body, .. }
                | StmtKind::While { body, .. }
                | StmtKind::Repeat { body, .. } => stack.push(body.clone()),
                StmtKind::Try {
                    body,
                    catch,
                    finally,
                } => {
                    stack.push(body.clone());
                    stack.extend(catch.iter().map(|c| c.body.clone()));
                    stack.extend(finally.iter().cloned());
                }
                _ => {}
            }
        }
    };
    let mut stack = vec![stmts.to_vec()];
    while let Some(list) = stack.pop() {
        visit(&list, &mut stack);
    }
    found
}

/// Every member a body of the workspace names through a path, `f.inner`
/// or `THIS.inner`, the configuration's included: a variable read from
/// outside its block keeps a value someone reads.
fn named_through_a_path<'db>(db: &'db dyn WorkspaceDataBase) -> FxHashSet<VariableDecl<'db>> {
    let mut named = FxHashSet::default();
    for file in db.get_files().iter().map(|entry| *entry) {
        let sema = semantic_index(db, file);
        let mut scopes: Vec<ScopeId<'db>> = Vec::new();
        let pous = sema.global_pous.iter().copied().chain(
            sema.namespaces
                .iter()
                .flat_map(|ns| ns.pous(db).iter().copied()),
        );
        for pou in pous {
            if matches!(pou, Pou::DataType(_) | Pou::Interface(_)) {
                continue;
            }
            let scope = pou.get_scope_id(db);
            scopes.push(scope);
            if let Some(methods) = scope.method_declarations(db) {
                scopes.extend(methods.iter().map(|m| m.get_scope_id(db)));
            }
        }
        scopes.extend(sema.programs.iter().map(|program| program.scope_id(db)));
        scopes.extend(sema.configs.iter().map(|config| config.scope_id(db)));
        for scope in scopes {
            let body = scope.inference(db);
            for (path, _) in body.typed_path_exprs() {
                if let PathExprKind::Field(_) = path.expr(db)
                    && let Some(var) = body.variable_for_path_expr(path)
                {
                    named.insert(var);
                }
            }
        }
    }
    named
}

impl<'db> Walk<'db> {
    /// The candidate a place names: `v`, `v[i]` or `v.f` name `v`.
    fn named(&self, access: VariableAccess<'db>) -> Option<VariableDecl<'db>> {
        let VariableAccessKind::Symbolic(begin) = access.kind(self.db) else {
            return None;
        };
        self.path_named(begin.expr(self.db)?)
    }

    fn path_named(
        &self,
        path: hir::hir_def::expressions::expression::PathExpr<'db>,
    ) -> Option<VariableDecl<'db>> {
        path.flatten(self.db).iter().find_map(|step| match step {
            PathExprWalkStep::Field { expr, .. } => self
                .body
                .variable_for_path_expr(*expr)
                .filter(|var| self.candidates.contains(var)),
            _ => None,
        })
    }

    /// Whether `access` writes the whole of what it names, not an element,
    /// a field or a bit.
    fn whole(&self, access: VariableAccess<'db>) -> bool {
        let VariableAccessKind::Symbolic(begin) = access.kind(self.db) else {
            return false;
        };
        access.multibits(self.db).is_none()
            && begin
                .expr(self.db)
                .is_some_and(|path| path.flatten(self.db).len() == 1)
    }

    fn places(
        &mut self,
        written: &mut Written<'db>,
        walk: impl FnOnce(&mut dyn FnMut(Place<'db>, Use)),
    ) {
        let mut seen = Vec::new();
        walk(&mut |place, how| seen.push((place, how)));
        for (place, how) in seen {
            match place {
                Place::Access(access) => {
                    let Some(var) = self.named(access) else {
                        continue;
                    };
                    // A part written keeps the rest of the old value.
                    if how == Use::Write && self.whole(access) {
                        written.insert(var);
                        self.written.insert(var);
                    } else if !written.contains(&var) {
                        self.stale.insert(var);
                    }
                }
                // Handed out, it is read when nobody here knows.
                Place::Referenced(begin) => {
                    if let Some(var) = begin.expr(self.db).and_then(|p| self.path_named(p)) {
                        self.stale.insert(var);
                    }
                }
            }
        }
    }

    fn block(&mut self, stmts: &[Stmt<'db>], written: Written<'db>) -> Ends<'db> {
        let mut ends = Ends {
            through: Some(written),
            ..Default::default()
        };
        for stmt in stmts {
            let Some(written) = ends.through.take() else {
                break;
            };
            let next = self.statement(*stmt, written);
            meet(&mut ends.exits, next.exits);
            meet(&mut ends.continues, next.continues);
            ends.through = next.through;
        }
        ends
    }

    fn statement(&mut self, stmt: Stmt<'db>, mut written: Written<'db>) -> Ends<'db> {
        let db = self.db;
        let through = |written| Ends {
            through: Some(written),
            ..Default::default()
        };
        match stmt.stmt(db) {
            StmtKind::Assignment { .. }
            | StmtKind::FuncCall(_)
            | StmtKind::EmptyPathExpression(_) => {
                self.places(&mut written, |f| accesses::statements(db, &[stmt], f));
                through(written)
            }
            StmtKind::Raise { message } => {
                self.places(&mut written, |f| accesses::expression(db, *message, f));
                Ends::default()
            }
            StmtKind::Return => Ends::default(),
            StmtKind::Exit => Ends {
                exits: Some(written),
                ..Default::default()
            },
            StmtKind::Continue => Ends {
                continues: Some(written),
                ..Default::default()
            },
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                let mut ends = Ends::default();
                self.places(&mut written, |f| accesses::expression(db, *condition, f));
                let branch = self.block(then.as_deref().unwrap_or_default(), written.clone());
                ends.add(branch);
                for (condition, stmts) in else_if {
                    self.places(&mut written, |f| accesses::expression(db, *condition, f));
                    let branch = self.block(stmts, written.clone());
                    ends.add(branch);
                }
                match else_ {
                    Some(stmts) => {
                        let branch = self.block(stmts, written);
                        ends.add(branch);
                    }
                    None => meet(&mut ends.through, Some(written)),
                }
                ends
            }
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                let mut ends = Ends::default();
                self.places(&mut written, |f| {
                    accesses::expression(db, *condition, f);
                    for (labels, _) in cases {
                        for label in labels {
                            accesses::case_label(db, *label, f);
                        }
                    }
                });
                for (_, stmts) in cases {
                    let branch = self.block(stmts, written.clone());
                    ends.add(branch);
                }
                match else_ {
                    Some(stmts) => {
                        let branch = self.block(stmts, written);
                        ends.add(branch);
                    }
                    None => meet(&mut ends.through, Some(written)),
                }
                ends
            }
            // A loop may run no pass: what it writes is not written after
            // it. Its first pass reads what came before.
            StmtKind::For {
                control_variable,
                start,
                end,
                step,
                body,
            } => {
                self.places(&mut written, |f| {
                    accesses::expression(db, *start, f);
                    accesses::place(db, *control_variable, Use::Write, f);
                    accesses::expression(db, *end, f);
                    if let Some(step) = step {
                        accesses::expression(db, *step, f);
                    }
                });
                self.block(body, written.clone());
                through(written)
            }
            StmtKind::While { condition, body } => {
                self.places(&mut written, |f| accesses::expression(db, *condition, f));
                self.block(body, written.clone());
                through(written)
            }
            // A REPEAT runs its body at least once.
            StmtKind::Repeat { condition, body } => {
                let ends = self.block(body, written);
                let mut at_until = ends.through;
                meet(&mut at_until, ends.continues);
                let mut after = at_until.clone();
                if let Some(at_until) = at_until.as_mut() {
                    self.places(at_until, |f| accesses::expression(db, *condition, f));
                }
                meet(&mut after, ends.exits);
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
                let entry = written.clone();
                let mut ends = Ends::default();
                let tried = self.block(body, written);
                ends.add(tried);
                if let Some(catch) = catch {
                    // The body may have raised before any of its writes.
                    let mut handler = entry;
                    if let Some(variable) = catch.variable {
                        self.places(&mut handler, |f| {
                            accesses::place(db, variable, Use::Write, f)
                        });
                    }
                    let handled = self.block(&catch.body, handler);
                    ends.add(handled);
                }
                if let Some(finally) = finally {
                    let entry = ends.through.take().unwrap_or_default();
                    let cleaned = self.block(finally, entry);
                    ends.through = cleaned.through;
                    meet(&mut ends.exits, cleaned.exits);
                    meet(&mut ends.continues, cleaned.continues);
                }
                ends
            }
            StmtKind::WasmPragma(_) | StmtKind::AllowPragma(_) => through(written),
        }
    }
}
