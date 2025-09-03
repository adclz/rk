use ariadne::CharSet;
use ariadne::Config;
use ariadne::Source;
use auto_lsp::{
    default::db::{FileManager, file::File},
    lsp_types::Url,
};
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use insta::assert_snapshot;
use rstest::{fixture, rstest};

use crate::tests::utils::test_diagnostic;
use crate::tests::utils::with_db;

#[rstest]
fn mismatch_type_in_assign(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:7:13 ]
       |
     4 |         test: INT;
       |         ^^|^  ^|^  
       |           `-------- 'test' is declared here
       |                |   
       |                `--- type defined here
       | 
     7 |     test := ULINT#5;
       |             ^^^|^^^  
       |                `----- expected a signed 16-bit integer
    ---'
    ");
}

#[rstest]
fn no_item_in_assign(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test2 := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostic(&mut with_db, source), @r"
    Error: 
       ,-[ file:///test.st:7:5 ]
       |
     7 |     test2 := ULINT#5;
       |     ^^|^^  
       |       `---- no item 'test2' in scope
    ---'
    ");
}

