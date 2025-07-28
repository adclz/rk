use std::sync::LazyLock;

use auto_lsp::{
    anyhow,
    default::db::BaseDatabase,
    lsp_types::{self, DocumentFormattingParams, TextEdit},
};
use topiary_core::{formatter, Language, Operation, TopiaryQuery};

static QUERY: &str = r#"
[
      (line_comment) 
    (c_style_comment)
    (pascal_style_comment)
] @leaf

; Surround spaces
[
    "NAMESPACE" "END_NAMESPACE"
    "FUNCTION" "END_FUNCTION"
    "FUNCTION_BLOCK" "END_FUNCTION_BLOCK"
    "TYPE" "END_TYPE"
    "CLASS" "END_CLASS"
    "INTERFACE" "END_INTERFACE"
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
    ":="
] @prepend_space @append_space

[
  (namespace_decl)
  (func_decl)
  (fb_decl)
  (class_decl)
  (interface_decl)

  ; Variables
  (input_decls)
  (output_decls)
  (in_out_decls)
  (temp_var_decls)
  (external_var_decls)
  (global_var_decls)
  (loc_var_decls)
] @allow_blank_line_before 

; Append spaces
[
  ":"
] @append_space

; Never put a space before a comma
(
  "," @prepend_antispace
)  

; Allow blank line before
[
  (namespace_decl)
] @allow_blank_line_before

; Line breaks
(
  [
    ; Decls
    (data_type_decl)
    (func_decl)
    (fb_decl)
    (class_decl)
    (interface_decl)
    (namespace_decl)
  ] @append_spaced_softline
  .
  [
    (line_comment) 
    (c_style_comment)
    (pascal_style_comment)
  ]* @do_nothing
)

[
    (namespace_elements)
    (func_body)
    (fb_body)
]   
    @prepend_spaced_softline


; Indent
[   "NAMESPACE" 
    "FUNCTION" 
    "CLASS" 
    "FUNCTION_BLOCK" 
    "TYPE" 
    "INTERFACE"
    "VAR" "VAR_INPUT" "VAR_OUTPUT" "VAR_IN_OUT" "VAR_TEMP" "VAR_EXTERNAL" "VAR_GLOBAL"
] @append_indent_start

[   "END_NAMESPACE"
    "END_FUNCTION"
    "END_CLASS"
    "END_FUNCTION_BLOCK" 
    "END_TYPE"
    "END_CLASS" 
    "END_INTERFACE" 
    "END_VAR"
] @prepend_indent_end @prepend_spaced_softline

[
    "VAR" 
    "VAR_INPUT"
    "VAR_OUTPUT" 
    "VAR_IN_OUT" 
    "VAR_TEMP" 
    "VAR_EXTERNAL" 
    "VAR_GLOBAL"
] @append_spaced_softline @prepend_spaced_softline
"#;

static LANG: LazyLock<Language> = LazyLock::new(|| Language {
    name: "IEC".into(),
    grammar: tree_sitter_rk::LANGUAGE.into(),
    query: TopiaryQuery::new(&tree_sitter_rk::LANGUAGE.into(), QUERY).unwrap(),
    indent: Some("\t".into()),
});

pub fn formatting(
    db: &impl BaseDatabase,
    params: DocumentFormattingParams,
) -> anyhow::Result<Option<Vec<TextEdit>>> {
    let uri = &params.text_document.uri;

    let file = db
        .get_file(uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    let document = file.document(db);

    let mut output = vec![];

    formatter(
        &mut document.texter.text.as_bytes(),
        &mut output,
        &LANG,
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

#[cfg(test)]
mod tests {
    use auto_lsp::tree_sitter;

    use super::*;

    #[test]
    fn load_formatting_query() {
        tree_sitter::Query::new(&tree_sitter_rk::LANGUAGE.into(), QUERY)
            .expect("Failed to create formatting query");
    }
}
