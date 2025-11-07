use auto_lsp::{core::span::Span, default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::{
    HirNodeInfo,
    hir_def::pous::{function::Function, function_block::FunctionBlock, interface::Interface},
};

use crate::completions::{self, context::{PouCompletionCtx, PrecizeCompletion, ScopeCompletionCtx}};

impl<'db, 'scope> PrecizeCompletion<'db, 'scope> for Interface<'db> {
    fn head_completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        ctx: PouCompletionCtx<'db, 'scope>,
    ) {
        if ctx.scope_ctx.offset <= ctx.name_span.start_byte {
            // Before the name of POU, returns nothing
            return
        }
        ctx.scope_ctx.items.extend(vec![completions::static_snippets::method()]);
    }
}
