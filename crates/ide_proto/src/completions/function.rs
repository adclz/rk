use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::{hir_def::pous::{function::Function, function_block::FunctionBlock}, HirNodeInfo};

use crate::{
    completions,
    pou::{CursorLocation, PrecizeCompletion},
};

impl<'db> PrecizeCompletion<'db> for Function<'db> {
    fn completion(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Vec<CompletionItem> {
        let mut results = vec![];

        match self.location(
            db,
            None,
            None,
            Some(self.variables(db)),
            None,
            offset,
        ) {
            CursorLocation::BeforeExtends => {
            },
            CursorLocation::BeforeImplements => {
            }
            // Before the first variable, both variables and methods can be suggested
            CursorLocation::BeforeVariables => results.extend(vec![
                completions::static_snippets::var_input(),
                completions::static_snippets::var_output(),
                completions::static_snippets::var_in_out(),
                completions::static_snippets::var_temp(),
                completions::static_snippets::var(),
            ]),
            CursorLocation::AfterVariables | CursorLocation::BeforeMethods | CursorLocation::AfterMethods => results.extend(vec![
                completions::static_snippets::var_input(),
                completions::static_snippets::var_output(),
                completions::static_snippets::var_in_out(),
                completions::static_snippets::var_temp(),
                completions::static_snippets::var(),
            ]),
        }

        results
    }
}
