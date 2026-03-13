use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        pous::{class::Class, function::Function, function_block::FunctionBlock, pou::Pou},
        program::ProgramDecl,
    },
    hir_ty::head::inheritance::MethodRef,
};

use crate::handlers::completions_utils::{
    CompletionCtx,
    pou_context::{HeadLocation, HeadResult, VarSection},
    static_snippets,
};

trait AllowedVarSections {
    fn allowed() -> VarSection;
}

impl AllowedVarSections for Function<'_> {
    fn allowed() -> VarSection {
        VarSection::INPUTS
            | VarSection::OUTPUTS
            | VarSection::IN_OUTS
            | VarSection::TEMPS
            | VarSection::VARS
    }
}

impl AllowedVarSections for Class<'_> {
    fn allowed() -> VarSection {
        VarSection::INPUTS
            | VarSection::OUTPUTS
            | VarSection::IN_OUTS
            | VarSection::TEMPS
            | VarSection::VARS
    }
}

impl AllowedVarSections for FunctionBlock<'_> {
    fn allowed() -> VarSection {
        VarSection::INPUTS
            | VarSection::OUTPUTS
            | VarSection::IN_OUTS
            | VarSection::TEMPS
            | VarSection::VARS
    }
}

impl AllowedVarSections for ProgramDecl<'_> {
    fn allowed() -> VarSection {
        VarSection::INPUTS
            | VarSection::OUTPUTS
            | VarSection::IN_OUTS
            | VarSection::TEMPS
            | VarSection::VARS
    }
}

impl<'db> CompletionCtx {
    pub fn located_pou_completion(
        &mut self,
        pou: Pou<'db>,
        db: &'db dyn WorkspaceDataBase,
    ) -> HeadResult {
        let doc = pou.get_scope_id(db).file(db).document(db);
        let root_node = doc.tree.root_node();
        let source = &doc.texter.text;
        let range = pou.get_span(db);

        let ctx = HeadResult::query_var_decls(root_node, source, range, self.offset);

        // If we're inside a variable declaration, don't provide completions
        // (they should be handled by that context)
        if ctx.is_inside_var_section() {
            return ctx;
        }

        match pou {
            Pou::Class(cl) => complete_class(cl, &ctx, self.offset, db, &mut self.items),
            Pou::Function(_) => complete_function(pou, &ctx, db, self.offset, &mut self.items),
            Pou::FunctionBlock(fb) => complete_fb(fb, &ctx, self.offset, db, &mut self.items),
            Pou::Interface(_) => {} // todo: add *magic* method completions?
            Pou::DataType(_) => {}  // will be handled by Spec
        }
        ctx
    }

    pub fn located_method_completion(
        &mut self,
        method: MethodRef<'db>,
        db: &'db dyn WorkspaceDataBase,
    ) -> HeadResult {
        let doc = method.get_scope_id(db).file(db).document(db);
        let root_node = doc.tree.root_node();
        let source = &doc.texter.text;
        let range = method.get_span(db);

        let mut ctx = HeadResult::query_var_decls(root_node, source, range, self.offset);

        // The method_decl node captures itself as @method, which makes
        // query_var_decls think we're "InMethods". Since we know we're
        // inside this specific method, treat InMethods as InBody.
        if ctx.head_location == HeadLocation::InMethods {
            ctx.head_location = HeadLocation::InBody;
        }

        if ctx.is_inside_var_section() {
            return ctx;
        }

        // Methods have the same var section structure as functions
        match ctx.head_location {
            HeadLocation::BeforeVars
            | HeadLocation::InVars
            | HeadLocation::InBodyAfterVars => {
                add_var_snippets(
                    Function::allowed().difference(ctx.active_variable_sections()),
                    &mut self.items,
                );
            }
            _ => {}
        }

        ctx
    }

    pub fn located_program_completion(
        &mut self,
        program: ProgramDecl<'db>,
        db: &'db dyn WorkspaceDataBase,
    ) -> HeadResult {
        let doc = program.scope_id(db).file(db).document(db);
        let root_node = doc.tree.root_node();
        let source = &doc.texter.text;
        let range = program.get_span(db);

        let ctx = HeadResult::query_var_decls(root_node, source, range, self.offset);

        if ctx.is_inside_var_section() {
            return ctx;
        }

        complete_program(program, &ctx, &mut self.items);
        ctx
    }
}

