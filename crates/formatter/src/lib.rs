use std::sync::LazyLock;

use auto_lsp::{
    anyhow,
    default::db::{BaseDatabase, file::File},
    lsp_types::{self, TextEdit},
};
use topiary_core::{Language, Operation, TopiaryQuery, formatter};

static SURROUND_SPACES: &str = r#"
[
    "NAMESPACE" "END_NAMESPACE"
    "FUNCTION" "END_FUNCTION"
    "FUNCTION_BLOCK" "END_FUNCTION_BLOCK"
    "TYPE" "END_TYPE"
    "CLASS" "END_CLASS"
    "INTERFACE" "END_INTERFACE"
    "VAR"
    "VAR_INPUT"
    "VAR_OUTPUT"
    "VAR_IN_OUT"
    "VAR_TEMP"
    "VAR_EXTERNAL"
    "VAR_GLOBAL"
    "END_VAR"
    "USING"
    "FINAL" "ABSTRACT" "OVERRIDE"
    "PUBLIC" "PROTECTED" "PRIVATE" "INTERNAL"
    "IMPLEMENTS" "EXTENDS"
    "METHOD" "END_METHOD"
    "IF" "THEN" "ELSE" "ELSIF"
    "CASE" "OF" "END_CASE"
    "FOR" "TO" "BY" "DO"
    "WHILE" "DO"
    "REPEAT" "UNTIL"
    "RETURN"
    "EXIT"
    "CONTINUE"
    ":=" "=" "<=" "<" ">=" ">" "<>" "+" "-" "*" "/" "%" "^"
    "&" "AND" "OR"
    (identifier)
    (line_comment)
    (c_style_comment)
    (pascal_style_comment)
] @prepend_space @append_space

"(" @append_antispace
")" @prepend_antispace
[":" ";" ","] @prepend_antispace
["NOT" ":"] @append_space
"#;

static NEW_LINES: &str = r#"
[
    "VAR"
    "VAR_INPUT"
    "VAR_OUTPUT"
    "VAR_IN_OUT"
    "VAR_TEMP"
    "VAR_EXTERNAL"
    "VAR_GLOBAL"
    "METHOD"

    "ELSE"
] @prepend_hardline @append_hardline

"STRUCT" @append_hardline

[
    "END_NAMESPACE"
    "END_FUNCTION"
    "END_CLASS"
    "END_FUNCTION_BLOCK"
    "END_TYPE"
    "END_INTERFACE"
    "END_VAR"
    "END_METHOD"

    "END_IF"
    "END_WHILE"
    "END_FOR"
    "UNTIL"
    "END_REPEAT"
    "END_CASE"
    "END_STRUCT"
    (case_selection)

    (var_decl_init_list)
    (input_var) (fb_input_var)
    (output_var) (fb_output_var)
    (in_out_var)
    (temp_var)
    (loc_var_decl)
    (loc_partly_var)
    (external_decl)
    (global_var_decl)
] @prepend_hardline

[
    (assign)
    (func_call)
    (invocation)
    (super_body_invocation)
    "RETURN"
    (if_stmt)
    (case_stmt)
    (for_stmt)
    (while_stmt)
    (repeat_stmt)
    "EXIT"
    "CONTINUE"
] @prepend_spaced_softline

(
  "," @append_spaced_softline
  .
  [(line_comment) (c_style_comment) (pascal_style_comment)]* @do_nothing
)

(
  [";"] @append_hardline
  .
  [(line_comment) (c_style_comment) (pascal_style_comment)]* @do_nothing
)

[
    (line_comment)
    "THEN"
    "ELSE"
    "DO"
    "OF"
] @append_hardline
"#;

static BLOCKS: &str = r#"
(func_call
  "(" @append_spaced_softline @append_indent_start
  ")" @prepend_spaced_softline @prepend_indent_end
)

(invocation
  "(" @append_spaced_softline @append_indent_start
  ")" @prepend_spaced_softline @prepend_indent_end
)

(struct_type_init
  "(" @append_spaced_softline @append_indent_start
  ")" @prepend_spaced_softline @prepend_indent_end
)

