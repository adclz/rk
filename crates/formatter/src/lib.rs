use std::sync::LazyLock;

use auto_lsp::{
    anyhow,
    default::db::file::File,
    lsp_types::{self, TextEdit},
};

use db::WorkspaceDataBase;
use topiary_core::{Language, Operation, TopiaryQuery, formatter};

static SURROUND_SPACES: &str = r##"
[
    "CONFIGURATION" "END_CONFIGURATION"
    "RESOURCE" "END_RESOURCE"
    "PROGRAM" "END_PROGRAM"
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
    "VAR_ACCESS"
    "VAR_CONFIG"
    "END_VAR"
    "USING"
    "FINAL" "ABSTRACT" "OVERRIDE"
    (public) (protected) (private) (internal) (ref_to)
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
    "TASK" "CONSTANT" "RETAIN" "NON_RETAIN" "WITH" "ON"
    ; Located variables (`x AT %QW28 : INT`), edge qualifiers
    ; (`BOOL R_EDGE`) and VAR_ACCESS directions (`INT READ_ONLY`) all follow
    ; another token. Without a space they GLUE onto it and the file stops
    ; parsing — `BOOLR_EDGE` is not a type.
    "AT" "R_EDGE" "F_EDGE" (ERR_invalid_edge_qualifier)
    (read_only) (read_write)
   
    ":=" "=" "=>" "<=" "<" ">=" ">" "<>" "+" "-" "*" "/" 
    "&" "AND" "OR" "XOR" "MOD" "NOT"
    (line_comment)
    (c_style_comment)
    (pascal_style_comment)
] @prepend_space @append_space

[(identifier) "%"] @prepend_space
["(" "[" "." "END_CASE"] @append_antispace
[")" "]" ":" ";" "," "." (deref_sign)] @prepend_antispace
["NOT" ":"] @append_space

; signed_int and signed_real_value: sign is part of the token, no formatting needed

; A duration's sign is part of the literal too, but the grammar makes it a
; separate anonymous "+"/"-" — the same tokens as binary plus/minus above.
; Spacing it turns `T#-14ms` into `T# - 14ms`, which no longer type-checks.
(time sign: ["+" "-"] @prepend_antispace @append_antispace)
(ltime sign: ["+" "-"] @prepend_antispace @append_antispace)

; Enum value: no space around # (Color#Red, not Color # Red)
(enum_value "#" @prepend_antispace @append_antispace)

; Extern pragma: normalize spacing between children
(extern_pragma "{" @append_antispace)
(extern_pragma "extern" @append_space)
(extern_pragma module: (pragma_string) @append_space)
(extern_pragma name: (pragma_string) @append_space)
(extern_pragma "}" @prepend_antispace)

; Wasm pragma: normalize spacing between children
(wasm_pragma "{" @append_antispace)
(wasm_pragma "wasm" @append_space)
(wasm_pragma type_ref: (identifier) @append_space)
(wasm_pragma instruction: (pragma_string) @append_space)
(wasm_pragma params: (extern_param_list) @prepend_space)
(wasm_pragma result: (extern_result) @prepend_space)
(wasm_pragma "}" @prepend_antispace)

; Shared pragma helpers
(extern_param_list "(" @append_antispace)
(extern_param_list "params" @append_space)
(extern_param_list ")" @prepend_antispace)
(extern_result "(" @append_antispace)
(extern_result "result" @append_space)
(extern_result ")" @prepend_antispace)
(pragma_string) @leaf

; Allow pragma: normalize spacing between children
; In pou_pragma position (above a POU or METHOD) it gets its own line —
; without this it glues onto the preceding END_VAR.
(pou_pragma (allow_pragma) @prepend_hardline)
(allow_pragma "{" @append_antispace)
(allow_pragma "allow" @append_space)
(allow_pragma rule: (pragma_string) @append_space)
(allow_pragma "}" @prepend_antispace)

; Test pragma: on its own line before the POU keyword
(test_pragma) @leaf @append_hardline
"##;

static NEW_LINES: &str = r##"
; VAR sections that never have qualifiers
[
    "VAR_IN_OUT"
    "VAR_TEMP"
    "VAR_ACCESS"
] @prepend_hardline @append_hardline

