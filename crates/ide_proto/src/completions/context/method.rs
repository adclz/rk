use auto_lsp::default::db::BaseDatabase;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::pous::class::MethodDecl};

use crate::completions::{
    context::{PouCompletionCtx, PrecizeCompletion},
    static_snippets::{all_stmts, method_var_snippets},
};

impl<'db, 'scope> PrecizeCompletion<'db, 'scope> for MethodDecl<'db> {
    fn head_completion(&'db self, db: &'db dyn WorkspaceDataBase, ctx: PouCompletionCtx<'db, 'scope>) {
        if ctx.scope_ctx.offset <= ctx.name_span.start_byte {
            // Before the name of POU, returns nothing
            return;
        }

        match (self.variables(db).first(), self.stmts(db).first()) {
            // If we are before first variable, suggest variable snippets
            (Some(var), _) if ctx.scope_ctx.offset < var.get_span(db).end_byte => {
                ctx.scope_ctx.items.extend(method_var_snippets());
            }
            // If we are before statements, suggest variable snippets and statement snippets
            (_, Some(stmt)) if ctx.scope_ctx.offset < stmt.get_span(db).end_byte => {
                ctx.scope_ctx.items.extend(method_var_snippets());
                ctx.scope_ctx.items.extend(all_stmts());
                ctx.scope_ctx.query_scope_items(db);
            }
            // we are in statements
            (_, Some(stmt)) if ctx.scope_ctx.offset > stmt.get_span(db).end_byte => {
                ctx.scope_ctx.items.extend(all_stmts());
                ctx.scope_ctx.query_scope_items(db);
            }
            _ => {
                // method is likely empty, suggest all
                ctx.scope_ctx.items.extend(method_var_snippets());
                ctx.scope_ctx.items.extend(all_stmts());
                ctx.scope_ctx.query_scope_items(db);
            }
        }
    }
}
