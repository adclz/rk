use auto_lsp::lsp_types::CompletionItem;
use db::WorkspaceDataBase;
use hir::{hir_def::{scope::{ScopeId, ScopeKind}, semantic_index::get_scope}, hir_ty::ty::Type};

pub mod completion_item_builder;
pub mod field_strategy;
pub mod pou_context;
pub mod pou_strategy;
pub mod scope_strategy;
pub mod static_snippets;

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryMode {
    Head,
    #[default]
    Body,
}
pub struct CompletionCtx {
    pub offset: usize,
    pub items: Vec<CompletionItem>,
    pub mode: QueryMode,
}

impl CompletionCtx {
    pub fn new(offset: usize, mode: QueryMode) -> Self {
        Self {
            offset,
            items: vec![],
            mode,
        }
    }

    pub fn with_signature(mut self) -> Self {
        self.mode = QueryMode::Head;
        self
    }

    /// Try field completion first, fall back to scope completion if type is unavailable or Never
    pub fn field_or_scope<'db>(
        &mut self,
        ty: Option<Type<'db>>,
        scope: ScopeId<'db>,
        query: &str,
        db: &'db dyn WorkspaceDataBase,
    ) -> &mut Self {
        if let Some(ty) = ty
            && !ty.is_never() {
                self.field_completion(ty, db);
                return self;
            }
              // check if we're in a pou body, if so add all statements as completion items
        if let ScopeKind::Pou(pou) = get_scope(db, scope).kind
            && self.located_pou_completion(pou, db).inside_head.is_in_body()
        {
            self.scope_completion(scope, "", db);
            self.items.extend(static_snippets::all_stmts());
        }
        self
    }

    pub fn take_items(self) -> Vec<CompletionItem> {
        self.items
    }
}
