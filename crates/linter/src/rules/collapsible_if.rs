use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::{
        expression::Expr,
        statement::{Stmt, StmtKind},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "collapsible-if";

/// L0209: nested IF without ELSE can be collapsed into `IF a AND b THEN`.
struct CollapsibleIf;

impl ErrorCode for CollapsibleIf {
    fn code(&self) -> &'static str {
        "L0209"
    }

    fn description(&self) -> &'static str {
        "collapsible IF statements"
    }
}

pub fn check_if<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmt: Stmt<'db>,
    outer_condition: &Expr<'db>,
    then: &Option<Vec<Stmt<'db>>>,
    else_if: &[(Expr<'db>, Vec<Stmt<'db>>)],
    else_: &Option<Vec<Stmt<'db>>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // Outer IF must have no ELSIF and no ELSE
    if !else_if.is_empty() || else_.is_some() {
        return;
    }

    // Then branch must have exactly one statement
    let Some(then_stmts) = then else { return };
    if then_stmts.len() != 1 {
        return;
    }

    // That single statement must be an IF with no ELSIF and no ELSE
    let inner_stmt = then_stmts[0];
    let StmtKind::If {
        condition: inner_condition,
        else_if: inner_else_if,
        else_: inner_else,
        ..
    } = inner_stmt.stmt(db)
    else {
        return;
    };

    if !inner_else_if.is_empty() || inner_else.is_some() {
        return;
    }

    let file = stmt.get_scope_id(db).file(db);
    let outer_text = outer_condition.as_call_site(db).to_string(db);
    let inner_text = inner_condition.as_call_site(db).to_string(db);

    let mut diag = diag()
        .message("IF statements can be collapsed into a single one".to_string())
        .desc(&CollapsibleIf)
        .range(stmt.get_span(db))
        .severity(DiagnosticSeverity::HINT)
        .call();

    diag.with_related(Related::new(
        "the condition of this IF statement...".to_string(),
        file,
        inner_condition.get_span(db),
    ));

    diag.with_related(Related::new(
        format!("...can be merged here: IF '{outer_text} AND {inner_text}' THEN"),
        file,
        outer_condition.get_span(db),
    ));

    diagnostics.push(diag);
}
