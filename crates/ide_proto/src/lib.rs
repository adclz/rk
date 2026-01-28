#![allow(deprecated)]
use auto_lsp::{define_semantic_token_modifiers, define_semantic_token_types};

pub mod comment_index;
pub mod hir_node;
pub mod handlers;
pub mod walk;

define_semantic_token_types![
    standard {
        NAMESPACE,
        FUNCTION,
        METHOD,
        INTERFACE,
        CLASS,
        STRUCT,
        ENUM
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
