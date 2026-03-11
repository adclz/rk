use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::RootDatabase;
use db::WorkspaceDataBase;
use hir::HirNodeInfo;
use hir::hir_ty::body::infer_body;
use ide_diagnostic::{IdeDiagnostic, Related};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::find_pou_with_name;
use crate::tests::utils::render_snapshot;
use crate::tests::utils::test_diagnostics;
use crate::tests::utils::test_snapshot;
use crate::tests::utils::with_db;

/// Formats an adjustment list as a human-readable label.
fn format_adjustments<'db>(
    db: &'db dyn WorkspaceDataBase,
    adjs: &[hir::hir_ty::body::Adjustment<'db>],
) -> String {
    if adjs.is_empty() {
        "<none>".into()
    } else {
        let parts: Vec<String> = adjs
            .iter()
            .map(|a| format!("{:?} -> {}", a.kind, a.target.type_name(db)))
            .collect();
        format!("[{}]", parts.join(", "))
    }
}

/// Collects path expression diagnostics for a POU.
/// Each path expression gets a label showing its resolved type and adjustments.
fn path_expr_diagnostics<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    pou_name: &str,
) -> Vec<IdeDiagnostic> {
    let pou = match find_pou_with_name(db, file, pou_name) {
        Some(p) => p,
        None => return vec![],
    };

    let infer_result = infer_body(db, pou.get_scope_id(db));

    let mut entries: Vec<(Span, String)> = vec![];
    for (path_expr, typ) in &infer_result.type_of_path_expr {
        let span = path_expr.get_span(db);
        let adjs = infer_result.adjustments_of_path_expr(*path_expr);
        let label = format!(
            "{} {}",
            typ.kind(),
            match adjs {
                Some(a) => format_adjustments(db, a),
                None => "<none>".into(),
            }
        );
        entries.push((span, label));
    }

    // Sort by position for deterministic output
    entries.sort_by_key(|(span, _)| span.start_byte);

    if entries.is_empty() {
        return vec![];
    }

    let (first_span, first_label) = &entries[0];
    let mut diag = ide_diagnostic::diag()
        .range(*first_span)
        .message(first_label.clone())
        .severity(DiagnosticSeverity::INFORMATION)
        .call();

    for (span, label) in &entries[1..] {
        diag.with_related(Related::new(label.clone(), file, *span));
    }

    vec![diag]
}

// Both tests below ensure that we correctly walk into path expressions.

#[rstest]
fn walk_array_path_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL
	VAR
		test: ARRAY[0..2] OF INT;
	END_VAR

	test[0] := 0;

END_FUNCTION
"#;

    // in case of index access, the index expression has the same offset as the parent expression
    assert_snapshot!(test_snapshot(&mut with_db, &[source], |db, file| {
        path_expr_diagnostics(db, file, "fn")
    }), @r"
    Advice: VARIABLE [Index -> INT]
       ,-[ file:///test0.st:7:2 ]
       |
     7 |     test[0] := 0;
       |     ^^|^
       |       `--- VARIABLE [Index -> INT]
       |       |
       |       `--- VARIABLE [Index -> INT]
    ---'
    ");
}

#[rstest]
fn walk_struct_with_array_path(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine:
    STRUCT
        power : ARRAY[1..10] OF INT;
    END_STRUCT
END_TYPE

FUNCTION fn: BOOL
	VAR
		test: Engine;
	END_VAR

	test.power[0] := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], |db, file| {
        path_expr_diagnostics(db, file, "fn")
    }), @r"
    Advice: VARIABLE <none>
        ,-[ file:///test0.st:13:2 ]
        |
     13 |     test.power[0] := 0.2;
        |     ^^|^ ^^|^^
        |       `--------- VARIABLE <none>
        |            |
        |            `---- STRUCT_ELEMENT [Index -> INT]
        |            |
        |            `---- STRUCT_ELEMENT [Index -> INT]
    ----'
    ");
}

#[rstest]
fn ref_to_array_index(mut with_db: RootDatabase) {
    let source = r#"
TYPE
	A1: ARRAY[1..99] OF INT;
END_TYPE

FUNCTION fn: BOOL

	VAR
		myA1: A1;
		myRefInt: REF_TO INT := REF(myA1[1]);
	END_VAR

	myRefInt := REF(myA1[11]);

END_FUNCTION
        "#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], |db, file| {
        path_expr_diagnostics(db, file, "fn")
    }), @r"
    Advice: VARIABLE <none>
        ,-[ file:///test0.st:13:2 ]
        |
     13 |     myRefInt := REF(myA1[11]);
        |     ^^^^|^^^        ^^|^
        |         `----------------- VARIABLE <none>
        |                       |
        |                       `--- VARIABLE [Index -> INT]
        |                       |
        |                       `--- VARIABLE [Index -> INT, Ref -> INT]
    ----'
    ");
}

