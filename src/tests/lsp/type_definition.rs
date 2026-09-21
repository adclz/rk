use auto_lsp::lsp_types::GotoDefinitionResponse;
use db::RootDatabase;
use ide_proto::{handlers::type_definition::type_definition, walk::descendant_at};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_source, with_db};

/// Where `textDocument/typeDefinition` lands, as the source text it selects.
fn target(db: &mut RootDatabase, source: &str, needle: &str) -> String {
    let file = add_source(db, source);
    let offset = source
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in the source"));
    let Some(node) = descendant_at(db, file, offset) else {
        return "<no node>".to_string();
    };
    let Some(answer) = type_definition(db, &node, offset) else {
        return "<none>".to_string();
    };
    let locations = match answer {
        GotoDefinitionResponse::Scalar(loc) => vec![loc],
        GotoDefinitionResponse::Array(locs) => locs,
        GotoDefinitionResponse::Link(links) => {
            return format!("{} link(s)", links.len());
        }
    };
    let lines: Vec<&str> = source.lines().collect();
    locations
        .iter()
        .map(|loc| {
            let line = loc.range.start.line as usize;
            let (a, b) = (
                loc.range.start.character as usize,
                loc.range.end.character as usize,
            );
            let text = lines.get(line).map(|l| &l[a.min(l.len())..b.min(l.len())]);
            format!("line {line}: {}", text.unwrap_or("?"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

const SOURCE: &str = r#"
TYPE Mode : (Idle, Running); END_TYPE

FUNCTION_BLOCK Gear
VAR_OUTPUT out : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION make : Gear
END_FUNCTION

PROGRAM Main
VAR
    m : Mode;
    g : Gear;
    n : INT;
END_VAR
    n := 1;
    m := Mode#Idle;
END_PROGRAM"#;

/// On a declaration, the TYPE, not the slot: `definition` lands on `m`, this
/// lands on `Mode`.
#[rstest]
fn a_declaration_goes_to_its_type(mut with_db: RootDatabase) {
    assert_snapshot!(target(&mut with_db, SOURCE, "m : Mode"), @"line 1: Mode");
}

/// And the same from a USE of it in the body.
#[rstest]
fn a_use_goes_to_its_type(mut with_db: RootDatabase) {
    assert_snapshot!(target(&mut with_db, SOURCE, "m := Mode#Idle"), @"line 1: Mode");
}

#[rstest]
fn a_block_typed_variable_goes_to_the_block(mut with_db: RootDatabase) {
    assert_snapshot!(target(&mut with_db, SOURCE, "g : Gear"), @"line 3: Gear");
}

/// An elementary type is written nowhere, so there is nothing to go to and
/// the request says so rather than landing on the declaration.
#[rstest]
fn an_elementary_type_has_nowhere_to_go(mut with_db: RootDatabase) {
    assert_snapshot!(target(&mut with_db, SOURCE, "n : INT"), @"<none>");
}
