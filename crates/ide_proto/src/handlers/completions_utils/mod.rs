use auto_lsp::lsp_types::CompletionItem;

pub mod completion_item_builder;
pub mod field_strategy;
pub mod pou_context;
pub mod pou_strategy;
pub mod pragma;
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

    pub fn take_items(self) -> Vec<CompletionItem> {
        self.items
    }
}
