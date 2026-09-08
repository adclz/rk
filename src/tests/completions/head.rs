use ast::generated::DataTypeDecl;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::tracked::get_ast;
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::handlers::completions_utils::{CompletionCtx, QueryMode, static_snippets};
use ide_proto::handlers::{CompletionHandler, CompletionRequest};
use ide_proto::walk::completion_descendant_at;
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

/// Test completion WITHIN a variable type declaration (head scope)
#[rstest]
pub fn head_scope_items(mut with_db: RootDatabase) {
    let source = r#"
TYPE Test: INT; END_TYPE

FUNCTION test_fn
    VAR
        my_var: INT;
        another_var: T
    END_VAR

    my_var := 10;
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "test_fn",
    )
    .unwrap();

    let offset = source.find("another_var").unwrap() + 1;
    let mut ctx = CompletionCtx::new(offset, QueryMode::Head);
    ctx.scope_completion(pou.get_scope_id(&with_db), "", &with_db);
    let completions = ctx.take_items();

    // only Test should be suggested
    assert_eq!(completions.len(), 1);
    assert!(format!("{completions:?}").contains("Test"));
}

/// TYPE declarations should NOT offer POU-level or body-level completions.
/// Only type-related completions (elementary types, STRUCT, ARRAY) should appear
/// when cursor is inside a TYPE block.
#[rstest]
pub fn type_decl_no_pou_completions(mut with_db: RootDatabase) {
    let source = "TYPE MyStruct :\nSTRUCT\n    x : INT;\nEND_STRUCT;\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor inside the TYPE body (before STRUCT keyword)
    let offset = source.find("STRUCT").unwrap() - 1;
    let result = completion_descendant_at(&with_db, file, offset);

    if let Some((node, node_key, is_last_before)) = result {
        let req = CompletionRequest {
            offset,
            trigger_character: None,
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

        // Should NOT have POU-level snippets
        assert!(
            !labels.contains(&"FUNCTION"),
            "TYPE should not offer FUNCTION: {labels:?}"
        );
        assert!(
            !labels.contains(&"FUNCTION_BLOCK"),
            "TYPE should not offer FUNCTION_BLOCK: {labels:?}"
        );
        assert!(
            !labels.contains(&"CLASS"),
            "TYPE should not offer CLASS: {labels:?}"
        );
        assert!(
            !labels.contains(&"PROGRAM"),
            "TYPE should not offer PROGRAM: {labels:?}"
        );
        assert!(
            !labels.contains(&"INTERFACE"),
            "TYPE should not offer INTERFACE: {labels:?}"
        );

        // Should NOT have statement-level snippets
        assert!(
            !labels.contains(&"IF"),
            "TYPE should not offer IF: {labels:?}"
        );
        assert!(
            !labels.contains(&"FOR"),
            "TYPE should not offer FOR: {labels:?}"
        );
        assert!(
            !labels.contains(&"WHILE"),
            "TYPE should not offer WHILE: {labels:?}"
        );
    }
}

/// Empty TYPE body should only offer type-relevant completions
#[rstest]
pub fn type_decl_empty_body_no_pou_completions(mut with_db: RootDatabase) {
    let source = "TYPE MyType :\n    t\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor on the `t` character — simulates user typing a type name
    let offset = source.find("    t").unwrap() + 5;
    let result = completion_descendant_at(&with_db, file, offset);

    if let Some((node, node_key, is_last_before)) = result {
        let req = CompletionRequest {
            offset,
            trigger_character: None,
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

        // Should NOT have POU-level snippets
        assert!(
            !labels.contains(&"FUNCTION"),
            "TYPE should not offer FUNCTION: {labels:?}"
        );
        assert!(
            !labels.contains(&"FUNCTION_BLOCK"),
            "TYPE should not offer FUNCTION_BLOCK: {labels:?}"
        );
        assert!(
            !labels.contains(&"IF"),
            "TYPE should not offer IF: {labels:?}"
        );
        assert!(
            !labels.contains(&"FOR"),
            "TYPE should not offer FOR: {labels:?}"
        );
    } else {
        // If no node found, completion_descendant_at returned None which means
        // top-level completions will show — this is the bug scenario
        panic!("Expected a node at offset {offset}, got None — top-level completions would show");
    }
}

/// When cursor is on empty line inside a TYPE with valid content,
/// the completion system should NOT return None (which would show POU snippets)
#[rstest]
pub fn type_decl_completion_descendant_not_none(mut with_db: RootDatabase) {
    let source = "TYPE MyStruct :\nSTRUCT\n    x : INT;\nEND_STRUCT;\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Check various positions inside the TYPE body
    let positions = [
        source.find("STRUCT").unwrap(),  // on STRUCT keyword
        source.find("x : INT").unwrap(), // on field name
        source.find("INT;").unwrap(),    // on INT type
    ];

    for offset in positions {
        let result = completion_descendant_at(&with_db, file, offset);
        assert!(
            result.is_some(),
            "Expected a node at offset {offset} inside TYPE body, got None"
        );
    }
}

/// When cursor is inside a bare TYPE with just whitespace, the server would
/// fall back to top-level completions because no HIR node covers the offset.
/// This verifies the scenario.
#[rstest]
pub fn type_decl_bare_shows_no_pou_snippets(mut with_db: RootDatabase) {
    // A TYPE with just an identifier — `t` is parsed as the spec target
    let source = "TYPE MyType :\n    t\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor right on the `t` identifier
    let offset = source.find("\n    t").unwrap() + 5;
    let result = completion_descendant_at(&with_db, file, offset);

    // This should find the Spec node covering `t`
    assert!(
        result.is_some(),
        "Expected Spec node at offset {offset} in TYPE body"
    );

    let (node, node_key, is_last_before) = result.unwrap();
    let req = CompletionRequest {
        offset,
        trigger_character: None,
        query: "".into(),
        node_index_pos: Some(node_key),
        is_last_before,
    };
    let completions = node.completion(&with_db, &req).unwrap_or_default();
    let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

    // Should offer type completions (elementary types, STRUCT, ARRAY)
    assert!(labels.contains(&"INT"), "should offer INT: {labels:?}");
    assert!(
        labels.contains(&"STRUCT"),
        "should offer STRUCT: {labels:?}"
    );
    assert!(labels.contains(&"ARRAY"), "should offer ARRAY: {labels:?}");

    // Should NOT offer POU/body completions
    assert!(
        !labels.contains(&"FUNCTION"),
        "TYPE should not offer FUNCTION: {labels:?}"
    );
    assert!(
        !labels.contains(&"IF"),
        "TYPE should not offer IF: {labels:?}"
    );
}

/// Incomplete TYPE where user hasn't typed the spec yet.
/// When completion_descendant_at returns None inside a TYPE body,
/// the server should detect the data_type_decl CST context and
/// offer type-level completions (not POU-level snippets).
#[rstest]
pub fn type_decl_incomplete_no_pou_completions(mut with_db: RootDatabase) {
    // TYPE with colon but no spec yet — user is about to type
    let source = "TYPE MyType :\n\nEND_TYPE\n";

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor on the empty line between `:` and END_TYPE
    let offset = source.find("\n\nEND_TYPE").unwrap() + 1;
    let result = completion_descendant_at(&with_db, file, offset);

    // completion_descendant_at returns None because no HIR node covers
    // the cursor in an incomplete TYPE. The server handles this by checking
    // the CST for data_type_decl context.
    assert!(
        result.is_none(),
        "Expected None for incomplete TYPE body at offset {offset}"
    );

    // Verify the AST-level check: cursor is inside a DataTypeDecl
    let ast = get_ast(&with_db, file);
    let in_type_decl = ast.iter().any(|node| {
        let range = node.get_range();
        range.start_byte <= offset
            && offset <= range.end_byte
            && node.lower().downcast_ref::<DataTypeDecl>().is_some()
    });

    assert!(
        in_type_decl,
        "cursor at offset {offset} should be inside a DataTypeDecl AST node"
    );
}

/// TYPE with enum should not offer POU/body completions
#[rstest]
pub fn type_enum_no_pou_completions(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyEnum : (
    Val1,
    Val2
);
END_TYPE
"#;

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Cursor between the enum values
    let offset = source.find("Val1").unwrap();
    let result = completion_descendant_at(&with_db, file, offset);

    if let Some((node, node_key, is_last_before)) = result {
        let req = CompletionRequest {
            offset,
            trigger_character: None,
            query: "".into(),
            node_index_pos: Some(node_key),
            is_last_before,
        };
        let completions = node.completion(&with_db, &req).unwrap_or_default();
        let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();

        assert!(
            !labels.contains(&"FUNCTION"),
            "TYPE enum should not offer FUNCTION: {labels:?}"
        );
        assert!(
            !labels.contains(&"IF"),
            "TYPE enum should not offer IF: {labels:?}"
        );
    }
}

#[test]
fn struct_and_array_snippets_exist() {
    let struct_item = static_snippets::struct_();
    assert_eq!(struct_item.label, "STRUCT");
    assert!(struct_item.insert_text.unwrap().contains("END_STRUCT"));

    let array_item = static_snippets::array();
    assert_eq!(array_item.label, "ARRAY");
    assert!(array_item.insert_text.unwrap().contains("OF"));
}

/// The cursor sits in a VAR section, where the declaration being typed has no
/// node of its own yet. Every POU kind must still offer what a section takes.
#[rstest]
#[case::function(
    r#"
FUNCTION fn : INT
VAR
    |
END_VAR
END_FUNCTION
"#
)]
#[case::function_block(
    r#"
