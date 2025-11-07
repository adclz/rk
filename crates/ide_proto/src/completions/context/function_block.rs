use auto_lsp::{core::span::Span, default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::statement::Stmt,
        interned::namespace::SpanNamespaceAccess,
        pous::{
            class::MethodDecl, function_block::FunctionBlock, pou::PouDecl, variable::VariableDecl,
        },
    },
};

use crate::completions::{
    self,
    context::{PouCompletionCtx, PrecizeCompletion, ScopeCompletionCtx},
    static_snippets::{all_stmts, fb_var_snippets, method},
};

impl<'db, 'scope> PrecizeCompletion<'db, 'scope> for FunctionBlock<'db> {
    fn head_completion(&'db self, db: &'db dyn BaseDatabase, ctx: PouCompletionCtx<'db, 'scope>) {
        if ctx.scope_ctx.offset <= ctx.name_span.start_byte {
            // Before the name of POU, returns nothing
            return;
        }

        let suggest = match (self.extends(db), self.implements(db).last()) {
            (None, None) => {
                vec![
                    completions::static_snippets::extends(),
                    completions::static_snippets::implements(),
                ]
            }
            (Some(_), None) => {
                vec![completions::static_snippets::implements()]
            }
            (None, Some(_)) => {
                vec![completions::static_snippets::extends()]
            }
            (Some(_), Some(_)) => {
                vec![]
            }
        };

        match (
            self.variables(db).first(),
            self.methods(db).first(),
            self.statements(db).first(),
        ) {
            // If we are before first variable, suggest both extends and implements and variable snippets
            (Some(var), _, _) if ctx.scope_ctx.offset < var.get_span(db).end_byte => {
                ctx.scope_ctx.items.extend(suggest);
                ctx.scope_ctx.items.extend(fb_var_snippets());
                return;
            }
            // If we are before first method, suggest both extends and implements and variable snippets
            (None, Some(m), _) if ctx.scope_ctx.offset < m.get_span(db).end_byte => {
                ctx.scope_ctx.items.extend(fb_var_snippets());
                ctx.scope_ctx.items.push(method());
                return;
            }
            // If we are in statements, suggest method snippets and statement snippets
            (_, _, Some(stmt)) if ctx.scope_ctx.offset < stmt.get_span(db).end_byte => {
                // if there methods, we can't show variables
                if self.methods(db).first().is_none() {
                    ctx.scope_ctx.items.extend(fb_var_snippets());
                }
                ctx.scope_ctx.items.push(method());
                ctx.scope_ctx.items.extend(all_stmts());
                ctx.scope_ctx.query_scope_items(db);
                return;
            }
            _ => {
                ctx.scope_ctx.items.extend(all_stmts());
                ctx.scope_ctx.items.push(method());
                ctx.scope_ctx.items.extend(fb_var_snippets());
                ctx.scope_ctx.query_scope_items(db);
            }
        }
    }
}
