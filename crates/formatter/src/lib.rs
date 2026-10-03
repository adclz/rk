// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    (public) (protected) (private) (internal)
    ; `REF_TO` is the `(ref_to)` alias in a VAR, and the bare keyword in a
    ; VAR_TEMP's `ref_spec`. Without the second, `REF_TO INT` there was
    ; written `REF_TOINT`, a type that does not exist.
    (ref_to) "REF_TO"
    "IMPLEMENTS" "EXTENDS"
    "METHOD" "END_METHOD"
    "IF" "THEN" "ELSE" "ELSIF"
    "CASE" "OF" "END_CASE"
    "FOR" "TO" "BY" "DO"
    "WHILE" "DO"
    "REPEAT" "UNTIL"
    "TASK" "CONSTANT" "RETAIN" "NON_RETAIN" "WITH" "ON"
    ; Located variables (`x AT %QW28 : INT`), edge qualifiers
    ; (`BOOL R_EDGE`) and VAR_ACCESS directions (`INT READ_ONLY`) all follow
    ; another token. Without a space they GLUE onto it and the file stops
    ; parsing — `BOOLR_EDGE` is not a type.
    "AT" "R_EDGE" "F_EDGE" (ERR_invalid_edge_qualifier)
    (read_only) (read_write)
   
    ":=" "=" "=>" "<=" "<" ">=" ">" "<>" "+" "-" "*" "/" 
    "&" "AND" "OR" "XOR" "MOD" "NOT"
    (c_style_comment)
    (pascal_style_comment)
] @prepend_space @append_space

; A line comment ends its line: a space after it went before the line
; break and was written as an indented blank line.
(line_comment) @prepend_space

; A whole statement on its own: nothing follows it but the terminator, and a
; trailing space there would be written before the `;` this file appends and
; removed once that `;` is a token — the same source formatting two ways.
["RETURN" "EXIT" "CONTINUE"] @prepend_space

[(identifier) "%"] @prepend_space
["(" "[" "." "END_CASE"] @append_antispace
[")" "]" ":" "," "." (deref_sign)] @prepend_antispace
; A `;` sits against what it ends: a statement, a declaration, a comment
; between the two, an `END_VAR` or a `STRUCT`. The one that opens a CASE
; branch, `1: ;`, follows the label's space instead.
(_ (_) . ";" @prepend_antispace)
(_ ["END_VAR" "STRUCT"] . ";" @prepend_antispace)
["NOT" ":"] @append_space

; signed_int and signed_real_value: sign is part of the token, no formatting needed

; A duration's sign is part of the literal too, but the grammar makes it a
; separate anonymous "+"/"-" — the same tokens as binary plus/minus above.
; Spacing it turns `T#-14ms` into `T# - 14ms`, which no longer type-checks.
(time sign: ["+" "-"] @prepend_antispace @append_antispace)
(ltime sign: ["+" "-"] @prepend_antispace @append_antispace)

; Enum value: no space around # (Color#Red, not Color # Red)
(enum_value "#" @prepend_antispace @append_antispace)

; The `*` of a partial address, `%Q*`, is the same token as the
; multiplication above: spaced like one, it was written `%Q *`.
(direct_variable (partly) @prepend_antispace @append_antispace)
(relative_direct_variable (partly) @prepend_antispace @append_antispace)
(loc_partly_var "*" @prepend_antispace @append_antispace)

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

; Test, export and once pragmas: each on its own line before the POU
; keyword. Two may stand above one FUNCTION, and without this they glue
; onto each other.
(test_pragma) @leaf @append_hardline
(export_pragma) @leaf @append_hardline
(once_pragma) @leaf @append_hardline
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

; Every other declaration opens its own line by rule; a type declaration
; relied on the `;` after it, so two of them sat on one line until the pass
; that appended the missing one.
(type_decl) @prepend_hardline
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
    (config_inst_init)
    (access_decl)
    (prog_access_decl)
] @prepend_hardline

; Blank line after closing keywords is handled by @allow_blank_line_before
; on the next declaration - no @append_hardline needed here. Nothing ends
; a USING's line either: whatever follows one, a declaration, a section, a
; body or another USING, opens its own. A line break appended to the `;`
; moved a comment after it, `USING a; // c`, down a line on the second
; pass, since the comment is the directive's sibling, not the `;`'s.

(func_decl body: (func_body) @prepend_hardline)
(fb_decl body: (fb_body) @prepend_hardline)
(method_decl body: (func_body) @prepend_hardline)
(prog_decl body: (fb_body) @prepend_hardline)

[
    "TASK"
    "PROGRAM"
    "RESOURCE"
    (extern_pragma)
] @prepend_spaced_softline

; A statement starts its own line. The `;` decides nothing here: it used to
; carry the line break, so a statement with one and a statement without
; were laid out two ways, and a file settled only on the second pass, once
; the `;` this file writes in had become a token. The first statement of a
; CASE branch is the exception, below; the first of any other body is
; listed by its parent, since a branch's body and a block's are the same
; `stmt_list` node.
(stmt_list
  (_)
  .
  [
    (assign)
    (func_call)
    (begin_path_expression)
    (if_stmt)
    (case_stmt)
    (for_stmt)
    (while_stmt)
    (repeat_stmt)
    (raise_stmt)
    (wasm_pragma)
    (allow_pragma)
    "RETURN"
    "EXIT"
    "CONTINUE"
  ] @prepend_hardline
)
(
  [
    (func_body (stmt_list . (_) @prepend_hardline))
    (fb_body (stmt_list . (_) @prepend_hardline))
    (if_stmt if_body: (stmt_list . (_) @prepend_hardline))
    (if_stmt else_body: (stmt_list . (_) @prepend_hardline))
    (else_if_stmt else_if_body: (stmt_list . (_) @prepend_hardline))
    (case_stmt default: (stmt_list . (_) @prepend_hardline))
    (for_stmt body: (stmt_list . (_) @prepend_hardline))
    (while_stmt while_body: (stmt_list . (_) @prepend_hardline))
    (repeat_stmt repeat_body: (stmt_list . (_) @prepend_hardline))
  ]
)

