#![allow(deprecated)]
use auto_lsp::{define_semantic_token_modifiers, define_semantic_token_types};

pub mod comment_index;
pub mod handlers;
pub mod hir_node;
pub mod walk;

define_semantic_token_types![
    standard {
        NAMESPACE,
        FUNCTION,
        METHOD,
        INTERFACE,
        CLASS,
        STRUCT,
        ENUM,
        ENUM_MEMBER,
        EVENT,
        // What a name IS, which the legend could not say: every variable was
        // reported as whatever its TYPE is, so an enum-typed one coloured as
        // the enum and a block-typed one as the block.
        VARIABLE,
        PARAMETER,
        PROPERTY
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
