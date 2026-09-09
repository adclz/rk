use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{
    InlineValue, InlineValueContext, InlineValueParams, Position, Range, TextDocumentIdentifier,
    WorkDoneProgressParams,
};
use db::RootDatabase;
use ide_proto::handlers::inline_value::inline_values;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

/// The lookups the server offers for the whole file, stopped at `stopped_line`.
fn shown(db: &mut RootDatabase, source: &str, stopped_line: u32) -> String {
    add_sources(db, &[source]);
    let file = *db.get_files().iter().last().unwrap();
    let whole = Range::new(Position::new(0, 0), Position::new(9999, 0));
    let params = InlineValueParams {
        work_done_progress_params: WorkDoneProgressParams::default(),
        text_document: TextDocumentIdentifier {
            uri: file.url(db).clone(),
        },
        range: whole,
        context: InlineValueContext {
            frame_id: 0,
            stopped_location: Range::new(
                Position::new(stopped_line, 0),
                Position::new(stopped_line, 0),
            ),
        },
    };

    let out: Vec<String> = inline_values(db, file, &params)
        .iter()
        .map(|value| match value {
            InlineValue::VariableLookup(l) => format!(
                "line {} {} (case-sensitive: {})",
                l.range.start.line,
                l.variable_name.clone().unwrap_or_default(),
                l.case_sensitive_lookup
            ),
            other => format!("{other:?}"),
        })
        .collect();
    match out.is_empty() {
        true => "<none>".to_string(),
        false => out.join("\n"),
    }
}

const SOURCE: &str = r#"
FUNCTION_BLOCK Gear
VAR_INPUT ratio : INT; END_VAR
VAR_OUTPUT out : INT; END_VAR
    out := ratio;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    n : INT;
    total : INT;
    g : Gear;
END_VAR
    n := 1;
    g(ratio := n);
    total := total + n;
END_PROGRAM"#;

/// Declarations and bare uses, up to where execution stopped: a value shown
/// below the stopped line is last scan's.
#[rstest]
fn lookups_stop_where_execution_did(mut with_db: RootDatabase) {
    assert_snapshot!(shown(&mut with_db, SOURCE, 14), @r"
    line 2 ratio (case-sensitive: false)
    line 3 out (case-sensitive: false)
    line 4 out (case-sensitive: false)
    line 4 ratio (case-sensitive: false)
    line 9 n (case-sensitive: false)
    line 10 total (case-sensitive: false)
    line 11 g (case-sensitive: false)
    line 13 n (case-sensitive: false)
    line 14 g (case-sensitive: false)
    line 14 n (case-sensitive: false)
    ");
}

/// Stopping earlier shows less, and nothing after the stop.
#[rstest]
fn nothing_past_the_stop(mut with_db: RootDatabase) {
    assert_snapshot!(shown(&mut with_db, SOURCE, 9), @r"
    line 2 ratio (case-sensitive: false)
    line 3 out (case-sensitive: false)
    line 4 out (case-sensitive: false)
    line 4 ratio (case-sensitive: false)
    line 9 n (case-sensitive: false)
    ");
}

/// A FIELD step is not a name the debugger's scope holds: `g.out` offers `g`,
/// never a bare `out` that would resolve to some other variable entirely.
#[rstest]
fn a_field_step_is_not_offered_alone(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Gear
VAR_OUTPUT out : INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    g : Gear;
    out : INT;
END_VAR
    out := g.out;
END_PROGRAM"#;
    assert_snapshot!(shown(&mut with_db, source, 10), @r"
    line 2 out (case-sensitive: false)
    line 7 g (case-sensitive: false)
    line 8 out (case-sensitive: false)
    line 10 out (case-sensitive: false)
    line 10 g (case-sensitive: false)
    ");
}