; The first statement of a CASE branch: a block starts its own line under
; the label, a plain statement stays on the label's line or keeps the line
; the source gave it. The source's line break is the body's, not its first
; statement's: Topiary records a break between a node and the node before
; it in walking order, and the body starts where its first statement does.
(case_selection case_do: (stmt_list) @prepend_input_softline)
(case_selection case_do: (stmt_list . [(if_stmt) (case_stmt) (for_stmt) (while_stmt) (repeat_stmt)] @prepend_hardline))

; An empty statement, a `;` of its own, keeps the line it had.
(stmt_list (_) . ";" @prepend_input_softline)

 (
  "," @append_spaced_softline
  .
  [(line_comment) (c_style_comment) (pascal_style_comment)]* @do_nothing
)

[(line_comment) (c_style_comment) (pascal_style_comment)] @prepend_input_softline

(
  [(line_comment) (c_style_comment) (pascal_style_comment)] @append_input_softline
  .
  [ "," ";" ]* @do_nothing
)

[
    (line_comment)
    "THEN"
    "ELSE"
    "DO"
    "REPEAT"
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
; The body of a REPEAT ends at UNTIL, which stands at the REPEAT's level
; with the condition, and END_REPEAT under it.
(repeat_stmt "REPEAT" @append_indent_start "UNTIL" @prepend_indent_end)

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
    (export_pragma)
    (once_pragma)
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

/// The terminators the grammar lets a source leave out, written in once.
/// Each pattern names the node a `;` follows and gives up when one is
/// already there.
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

    (type_decl)
    (task_config)
    (prog_config)
    (access_decl)
    (prog_access_decl)
    (config_inst_init)

    (assign)
    "RETURN"
    "EXIT"
    "CONTINUE"
    (if_stmt)
    (for_stmt)
    (case_stmt)
    (while_stmt)
    (repeat_stmt)
    (raise_stmt)
  ] @append_delimiter
  .
  ";"* @do_nothing
  (#delimiter! ";")
)

; A call, `SUPER()` or `THIS.m()` is a statement only as a child of a
; statement list; in an expression it takes no `;`. The guard has to see
; the `;` as the statement's sibling: `SUPER()` matched as the invocation
; inside its `begin_path_expression` saw none there and was given a second
; one, `SUPER();;`, on every pass.
(stmt_list
  [(func_call) (begin_path_expression)] @append_delimiter
  .
  ";"* @do_nothing
  (#delimiter! ";")
)

; A USING directive holds its own `;`, so the sibling guard above cannot see
; it and the guard has to look inside. Matching an arbitrary child instead
; produced one match per name, and every name but the last had no `;` after
; it to suppress on, which wrote `USING a, b;;`. An anchor is no help here:
; anchors skip anonymous nodes, so `;` is invisible to one.
(
  (using_directive
    ";"* @do_nothing
  ) @append_delimiter
  (#delimiter! ";")
)

(
    (struct_elem_decl) @append_delimiter
    .
    ";"* @do_nothing
    (#delimiter! ";")
)

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
    {SEMI_COLONS}
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
/// gap, on a save that check had already rejected with E0002. `has_error`
/// covers both kinds. A recovery rule of the grammar, an `ERR_*` node, is
/// refused too: it is a syntax error `check` reports, and what the
/// formatter wrote for one (`x :=;`) was anyone's guess.
pub fn format_source(source: &str) -> anyhow::Result<String> {
    let tree = parse(source)?;
    if let Some((line, column, what)) = syntax_error(&tree) {
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

fn parse(source: &str) -> anyhow::Result<auto_lsp::tree_sitter::Tree> {
    let mut parser = auto_lsp::tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_rk::LANGUAGE.into())?;
    parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("could not parse document"))
}

/// Whether the formatter would take `source`: it parses with no ERROR,
/// MISSING or `ERR_*` node. The fuzz oracle asks before holding a refusal
/// against it.
pub fn accepts(source: &str) -> bool {
    parse(source).is_ok_and(|tree| syntax_error(&tree).is_none())
}

/// The first syntax error in `tree`, as a 1-based line, column and what it
/// is: an ERROR or MISSING node, or else a node of a recovery rule.
fn syntax_error(tree: &auto_lsp::tree_sitter::Tree) -> Option<(usize, usize, String)> {
    let root = tree.root_node();
    if root.has_error() {
        return Some(first_syntax_error(root));
    }
    let node = first_recovered_error(root)?;
    let p = node.start_position();
    Some((p.row + 1, p.column + 1, "unexpected input".to_string()))
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

/// The first node a recovery rule of the grammar produced, in source order.
fn first_recovered_error(
    node: auto_lsp::tree_sitter::Node<'_>,
) -> Option<auto_lsp::tree_sitter::Node<'_>> {
    if node.kind().starts_with("ERR_") {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = first_recovered_error(child) {
            return Some(found);
        }
    }
    None
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