; VAR sections that can have CONSTANT/RETAIN/NON_RETAIN qualifiers
; (no @append_hardline - declarations already have @prepend_hardline)
[
    "VAR"
    "VAR_INPUT"
    "VAR_OUTPUT"
    "VAR_EXTERNAL"
    "VAR_GLOBAL"
    "VAR_CONFIG"
] @prepend_hardline

[
    "USING" 
    "NAMESPACE"
    "CONFIGURATION"
    "RESOURCE"
    "PROGRAM"
    "FUNCTION"
    "FUNCTION_BLOCK"
    "TYPE"
    "CLASS"
    "INTERFACE"
    "METHOD"
] @prepend_hardline
["TYPE" "STRUCT"] @append_hardline
(namespace_decl . (namespace_h_name) @append_hardline)

[
    "END_RESOURCE"
    "END_CONFIGURATION"
    "END_PROGRAM"
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

    (task_config)
    (prog_config)

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

; Blank line after closing keywords is handled by @allow_blank_line_before
; on the next declaration - no @append_hardline needed here.
("USING" (_) ";"? @append_hardline)

(func_decl body: (func_body) @prepend_hardline)
(fb_decl body: (fb_body) @prepend_hardline)
(method_decl body: (func_body) @prepend_hardline)
(prog_decl body: (fb_body) @prepend_hardline)

[
    "TASK"
    "PROGRAM"
    "RESOURCE"
    (assign)
    (invocation)
    (super_body_invocation)
    (extern_pragma)
    (wasm_pragma)
    (allow_pragma)
    "RETURN"
    (if_stmt)
    (case_stmt)
    (for_stmt)
    (while_stmt)
    (repeat_stmt)
    "EXIT"
    "CONTINUE"
] @prepend_spaced_softline

; func_call as a statement (inside stmt_list) - not inside expressions
(stmt_list (func_call) @prepend_spaced_softline)

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

["ELSE" "ELSIF"] @prepend_hardline

; Binary operators: if any operator in the chain breaks, they all break
["OR" "XOR" "&" "AND" "MOD" "**"] @prepend_spaced_softline
(eq) @prepend_spaced_softline
(ord) @prepend_spaced_softline
(add) @prepend_spaced_softline
(mult) @prepend_spaced_softline

; Continuation indent: binary operator chains indent one level
; The indent starts before the first operator and ends after the right operand
(or_operator "OR" @prepend_indent_start right: (_) @append_indent_end)
(xor_operator "XOR" @prepend_indent_start right: (_) @append_indent_end)
(and_operator ["&" "AND"] @prepend_indent_start right: (_) @append_indent_end)
(add_operator (add) @prepend_indent_start right: (_) @append_indent_end)
(mult_operator (mult) @prepend_indent_start right: (_) @append_indent_end)
(eq_operator (eq) @prepend_indent_start right: (_) @append_indent_end)
(ord_operator (ord) @prepend_indent_start right: (_) @append_indent_end)
(power_operator "**" @prepend_indent_start right: (_) @append_indent_end)
"##;

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
    "CONFIGURATION"
    "RESOURCE"
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
    "VAR_ACCESS"
    "VAR_CONFIG"
    "STRUCT"

    "ELSE"
    "THEN"
] @append_indent_start

; Loops pair their opening keyword with their closing one IN ONE PATTERN, so
; both fire or neither does. Listed separately, a half-typed `FOR i := 0 TO 10`
; with no `DO` yet would close an indentation block it never opened, and the
; whole file would fail to format — exactly when an editor formats on save.
(for_stmt "DO" @append_indent_start "END_FOR" @prepend_indent_end)
(while_stmt "DO" @append_indent_start "END_WHILE" @prepend_indent_end)
(repeat_stmt "REPEAT" @append_indent_start "END_REPEAT" @prepend_indent_end)

(prog_decl name: (identifier) @append_indent_start) ; using "PROGRAM" will break the indentation in CONFIGURATION and RESOURCE

(case_stmt "OF" @append_indent_start)

; case selection
(case_selection
    case_of: (_) @append_indent_start
    case_do: (stmt_list) @append_indent_end
)