fn complete_program(_program: ProgramDecl, ctx: &HeadResult, items: &mut Vec<CompletionItem>) {
    match ctx.head_location {
        HeadLocation::BeforeVars
        | HeadLocation::InVars
        | HeadLocation::BeforeMethods
        | HeadLocation::InBodyAfterVars
        | HeadLocation::InBodyAfterMethods => {
            add_var_snippets(
                ProgramDecl::allowed().difference(ctx.active_variable_sections()),
                items,
            );
        }
        HeadLocation::InMethods | HeadLocation::InBody => {}
    }
}

fn complete_class(
    cl: Class,
    ctx: &HeadResult,
    _offset: usize,
    db: &dyn WorkspaceDataBase,
    items: &mut Vec<CompletionItem>,
) {
    match ctx.head_location {
        HeadLocation::BeforeVars => {
            add_var_snippets(
                Class::allowed().difference(ctx.active_variable_sections()),
                items,
            );
            if cl.extends(db).is_none() {
                items.push(static_snippets::extends());
            }
            if cl.implements(db).is_empty() {
                items.push(static_snippets::implements());
            }
        }
        HeadLocation::InVars => {
            add_var_snippets(
                Class::allowed().difference(ctx.active_variable_sections()),
                items,
            );
        } //
        HeadLocation::BeforeMethods => {
            add_var_snippets(
                Class::allowed().difference(ctx.active_variable_sections()),
                items,
            );
            items.push(static_snippets::method());
        }
        HeadLocation::InMethods | HeadLocation::InBodyAfterMethods => {
            items.push(static_snippets::method());
        }
        HeadLocation::InBodyAfterVars => {
            add_var_snippets(
                Class::allowed().difference(ctx.active_variable_sections()),
                items,
            );
            items.push(static_snippets::method());
        }
        HeadLocation::InBody => {}
    }
}

fn complete_fb(
    cl: FunctionBlock,
    ctx: &HeadResult,
    _offset: usize,
    db: &dyn WorkspaceDataBase,
    items: &mut Vec<CompletionItem>,
) {
    match ctx.head_location {
        HeadLocation::BeforeVars => {
            add_var_snippets(
                FunctionBlock::allowed().difference(ctx.active_variable_sections()),
                items,
            );
            if cl.extends(db).is_none() {
                items.push(static_snippets::extends());
            }
            if cl.implements(db).is_empty() {
                items.push(static_snippets::implements());
            }
        }
        HeadLocation::InVars => {
            add_var_snippets(
                FunctionBlock::allowed().difference(ctx.active_variable_sections()),
                items,
            );
        } //
        HeadLocation::BeforeMethods => {
            add_var_snippets(
                FunctionBlock::allowed().difference(ctx.active_variable_sections()),
                items,
            );
            items.push(static_snippets::method());
        }
        HeadLocation::InMethods | HeadLocation::InBodyAfterMethods => {
            items.push(static_snippets::method());
        }
        HeadLocation::InBodyAfterVars => {
            add_var_snippets(
                FunctionBlock::allowed().difference(ctx.active_variable_sections()),
                items,
            );
            items.push(static_snippets::method());
        }
        HeadLocation::InBody => {}
    }
}

fn complete_function(
    _pou: Pou,
    ctx: &HeadResult,
    _db: &dyn WorkspaceDataBase,
    _offset: usize,
    items: &mut Vec<CompletionItem>,
) {
    match ctx.head_location {
        HeadLocation::BeforeVars | HeadLocation::InVars => {
            add_var_snippets(
                Function::allowed().difference(ctx.active_variable_sections()),
                items,
            );
        }
        HeadLocation::BeforeMethods | HeadLocation::InMethods => {
            add_var_snippets(
                Function::allowed().difference(ctx.active_variable_sections()),
                items,
            );
        }
        HeadLocation::InBodyAfterMethods | HeadLocation::InBodyAfterVars => {
            add_var_snippets(
                Function::allowed().difference(ctx.active_variable_sections()),
                items,
            );
        }
        HeadLocation::InBody => {}
    }
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