FUNCTION_BLOCK fb
VAR
    |
END_VAR
END_FUNCTION_BLOCK
"#
)]
#[case::program(
    r#"
PROGRAM prog
VAR
    |
END_VAR
END_PROGRAM
"#
)]
#[case::method(
    r#"
FUNCTION_BLOCK fb
METHOD m
VAR
    |
END_VAR
END_METHOD
END_FUNCTION_BLOCK
"#
)]
#[case::after_the_colon(
    r#"
FUNCTION_BLOCK fb
VAR
    x : |
END_VAR
END_FUNCTION_BLOCK
"#
)]
#[case::after_the_name(
    r#"
FUNCTION_BLOCK fb
VAR
    x |
END_VAR
END_FUNCTION_BLOCK
"#
)]
pub fn var_section_items(mut with_db: RootDatabase, #[case] marked: &str) {
    let labels = complete_at(&mut with_db, marked);

    for expected in ["INT", "STRING", "STRUCT", "ARRAY", "AT"] {
        assert!(
            labels.iter().any(|l| l == expected),
            "no {expected} in {labels:?}"
        );
    }
    for unwanted in ["USING", "IF", "FOR"] {
        assert!(
            !labels.iter().any(|l| l == unwanted),
            "{unwanted} inside a VAR section"
        );
    }
}

