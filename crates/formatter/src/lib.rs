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
    (public) (protected) (private) (internal)
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
    "REF_TO"
    ":=" "=" "<=" "<" ">=" ">" "<>" "+" "-" "*" "/" "%"
    "&" "AND" "OR" "NOT"
    (line_comment)
    (c_style_comment)
    (pascal_style_comment)
] @prepend_space @append_space

(identifier) @prepend_space
["(" "[" "."] @append_antispace
[")" "]" ":" ";" "," "." "^"] @prepend_antispace
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

    "ELSE"
] @prepend_hardline @append_hardline

["USING" "METHOD"] @prepend_hardline
["TYPE" "STRUCT"] @append_hardline
(namespace_decl . (namespace_h_name) @append_hardline)

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
    (struct_elem_decl)
] @prepend_hardline

 [
    "END_NAMESPACE"
    "END_FUNCTION"
    "END_CLASS"
    "END_FUNCTION_BLOCK"
    "END_TYPE"
    "END_INTERFACE"
    "END_VAR"
    "END_METHOD"
] @append_hardline
("USING" (_) ";" @append_hardline)

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
  ";" @append_spaced_softline
  .
  [(line_comment) (c_style_comment) (pascal_style_comment)]* @do_nothing
)  

[(line_comment) (c_style_comment) (pascal_style_comment)] @prepend_input_softline

(
  [(line_comment) (c_style_comment) (pascal_style_comment)] @append_input_softline
  .
  [ "," ";" ]* @do_nothing
)

(stmt_list  ";" @do_nothing)

[
    (line_comment)
    "THEN"
    "ELSE"
    "DO"
] @append_hardline

(case_stmt "OF" @append_hardline)
"#;

static BLOCKS: &str = r#"
(func_call
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

(using_directive 
    . (_) "," @append_spaced_softline @append_indent_start
	(namespace_h_name) @prepend_spaced_softline @append_indent_end
	. 
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
] @append_indent_start

(case_stmt "OF" @append_indent_start)

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
    (data_type_decl)
    (class_decl)
    (interface_decl)
    (method_decl)
    (using_directive)

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
    "NAMESPACE" "END_NAMESPACE"
    "FUNCTION" "END_FUNCTION"
    "CLASS" "END_CLASS"
    "FUNCTION_BLOCK" "END_FUNCTION_BLOCK"
    "TYPE" "END_TYPE"
    "INTERFACE" "END_INTERFACE"
    "METHOD" "END_METHOD"
    "VAR" "END_VAR"
    "STRUCT" "END_STRUCT"
    (line_comment) (c_style_comment) (pascal_style_comment)
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

    (assign)
    (super_body_invocation)
    "RETURN"
    "CONTINUE"
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

((stmt_list (func_call)*  @append_delimiter
 .
 ";"* @do_nothing
 (#delimiter! ";")
)) 

(using_directive
 "USING" (_) 
	.
    ";"* @do_nothing
    (#delimiter! ";")) @append_delimiter

( 
    (struct_elem_decl) @append_delimiter
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
    {INDENTATIONS}
    {ALLOW_BLANK_LINE}
    {LEAF}
    {SEMI_COLONS}
    {BLOCKS}
    {NEW_LINES}
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
    ).map_err(|e| anyhow::anyhow!("could not format document: {}", e))?;

    let output = String::from_utf8(output)?;

    Ok(Some(vec![TextEdit::new(
        auto_lsp::lsp_types::Range {
            start: lsp_types::Position::new(0, 0),
            end: lsp_types::Position::new(document.texter.br_indexes.0.len() as u32, 0),
        },
        output,
    )]))
}