[   "END_CONFIGURATION"
    "END_RESOURCE"
    "END_PROGRAM"
    "END_NAMESPACE"
    "END_FUNCTION"
    "END_CLASS"
    "END_FUNCTION_BLOCK"
    "END_TYPE"
    "END_INTERFACE"
    "END_METHOD"
    "END_VAR"
    "END_STRUCT"

    "END_IF"
    "END_CASE"

    "ELSE"
    "ELSIF"
    
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

    (task_config)
    (prog_config)
    (single_resource_decl)

    "RETURN"
    "CONTINUE"
    "CONFIGURATION" "END_CONFIGURATION"
    "RESOURCE" "END_RESOURCE"
    "PROGRAM" "END_PROGRAM"
    "NAMESPACE" "END_NAMESPACE"
    "FUNCTION" "END_FUNCTION"
    "CLASS" "END_CLASS"
    "FUNCTION_BLOCK" "END_FUNCTION_BLOCK"
    "TYPE" "END_TYPE"
    "INTERFACE" "END_INTERFACE"
    "METHOD" "END_METHOD"
    "VAR"
    "STRUCT" "END_STRUCT"
    (line_comment) (c_style_comment) (pascal_style_comment)
    (namespace_elements)
    (test_pragma)
    (allow_pragma)
] @allow_blank_line_before

(stmt_list . (_) @allow_blank_line_before)

; Allow blank lines before END_VAR only when preceded by a declaration
(_ (_) . "END_VAR" @allow_blank_line_before)
"#;

static LEAF: &str = r#"
[
    (line_comment)
    (c_style_comment)
    (pascal_style_comment)
    (s_byte_char_str)
    (d_byte_char_str)
] @leaf
"#;

// should we keep this ? semi colons are just making things worse
#[allow(dead_code)]
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
    {BLOCKS}
    {NEW_LINES}
"#
        ),
    )
    .unwrap(),
    indent: Some("\t".into()),
});

/// Format one source text, refusing anything the grammar could not parse.
///
/// The one gate for every caller. Topiary's own refusal trips on ERROR nodes
/// but not on MISSING ones (a token the parser inserted to recover), and it
/// formatted such a file: one indent level cascaded over every POU after the
/// gap, on a save that check had already rejected with E0019. `has_error`
/// covers both kinds.
pub fn format_source(source: &str) -> anyhow::Result<String> {
    let mut parser = auto_lsp::tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_rk::LANGUAGE.into())?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("could not parse document"))?;
    if tree.root_node().has_error() {
        let (line, column, what) = first_syntax_error(tree.root_node());
        anyhow::bail!("syntax error at line {line}, column {column}: {what}; nothing was written");
    }

    let mut output = vec![];
    formatter(
        &mut source.as_bytes(),
        &mut output,
        &TOPIARY_LANG,
        Operation::Format {
            skip_idempotence: true,
            tolerate_parsing_errors: false,
        },
    )
    .map_err(|e| anyhow::anyhow!("could not format document: {}", e))?;
    Ok(String::from_utf8(output)?)
}

/// The first ERROR or MISSING node, as a 1-based line, column, and what it is.
fn first_syntax_error(node: auto_lsp::tree_sitter::Node<'_>) -> (usize, usize, String) {
    if node.is_missing() {
        let p = node.start_position();
        // An anonymous node's kind is the literal token (`)`, `;`); a named
        // one's is a grammar rule, which is not a word for the user.
        let what = match node.is_named() {
            true => "a token is missing".to_string(),
            false => format!("missing '{}'", node.kind()),
        };
        return (p.row + 1, p.column + 1, what);
    }
    if node.is_error() {
        let p = node.start_position();
        return (p.row + 1, p.column + 1, "unexpected input".to_string());
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.has_error() {
            return first_syntax_error(child);
        }
    }
    let p = node.start_position();
    (p.row + 1, p.column + 1, "unexpected input".to_string())
}

pub fn format(db: &impl WorkspaceDataBase, file: File) -> anyhow::Result<Option<Vec<TextEdit>>> {
    let document = file.document(db);
    let output = format_source(&document.texter.text)?;

    Ok(Some(vec![TextEdit::new(
        auto_lsp::lsp_types::Range {
            start: lsp_types::Position::new(0, 0),
            end: lsp_types::Position::new(document.texter.br_indexes.0.len() as u32, 0),
        },
        output,
    )]))
}
