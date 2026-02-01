use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{expressions::expression::PathExpr, namespace::NamespaceDecl, pous::pou::Pou},
};

use crate::handlers::{
    CompletionHandler,
    completions_utils::{
        pou_context::{HeadLocation, HeadResult, VarSection},
        static_snippets,
    },
};

impl<'db> CompletionHandler<'db> for NamespaceDecl<'db> {
    fn completion(
        &'db self,
        _db: &'db dyn WorkspaceDataBase,
        _offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        return Some(vec![
            static_snippets::namespace(),
            static_snippets::using(),
            static_snippets::function(),
            static_snippets::function_block(),
            static_snippets::class(),
            static_snippets::interface(),
            static_snippets::type_(),
        ]);
    }
}

impl<'db> CompletionHandler<'db> for Pou<'db> {
    fn completion(
        &'db self,
        db: &'db dyn WorkspaceDataBase,
        offset: usize,
    ) -> Option<Vec<CompletionItem>> {
        let doc = self.get_scope_id(db).file(db).document(db);

        let root_node = doc.tree.root_node();
        let source = &doc.texter.text;
        let range = *self.get_span(db).ts();

        let ctx = HeadResult::query_var_decls(root_node, source, range, offset);
        if ctx.is_inside_var_section() {
            return None;
        }
        
        let mut results = vec![];
        match self {
            Pou::Class(cl) => match ctx.inside_head {
                HeadLocation::BeforeVars => {
                    var_snippets_filtered(
                        allowed_vars(*self).difference(ctx.active_variable_sections()),
                        &mut results,
                    );
                    if cl.extends(db).is_none() {
                        results.push(static_snippets::extends());
                    }
                    if cl.implements(db).is_empty() {
                        results.push(static_snippets::implements());
                    }
                    return Some(results);
                }
                HeadLocation::InVars => {
                    return Some(results);
                }
                HeadLocation::BeforeMethods => {
                    var_snippets_filtered(
                        allowed_vars(*self).difference(ctx.active_variable_sections()),
                        &mut results,
                    );
                    results.push(static_snippets::method());
                    return Some(results);
                }
                HeadLocation::InMethods => {
                    return Some(vec![static_snippets::method()]);
                }
                HeadLocation::InStmts => {
                    return Some(static_snippets::all_stmts());
                }
            },
            Pou::Function(_) => match ctx.inside_head {
                HeadLocation::BeforeVars => {
                    var_snippets_filtered(
                        allowed_vars(*self).difference(ctx.active_variable_sections()),
                        &mut results,
                    );
                    return Some(results);
                }
                HeadLocation::InVars => {
                    var_snippets_filtered(
                        allowed_vars(*self).difference(ctx.active_variable_sections()),
                        &mut results,
                    );
                    return Some(results);
                }
                HeadLocation::BeforeMethods | HeadLocation::InMethods | HeadLocation::InStmts => {
                    return Some(static_snippets::all_stmts());
                }
            },
            Pou::FunctionBlock(_) => {}
            Pou::Interface(_) => {}
            Pou::DataType(_) => {}
        }
        None
    }
}

#[inline]
fn allowed_vars(pou: Pou<'_>) -> VarSection {
    match pou {
        Pou::Class(_) | Pou::FunctionBlock(_) | Pou::Function(_) => {
            VarSection::INPUTS
                | VarSection::OUTPUTS
                | VarSection::IN_OUTS
                | VarSection::TEMPS
                | VarSection::VARS
        }
        Pou::Interface(_) | Pou::DataType(_) => VarSection::empty(),
    }
}

fn var_snippets_filtered(
    include: VarSection,
    items: &mut Vec<CompletionItem>,
) {
    if include.contains(VarSection::INPUTS) {
        items.push(static_snippets::var_input());
    }
    if include.contains(VarSection::OUTPUTS) {
        items.push(static_snippets::var_output());
    }
    if include.contains(VarSection::IN_OUTS) {
        items.push(static_snippets::var_in_out());
    }
    if include.contains(VarSection::TEMPS) {
        items.push(static_snippets::var_temp());
    }
    if include.contains(VarSection::VARS) {
        items.push(static_snippets::var());
    }
}

impl<'db> CompletionHandler<'db> for PathExpr<'db> {
    fn completion(&'db self, db: &'db dyn WorkspaceDataBase, offset: usize) -> Option<Vec<CompletionItem>> {
        eprintln!("PathExpr completion called at offset {}", offset);
        Some(static_snippets::all_stmts())
    }
}