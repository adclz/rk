use auto_lsp::{core::span::Span, default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::{
    HirNodeInfo,
    hir_def::pous::{class::Class, function::Function, function_block::FunctionBlock},
};

use crate::completions::{self, context::{PouCompletionCtx, PrecizeCompletion, ScopeCompletionCtx}, static_snippets::{all_stmts, class_var_snippets, method}};

impl<'db, 'scope> PrecizeCompletion<'db, 'scope> for Class<'db> {
    fn head_completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        ctx: PouCompletionCtx<'db, 'scope>,
    ) {
        if ctx.scope_ctx.offset <= ctx.name_span.start_byte {
            // Before the name of POU, returns nothing
            return
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

        match (self.variables(db).last(), self.methods(db).last()) {
            (None, None) => {
                ctx.scope_ctx.items.extend(suggest);
                ctx.scope_ctx.items.extend(class_var_snippets());
                return;
            }
            // If we are in variables, suggest both extends and implements and variable snippets
            (Some(var), _) if ctx.scope_ctx.offset < var.get_span(db).start_byte => {
                ctx.scope_ctx.items.extend(suggest);
                ctx.scope_ctx.items.extend(class_var_snippets());
                return;
            }
            // If we are in methods, suggest variable, method snippets
            (_, Some(m)) if ctx.scope_ctx.offset < m.get_span(db).start_byte => {
                ctx.scope_ctx.items.extend(class_var_snippets());
                ctx.scope_ctx.items.push(method());
                return;
            }
            _ => { /* after variables and statements, do nothing */ }
        }
    }
}
