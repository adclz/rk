use auto_lsp::{
    core::span::Span,
    default::db::BaseDatabase,
    lsp_types::{CompletionContext, CompletionItem},
};
use hir::{
    HirNodeInfo, hir_def::{
        expressions::statement::Stmt,
        interned::namespace::SpanNamespaceAccess,
        pous::{
            class::MethodDecl,
            function_block::FunctionBlock,
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::{ScopeId, ScopeKind}, semantic_index::get_scope,
    }, hir_ty::name_res::global_pou_index, query_string::scope::query_scope_items
};

use crate::{
    ToProtocol,
    completions::{
        self,
        item_builder::CompletionBuilder,
        static_snippets::{all_stmts, fb_var_snippets},
    },
};

pub mod class;
pub mod function;
pub mod function_block;
pub mod interface;
pub mod method;

pub trait PrecizeCompletion<'db, 'scope> {
    fn head_completion(
        &'db self,
        db: &'db dyn BaseDatabase,
        ctx: PouCompletionCtx<'db, 'scope>,
    );
}

pub struct ScopeCompletionCtx<'db> {
    pub scope: ScopeId<'db>,
    pub offset: usize,
    pub query: &'db str,
    pub items: Vec<CompletionItem>,
}

pub struct PouCompletionCtx<'db, 'scope> {
    pub scope_ctx: &'scope mut ScopeCompletionCtx<'db>,
    pub name_span: Span,
}

impl<'db> ScopeCompletionCtx<'db> {
    pub fn new(scope: ScopeId<'db>, offset: usize, query: &'db str) -> Self {
        Self {
            scope,
            offset,
            query,
            items: vec![],
        }
    }

    pub fn take_items(&mut self) -> Vec<CompletionItem> {
        std::mem::take(&mut self.items)
    }

    pub fn scoped(mut self, db: &'db dyn BaseDatabase) -> Self {
        match &get_scope(db, self.scope).kind {
            ScopeKind::Pou(p) => match p.pou(db) {
                Pou::FunctionBlock(f) => f.head_completion(
                    db,
                    PouCompletionCtx {
                        scope_ctx: &mut self,
                        name_span: p.name_span(db),
                    },
                ),
                Pou::Function(f) => f.head_completion(
                    db,
                    PouCompletionCtx {
                        scope_ctx: &mut self,
                        name_span: p.name_span(db),
                    },
                ),
                Pou::Interface(i) => i.head_completion(
                    db,
                    PouCompletionCtx {
                        scope_ctx: &mut self,
                        name_span: p.name_span(db), 
                    }
                ),
                Pou::Class(c) => c.head_completion(
                    db,
                    PouCompletionCtx {
                        scope_ctx: &mut self,
                        name_span: p.name_span(db),
                    },
                ),
                Pou::DataType(dt) => {
                    dt.spec(db).completion(db, self.offset)
                        .map(|completions| {
                            self.items.extend_from_slice(&completions);
                        });
                }
            },
            ScopeKind::MethodDecl(m) => {
                m.head_completion(
                    db,
                    PouCompletionCtx {
                        scope_ctx: &mut self,
                        name_span: m.get_name_span(db).unwrap(),
                    },
                );
            },
            _ => { }
        };
        self
    }

    pub fn query_scope_items(&mut self, db: &'db dyn BaseDatabase) {
        let builder = CompletionBuilder::default()
            .with_import(db, self.scope)
            .with_signature();

        let pous = query_scope_items(db, self.query, self.scope, |pou| {
            matches!(pou.pou(db), Pou::Function(_))
        });

        for pou in pous.local_pous {
            self.items.push(builder.build_pou(db, &pou, None));
        }

        for (ns, pou) in pous.need_imports {
            self.items.push(builder.build_pou(db, &pou, Some(&ns)));
        }

        for var in pous.local_variables {
            self.items.push(builder.build_variable(db, &var));
        }
    }
}