/// An access specifier is written between the POU keyword and the name, where
/// a half-typed one parses as the name itself.
#[rstest]
#[case::function(
    r#"
FUNCTION PRI|
END_FUNCTION
"#,
    &["PUBLIC", "PROTECTED", "PRIVATE", "INTERNAL"]
)]
#[case::method(
    r#"
FUNCTION_BLOCK fb
METHOD PUB|
END_METHOD
END_FUNCTION_BLOCK
"#,
    &["PUBLIC", "PROTECTED", "PRIVATE", "INTERNAL"]
)]
#[case::namespace(
    r#"
NAMESPACE INT|
END_NAMESPACE
"#,
    &["INTERNAL"]
)]
pub fn header_visibility_items(
    mut with_db: RootDatabase,
    #[case] marked: &str,
    #[case] expected: &[&str],
) {
    assert_eq!(complete_at(&mut with_db, marked), expected);
}

/// A specifier already written leaves the position to the name.
#[rstest]
pub fn header_visibility_written_once(mut with_db: RootDatabase) {
    let marked = r#"
FUNCTION_BLOCK fb
METHOD PRIVATE m|
END_METHOD
END_FUNCTION_BLOCK
"#;
    assert!(complete_at(&mut with_db, marked).is_empty());
}

/// Completes at the `|` marker, which is stripped from the source.
fn complete_at(db: &mut RootDatabase, marked: &str) -> Vec<String> {
    let offset = marked.find('|').expect("a cursor marker");
    let source = marked.replace('|', "");
    add_sources(db, &[&source]);
    let file = *db.get_files().iter().last().unwrap();
    ide_proto::handlers::completions::complete(db, file, offset, None)
        .into_iter()
        .map(|item| item.label)
        .collect()
}

