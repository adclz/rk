use auto_lsp::default::db::BaseDatabase;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::pous::function::Function};

use crate::completions::{
    context::{PouCompletionCtx, PrecizeCompletion},
    static_snippets::{all_stmts, fn_var_snippets},
};

impl<'db, 'scope> PrecizeCompletion<'db, 'scope> for Function<'db> {
    fn head_completion(&'db self, db: &'db dyn WorkspaceDataBase, ctx: PouCompletionCtx<'db, 'scope>) {
        if ctx.scope_ctx.offset <= ctx.name_span.start_byte {
            // Before the name of POU, returns nothing
            return;
        }
        match (self.variables(db).first(), self.statements(db).first()) {
            // If we are before variables, suggest both extends and implements and variable snippets
            (Some(var), _) if ctx.scope_ctx.offset < var.get_span(db).end_byte => {
                ctx.scope_ctx.items.extend(fn_var_snippets());
            }
            // If we are before statements, suggest variable snippets and statement snippets
            (_, Some(stmt)) if ctx.scope_ctx.offset < stmt.get_span(db).end_byte => {
                ctx.scope_ctx.items.extend(fn_var_snippets());
                ctx.scope_ctx.items.extend(all_stmts());
                ctx.scope_ctx.query_scope_items(db);
            }
            _ => {
                ctx.scope_ctx.items.extend(all_stmts());
                ctx.scope_ctx.items.extend(fn_var_snippets());
                ctx.scope_ctx.query_scope_items(db);
            }
        }
    }
}
