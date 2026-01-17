use db::WorkspaceDataBase;
use hir::hir_def::pous::interface::Interface;

use crate::completions::{
    self,
    context::{PouCompletionCtx, PrecizeCompletion},
};

impl<'db, 'scope> PrecizeCompletion<'db, 'scope> for Interface<'db> {
    fn head_completion(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        ctx: PouCompletionCtx<'db, 'scope>,
    ) {
        if ctx.scope_ctx.offset <= ctx.name_span.start_byte {
            // Before the name of POU, returns nothing
            return;
        }
        ctx.scope_ctx
            .items
            .extend(vec![completions::static_snippets::method()]);
    }
}