(array_type_init
  "[" @append_spaced_softline @append_indent_start
  "]" @prepend_spaced_softline @prepend_indent_end
)
"#;

static INDENTATIONS: &str = r#"
[
    "NAMESPACE"
    "FUNCTION"
    "FUNCTION_BLOCK"
    "TYPE"
    "CLASS"
    "INTERFACE"
    "METHOD"
    "VAR"
    "VAR_INPUT"
    "VAR_OUTPUT"
    "VAR_IN_OUT"
    "VAR_TEMP"
    "VAR_EXTERNAL"
    "VAR_GLOBAL"
    "STRUCT"

    "THEN"
    "ELSE"
    "DO"
    "REPEAT"
    "OF"
] @append_indent_start

; case selection
(case_selection
    case_of: (_) @append_indent_start
    case_do: (stmt_list) @append_indent_end
)

[   "END_NAMESPACE"
    "END_FUNCTION"
    "END_CLASS"
    "END_FUNCTION_BLOCK"
    "END_TYPE"
    "END_INTERFACE"
    "END_METHOD"
    "END_VAR"
    "END_STRUCT"

    "END_IF"
    "END_WHILE"
    "END_FOR"
    "END_REPEAT"
    "END_CASE"
] @prepend_indent_end
"#;

static ALLOW_BLANK_LINE: &str = r#"
[
    (namespace_decl)
    (func_decl)
    (fb_decl)
    (type_decl)
    (class_decl)
    (interface_decl)

    (method_decl)

    (var_decls)
    (input_decls) (fb_input_decls)
    (output_decls) (fb_output_decls)
    (in_out_decls)
    (temp_var_decls)
    (external_var_decls)
    (global_var_decl)

    (func_body)
    (fb_body)

    "RETURN"
    "CONTINUE"
] @allow_blank_line_before

(stmt_list . (_) @allow_blank_line_before)
"#;

static LEAF: &str = r#"
[
    (line_comment)
    (c_style_comment)
    (pascal_style_comment)
] @leaf
"#;

static SEMI_COLONS: &str = r#"
(
  [
    (var_decl_init_list)
    (input_var) (fb_input_var)
    (output_var) (fb_output_var)
    (in_out_var)
    (temp_var)
    (loc_var_decl)
    (loc_partly_var)
    (external_decl)
    (global_var_decl)
  ] @append_delimiter
  .
  ";"* @do_nothing
  (#delimiter! ";")
)

([
    (assign)
    (invocation)
    (super_body_invocation)
    "RETURN"
    (if_stmt)
    (for_stmt)
    (case_stmt)
    (while_stmt)
    (repeat_stmt)
] @append_delimiter
    .
    ";"* @do_nothing
    (#delimiter! ";")
)

(case_selection ";" @delete)
"#;

pub static TOPIARY_LANG: LazyLock<Language> = LazyLock::new(|| Language {
    name: "IEC".into(),
    grammar: tree_sitter_rk::LANGUAGE.into(),
    query: TopiaryQuery::new(
        &tree_sitter_rk::LANGUAGE.into(),
        &format!(
            r#"
    {SURROUND_SPACES}
    {NEW_LINES}
    {INDENTATIONS}
    {ALLOW_BLANK_LINE}
    {LEAF}
    {SEMI_COLONS}
    {BLOCKS}
"#
        ),
    )
    .unwrap(),
    indent: Some("\t".into()),
});

pub fn format(db: &impl BaseDatabase, file: File) -> anyhow::Result<Option<Vec<TextEdit>>> {
    let document = file.document(db);

    let mut output = vec![];

    formatter(
        &mut document.texter.text.as_bytes(),
        &mut output,
        &TOPIARY_LANG,
        Operation::Format {
            skip_idempotence: true,
            tolerate_parsing_errors: false,
        },
    )
    .unwrap();

    let output = String::from_utf8(output)?;

    Ok(Some(vec![TextEdit::new(
        auto_lsp::lsp_types::Range {
            start: lsp_types::Position::new(0, 0),
            end: lsp_types::Position::new(document.texter.br_indexes.0.len() as u32, 0),
        },
        output,
    )]))
}