/// `AT` maps a variable to an address, which only a plain VAR section and a
/// VAR_GLOBAL take. The interface sections, VAR_TEMP and VAR_EXTERNAL refuse
/// one, and every section still offers the types.
#[rstest]
#[case::var(
    r#"
FUNCTION_BLOCK fb
VAR
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    true
)]
#[case::var_retain(
    r#"
FUNCTION_BLOCK fb
VAR RETAIN
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    true
)]
#[case::var_non_retain(
    r#"
FUNCTION_BLOCK fb
VAR NON_RETAIN
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    true
)]
#[case::var_constant(
    r#"
FUNCTION_BLOCK fb
VAR CONSTANT
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    true
)]
#[case::program_var(
    r#"
PROGRAM prog
VAR
    |
END_VAR
END_PROGRAM
"#,
    true
)]
#[case::var_global(
    r#"
CONFIGURATION conf
VAR_GLOBAL
    |
END_VAR
END_CONFIGURATION
"#,
    true
)]
#[case::var_input(
    r#"
FUNCTION_BLOCK fb
VAR_INPUT
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    false
)]
#[case::var_output(
    r#"
FUNCTION_BLOCK fb
VAR_OUTPUT
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    false
)]
#[case::var_in_out(
    r#"
FUNCTION_BLOCK fb
VAR_IN_OUT
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    false
)]
#[case::var_temp(
    r#"
FUNCTION_BLOCK fb
VAR_TEMP
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    false
)]
#[case::var_external(
    r#"
FUNCTION_BLOCK fb
VAR_EXTERNAL
    |
END_VAR
END_FUNCTION_BLOCK
"#,
    false
)]
#[case::method_var_input(
    r#"
FUNCTION_BLOCK fb
METHOD m
VAR_INPUT
    |
END_VAR
END_METHOD
END_FUNCTION_BLOCK
"#,
    false
)]
pub fn at_only_where_a_location_is_written(
    mut with_db: RootDatabase,
    #[case] marked: &str,
    #[case] takes_a_location: bool,
) {
    let labels = complete_at(&mut with_db, marked);

    assert!(labels.iter().any(|l| l == "INT"), "no types in {labels:?}");
    assert_eq!(
        labels.iter().any(|l| l == "AT"),
        takes_a_location,
        "AT in {labels:?}"
    );
}

/// The byte just past `END_VAR` closes the section: the cursor there belongs
/// to whatever follows, not to the declarations.
#[rstest]
pub fn end_var_closes_the_section(mut with_db: RootDatabase) {
    let marked = r#"
FUNCTION_BLOCK fb
VAR
    x : INT;
END_VAR|
END_FUNCTION_BLOCK
"#;
    let labels = complete_at(&mut with_db, marked);

    assert!(!labels.iter().any(|l| l == "AT"), "AT past END_VAR");
    assert!(
        labels.iter().any(|l| l == "VAR_INPUT"),
        "no section keywords in {labels:?}"
    );
}

/// A `{` opens a pragma, and each position takes only the ones legal there.
/// The cursor sits between braces because the editor auto-closes the first.
#[rstest]
#[case::file_level(
    r#"
{|}
FUNCTION fn : INT
END_FUNCTION
"#,
    &["{test}", "{extern}", "{once}", "{warn}", "{info}", "{allow}"]
)]
#[case::in_a_namespace(
    r#"
NAMESPACE ns
{|}
FUNCTION fn : INT
END_FUNCTION
END_NAMESPACE
"#,
    &["{test}", "{extern}", "{once}", "{warn}", "{info}", "{allow}"]
)]
#[case::in_a_pou_head(
    r#"
FUNCTION_BLOCK fb
{|}
METHOD m
END_METHOD
END_FUNCTION_BLOCK
"#,
    &[]
)]
#[case::in_an_interface(
    r#"
INTERFACE i
{|}
END_INTERFACE
"#,
    &[]
)]
#[case::in_a_body(
    r#"
FUNCTION_BLOCK fb
VAR
    x : INT;
END_VAR
    {|}
END_FUNCTION_BLOCK
"#,
    &["{wasm}", "{allow}"]
)]
#[case::in_a_method_body(
    r#"
FUNCTION_BLOCK fb
METHOD m
VAR
    x : INT;
END_VAR
    {|}
