#![allow(deprecated)]
use auto_lsp::{define_semantic_token_modifiers, define_semantic_token_types};

pub mod begin_path_expr;
pub mod comment_index;
pub mod completions;
pub mod expr;
pub mod implementation;
pub mod init_expr;
pub mod method_ref;
pub mod namespace;
pub mod namespace_access;
pub mod param;
pub mod path_expr;
pub mod pou;
pub mod spec;
pub mod struct_element;
pub mod to_proto;
pub mod typ;
pub mod using;
pub mod var_access;
pub mod variable;

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
