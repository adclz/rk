use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::pous::{class::Class, pou::Pou}};

use crate::handlers::completions_utils::{
    pou_context::{HeadLocation, HeadResult, VarSection},
    static_snippets,
    scope::{QueryMode, ScopeCompletionCtx},
};

/// Handles completions for a Pou (Function, FunctionBlock, Class, Interface, DataType)
/// Returns `None` if no completions should be provided (e.g., inside var declarations)
/// Returns `Some(())` with completions added to the items vec
pub fn complete_pou(
    pou: Pou,
    db: &dyn WorkspaceDataBase,
    offset: usize,
    items: &mut Vec<CompletionItem>,
) -> Option<()> {
    let doc = pou.get_scope_id(db).file(db).document(db);
    let root_node = doc.tree.root_node();
    let source = &doc.texter.text;
    let range = *pou.get_span(db).ts();

    let ctx = HeadResult::query_var_decls(root_node, source, range, offset);
    
    // If we're inside a variable declaration, don't provide completions
    // (they should be handled by that context)
    if ctx.is_inside_var_section() {
        return None;
    }

    match pou {
        Pou::Class(cl) => complete_class(cl, ctx, db, items),
        Pou::Function(_) => complete_function(pou, ctx, db, offset, items),
        Pou::FunctionBlock(_) => complete_function_block(pou, ctx, db, offset, items),
        Pou::Interface(_) => {},
        Pou::DataType(_) => {},
    }

    Some(())
}

fn complete_class(
    cl: Class,
    ctx: HeadResult,
    db: &dyn WorkspaceDataBase,
    items: &mut Vec<CompletionItem>,
) {
    match ctx.inside_head {
        HeadLocation::BeforeVars => {
            let allowed = VarSection::INPUTS
                | VarSection::OUTPUTS
                | VarSection::IN_OUTS
                | VarSection::TEMPS
                | VarSection::VARS;
            add_var_snippets(
                allowed.difference(ctx.active_variable_sections()),
                items,
            );
            if cl.extends(db).is_none() {
                items.push(static_snippets::extends());
            }
            if cl.implements(db).is_empty() {
                items.push(static_snippets::implements());
            }
        }
        HeadLocation::InVars => {} // No completions inside var declarations
        HeadLocation::BeforeMethods => {
            let allowed = VarSection::INPUTS
                | VarSection::OUTPUTS
                | VarSection::IN_OUTS
                | VarSection::TEMPS
                | VarSection::VARS;
            add_var_snippets(
                allowed.difference(ctx.active_variable_sections()),
                items,
            );
            items.push(static_snippets::method());
        }
        HeadLocation::InMethods => {
            items.push(static_snippets::method());
        }
        HeadLocation::InStmts => {
            items.push(static_snippets::method());
        }
    }
}

fn complete_function(
    pou: Pou,
    ctx: HeadResult,
    db: &dyn WorkspaceDataBase,
    offset: usize,
    items: &mut Vec<CompletionItem>,
) {
    match ctx.inside_head {
        HeadLocation::BeforeVars | HeadLocation::InVars => {
            let allowed = VarSection::INPUTS
                | VarSection::OUTPUTS
                | VarSection::IN_OUTS
                | VarSection::TEMPS
                | VarSection::VARS;
            add_var_snippets(
                allowed.difference(ctx.active_variable_sections()),
                items,
            );
        }
        HeadLocation::BeforeMethods | HeadLocation::InMethods | HeadLocation::InStmts => {
            // In body context: add statements and scope items
            items.extend(static_snippets::all_stmts());
            
            let mut scope_ctx = ScopeCompletionCtx::new(
                QueryMode::Body,
                pou.get_scope_id(db),
                offset,
                "",
            );
            scope_ctx.query_scope_items(db);
            items.extend(scope_ctx.take_items());
        }
    }
}

fn complete_function_block(
    pou: Pou,
    ctx: HeadResult,
    db: &dyn WorkspaceDataBase,
    offset: usize,
    items: &mut Vec<CompletionItem>,
) {
    // FunctionBlock follows same pattern as Function for now
    complete_function(pou, ctx, db, offset, items)
}

fn add_var_snippets(include: VarSection, items: &mut Vec<CompletionItem>) {
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