#[rstest]
fn multidim_array_index(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL

	VAR
		myA1: ARRAY[1..10, 1..10] OF INT;
        myInt: INT;
	END_VAR

	myInt := myA1[2][3];

END_FUNCTION
        "#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], |db, file| {
        path_expr_diagnostics(db, file, "fn")
    }), @r"
    Advice: VARIABLE <none>
       ,-[ file:///test0.st:9:2 ]
       |
     9 |     myInt := myA1[2][3];
       |     ^^|^^    ^^|^
       |       `------------ VARIABLE <none>
       |                |
       |                `--- VARIABLE [Index -> ARRAY [1..10, 1..10] OF INT]
       |                |
       |                `--- VARIABLE [Index -> INT]
       |                |
       |                `--- VARIABLE [Index -> ARRAY [1..10, 1..10] OF INT, Index -> INT]
    ---'
    ");
}

#[rstest]
fn ref_to_multidim_array_index(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL

	VAR
		myA1: ARRAY[1..10, 1..10] OF INT;
        myInt: REF_TO INT;
	END_VAR

	myInt := REF(myA1[2][3]);

END_FUNCTION
        "#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], |db, file| {
        path_expr_diagnostics(db, file, "fn")
    }), @r"
    Advice: VARIABLE <none>
       ,-[ file:///test0.st:9:2 ]
       |
     9 |     myInt := REF(myA1[2][3]);
       |     ^^|^^        ^^|^
       |       `---------------- VARIABLE <none>
       |                    |
       |                    `--- VARIABLE [Index -> ARRAY [1..10, 1..10] OF INT]
       |                    |
       |                    `--- VARIABLE [Index -> INT, Ref -> INT]
       |                    |
       |                    `--- VARIABLE [Index -> ARRAY [1..10, 1..10] OF INT, Index -> INT]
    ---'
    ");
}

#[rstest]
fn invalid_type_access_array_index(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL
	VAR
		test: ARRAY[0..2] OF BOOL;
	END_VAR

	test[0] := 0.5;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:7:13 ]
       |
     4 |        test: ARRAY[0..2] OF BOOL;
       |        ^^|^
       |          `--- type is declared by variable 'test' here
       |
     7 |     test[0] := 0.5;
       |                ^|^
       |                 `--- cannot infer '<float>' to 'BOOL': invalid boolean literal
    ---'
    ");
    let file = *with_db.get_files().iter().last().unwrap();
    assert_snapshot!(render_snapshot(&with_db, file, path_expr_diagnostics(&with_db, file, "fn")), @r"
    Advice: VARIABLE [Index -> BOOL]
       ,-[ file:///test0.st:7:2 ]
       |
     7 |     test[0] := 0.5;
       |     ^^|^
       |       `--- VARIABLE [Index -> BOOL]
       |       |
       |       `--- VARIABLE [Index -> BOOL]
    ---'
    ");
}

#[rstest]
fn invalid_struct_field_access(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine:
    STRUCT
        power : INT;
        oil : REAL;
    END_STRUCT
END_TYPE

FUNCTION fn: BOOL
	VAR
		test: Engine;
	END_VAR

	test.powerr := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
        ,-[ file:///test0.st:14:7 ]
        |
      3 | ,->     STRUCT
        : :
      6 | |->     END_STRUCT
        | |
        | `-------------------- type is defined by 'Engine' here
        |
     14 |         test.powerr := 0.2;
        |              ^^^|^^
        |                 `---- 'Engine' has no field named 'powerr'
    ----'
    ");
}

#[rstest]
fn invalid_struct_field_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine:
    STRUCT
        power : INT;
        oil : REAL;
    END_STRUCT
END_TYPE

FUNCTION fn: BOOL
	VAR
		test: Engine;
	END_VAR

	test.power := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
        ,-[ file:///test0.st:14:16 ]
        |
      4 |         power : INT;
        |         ^^|^^
        |           `---- type is defined by struct field 'power' here
        |
     14 |     test.power := 0.2;
        |                   ^|^
        |                    `--- cannot infer '<float>' to 'INT': invalid INT literal
    ----'
    ");
}

#[rstest]
fn index_expression_on_struct(mut with_db: RootDatabase) {
    let source = r#"
TYPE Engine:
    STRUCT
        power : INT;
        oil : REAL;
    END_STRUCT
END_TYPE

FUNCTION fn: BOOL
	VAR
		test: Engine;
	END_VAR

	test[0] := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0213] Error: invalid operation
        ,-[ file:///test0.st:14:2 ]
        |
     14 |     test[0] := 0.2;
        |     ^^|^
        |       `--- cannot index into type 'Engine'
    ----'
    ");
}

#[rstest]
fn field_expression_on_array(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL
	VAR
		test: ARRAY[0..1] OF INT;
	END_VAR

	test.not_a_field := 0.2;

END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0211] Error: no such field
       ,-[ file:///test0.st:7:7 ]
       |
     4 |        test: ARRAY[0..1] OF INT;
       |        ^^|^
       |          `--- type is declared by variable 'test' here
       |
     7 |     test.not_a_field := 0.2;
       |          ^^^^^|^^^^^
       |               `------- 'ARRAY [0..1] OF INT' has no field named 'not_a_field'
    ---'
    ");
}

/// When a variable's type is unresolved (Never), field access on it
/// should NOT produce a cascading "has no field" error.
#[rstest]
fn no_cascading_error_on_never_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn0
    VAR
        test: unknown;
    END_VAR
        test.wrong := 123;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:4:15 ]
       |
     4 |         test: unknown;
       |               ^^^|^^^
       |                  `----- no item found for path 'unknown'
    ---'
    ");
}
