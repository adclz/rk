use auto_lsp::{
    core::{document_symbols_builder::DocumentSymbolsBuilder, semantic_tokens_builder::SemanticTokensBuilder}, default::db::BaseDatabase, define_semantic_token_modifiers, define_semantic_token_types, lsp_types::{
        CodeLens, CompletionItem, GotoDefinitionResponse, Hover, InlayHint,
        request::{GotoDeclarationResponse, GotoImplementationResponse},
    }
};
use hir::{
    HirNodeInfo, hir_def::{expressions::statement::StmtKind}
};

pub mod completions;
pub mod namespace_access;
pub mod method_ref;
pub mod namespace;
pub mod pou;
pub mod expr;
pub mod init_expr;
pub mod path_expr;
pub mod using;
pub mod spec;
pub mod struct_element;
pub mod variable;
pub mod implementation;
pub mod comment_index;
pub mod var_access;
pub mod typ;
pub mod begin_path_expr;
pub mod param;
pub mod to_proto;

define_semantic_token_types![
    standard {
        NAMESPACE,
        FUNCTION,
        INTERFACE,
        CLASS,
    }

    custom {

    }
];

define_semantic_token_modifiers![
    standard {

    }

    custom {
        //(INTERNAL, "internal"),
    }
];