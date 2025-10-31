use auto_lsp::{default::db::BaseDatabase, lsp_types::CompletionItem};
use hir::{HirNodeInfo, hir_def::pous::function_block::FunctionBlock};

use crate::{
    completions,
    pou::{CursorLocation, PrecizeCompletion},
};

impl<'db> PrecizeCompletion<'db> for FunctionBlock<'db> {
    fn completion(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Vec<CompletionItem> {
        let mut results = vec![];

        match self.location(
            db,
            self.extends(db),
            None,
            Some(self.variables(db)),
            Some(self.methods(db)),
            offset,
        ) {
            CursorLocation::BeforeExtends => {
                results.extend(vec![
                    completions::static_snippets::extends(),
                ])
            },
            CursorLocation::BeforeImplements => {
                results.extend(vec![
                    completions::static_snippets::implements(),
                ]);
            }
            // Before the first variable, both variables and methods can be suggested
            CursorLocation::BeforeVariables => results.extend(vec![
                completions::static_snippets::extends(),
                completions::static_snippets::implements(),
                completions::static_snippets::var_input(),
                completions::static_snippets::var_output(),
                completions::static_snippets::var_in_out(),
                completions::static_snippets::var_temp(),
                completions::static_snippets::var(),
                completions::static_snippets::method(),
            ]),
            CursorLocation::AfterVariables | CursorLocation::BeforeMethods => results.extend(vec![
                completions::static_snippets::var_input(),
                completions::static_snippets::var_output(),
                completions::static_snippets::var_in_out(),
                completions::static_snippets::var_temp(),
                completions::static_snippets::var(),
                completions::static_snippets::method(),
            ]),
            CursorLocation::AfterMethods => results.extend(vec![
                completions::static_snippets::method(),
                completions::static_snippets::if_(),
                completions::static_snippets::for_(),
                completions::static_snippets::while_(),
                completions::static_snippets::repeat(),
            ]),
        }

        results
    }
}