END_METHOD
END_FUNCTION_BLOCK
"#,
    &["{wasm}", "{allow}"]
)]
#[case::in_a_function_head(
    r#"
FUNCTION fn : INT
{|}
VAR
    x : INT;
END_VAR
END_FUNCTION
"#,
    &[]
)]
#[case::in_a_var_section(
    r#"
FUNCTION_BLOCK fb
VAR
    {|}
END_VAR
END_FUNCTION_BLOCK
"#,
    &[]
)]
pub fn pragma_items(mut with_db: RootDatabase, #[case] marked: &str, #[case] expected: &[&str]) {
    assert_eq!(complete_at(&mut with_db, marked), expected);
}

/// Without a `{` a pragma is not what is being written, so none is offered.
#[rstest]
pub fn no_pragmas_without_a_brace(mut with_db: RootDatabase) {
    let marked = r#"
FUNCTION_BLOCK fb
VAR
    x : INT;
END_VAR
    |
END_FUNCTION_BLOCK
"#;
    let labels = complete_at(&mut with_db, marked);

    assert!(!labels.is_empty());
    assert!(!labels.iter().any(|l| l.starts_with('{')), "{labels:?}");
}

/// The editor auto-closes `{`, so the closing brace is written only when the
/// source has none: completing it twice would leave `{test}}`.
#[rstest]
#[case::auto_closed("{|}\nFUNCTION fn : INT\nEND_FUNCTION\n", "test")]
#[case::unclosed("{|\nFUNCTION fn : INT\nEND_FUNCTION\n", "test}")]
pub fn a_pragma_closes_itself_only_once(
    mut with_db: RootDatabase,
    #[case] marked: &str,
    #[case] expected: &str,
) {
    let offset = marked.find('|').expect("a cursor marker");
    let source = marked.replace('|', "");
    add_sources(&mut with_db, &[&source]);
    let file = *with_db.get_files().iter().last().unwrap();
    let items = ide_proto::handlers::completions::complete(&with_db, file, offset, None);
    let test = items.iter().find(|i| i.label == "{test}").expect("{test}");

    assert_eq!(test.insert_text.as_deref(), Some(expected));
    assert_eq!(test.filter_text.as_deref(), Some("test"));
}

/// An INTERFACE holds method prototypes, so its statement snippets were
/// offering IF and FOR where neither can go.
#[rstest]
pub fn an_interface_takes_no_statements(mut with_db: RootDatabase) {
    let marked = r#"
INTERFACE i
|
END_INTERFACE
"#;
    let labels = complete_at(&mut with_db, marked);

    for stmt in ["IF", "FOR", "WHILE", "REPEAT"] {
        assert!(!labels.iter().any(|l| l == stmt), "{stmt} in an INTERFACE");
    }
}

/// `{allow}` names a lint rule, and the names come from the linter itself so
/// the list cannot fall behind the rules. The argument is a plain string, so
/// only its position inside an `{allow}` tells it apart from any other.
#[rstest]
#[case::between_the_quotes(
    r#"
{allow '|'}
FUNCTION f : INT
END_FUNCTION
"#,
    true
)]
#[case::part_way_through_a_name(
    r#"
{allow 'unu|'}
FUNCTION f : INT
END_FUNCTION
"#,
    true
)]
#[case::a_second_rule_in_one_pragma(
    r#"
{allow 'dead-code' '|'}
FUNCTION f : INT
END_FUNCTION
"#,
    true
)]
#[case::outside_the_quotes(
    r#"
{allow 'dead-code'}
FUNCTION f : INT
    |
END_FUNCTION
"#,
    false
)]
#[case::another_pragma_taking_a_string(
    r#"
FUNCTION f : INT
    {wasm '|'}
END_FUNCTION
"#,
    false
)]
#[case::a_string_that_is_not_a_pragma(
    r#"
FUNCTION f : INT
VAR
    s : STRING := '|';
END_VAR
END_FUNCTION
"#,
    false
)]
pub fn allow_names_a_lint_rule(
    mut with_db: RootDatabase,
    #[case] marked: &str,
    #[case] names_rules: bool,
) {
    let labels = complete_at(&mut with_db, marked);

    assert_eq!(
        labels == linter::rules::ALL_RULE_NAMES,
        names_rules,
        "{labels:?}"
    );
}
