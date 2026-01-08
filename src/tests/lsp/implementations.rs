use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::request::GotoImplementationResponse;
use db::RootDatabase;
use ide_proto::to_proto::ToProtocol;
use insta::assert_debug_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::find_pou_with_name;
use crate::tests::utils::with_db;

#[rstest]
pub fn class_extends_class(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
        END_CLASS

        CLASS Mid EXTENDS Base
        END_CLASS
"#;

    add_sources(&mut with_db, &[source]);
    let cl = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "Base",
    )
    .unwrap();

    assert_debug_snapshot!(if let GotoImplementationResponse::Link(link) = cl.implementation(&with_db).unwrap() {
            link.iter().map(|link| {
                format!("{:?}\n", link.target_range)
            }).collect::<Vec<_>>()
        } else {
            panic!("Unexpected implementation content")
        }, @r#"
    [
        "Range { start: Position { line: 4, character: 8 }, end: Position { line: 5, character: 17 } }\n",
    ]
    "#);
}

#[rstest]
pub fn function_block_extends(mut with_db: RootDatabase) {
    let source = r#"
        CLASS Base
        END_CLASS

        FUNCTION_BLOCK Mid EXTENDS Base
        END_FUNCTION_BLOCK
"#;

    add_sources(&mut with_db, &[source]);
    let cl = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "Base",
    )
    .unwrap();

    assert_debug_snapshot!(if let GotoImplementationResponse::Link(link) = cl.implementation(&with_db).unwrap() {
            link.iter().map(|link| {
                format!("{:?}\n", link.target_range)
            }).collect::<Vec<_>>()
        } else {
            panic!("Unexpected implementation content")
        }, @r#"
    [
        "Range { start: Position { line: 4, character: 8 }, end: Position { line: 5, character: 26 } }\n",
    ]
    "#);
}

#[rstest]
pub fn interface_implements_interface(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE I1
        END_INTERFACE

        INTERFACE I2 Mid IMPLEMENTS I1
        END_INTERFACE
"#;

    add_sources(&mut with_db, &[source]);
    let cl =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "I1").unwrap();

    assert_debug_snapshot!(if let GotoImplementationResponse::Link(link) = cl.implementation(&with_db).unwrap() {
            link.iter().map(|link| {
                format!("{:?}\n", link.target_range)
            }).collect::<Vec<_>>()
        } else {
            panic!("Unexpected implementation content")
        }, @"[]");
}

#[rstest]
pub fn class_implements_multiple_interfaces(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE I1
        END_INTERFACE

        INTERFACE I2
        END_INTERFACE

        INTERFACE I3
        END_INTERFACE

        CLASS Mid IMPLEMENTS I1, I2, I3
        END_CLASS
"#;

    add_sources(&mut with_db, &[source]);
    let i1 =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "I1").unwrap();

    assert_debug_snapshot!(if let GotoImplementationResponse::Link(link) = i1.implementation(&with_db).unwrap() {
            link.iter().map(|link| {
                format!("{:?}\n", link.target_range)
            }).collect::<Vec<_>>()
        } else {
            panic!("Unexpected implementation content")
        }, @r#"
    [
        "Range { start: Position { line: 10, character: 8 }, end: Position { line: 11, character: 17 } }\n",
    ]
    "#);

    let i2 =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "I2").unwrap();

    assert_debug_snapshot!(if let GotoImplementationResponse::Link(link) = i2.implementation(&with_db).unwrap() {
            link.iter().map(|link| {
                format!("{:?}\n", link.target_range)
            }).collect::<Vec<_>>()
        } else {
            panic!("Unexpected implementation content")
        }, @r#"
    [
        "Range { start: Position { line: 10, character: 8 }, end: Position { line: 11, character: 17 } }\n",
    ]
    "#);

    let i3 =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "I3").unwrap();

    assert_debug_snapshot!(if let GotoImplementationResponse::Link(link) = i3.implementation(&with_db).unwrap() {
            link.iter().map(|link| {
                format!("{:?}\n", link.target_range)
            }).collect::<Vec<_>>()
        } else {
            panic!("Unexpected implementation content")
        }, @r#"
    [
        "Range { start: Position { line: 10, character: 8 }, end: Position { line: 11, character: 17 } }\n",
    ]
    "#);
}
