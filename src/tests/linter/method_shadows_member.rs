// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn method_local_shadows_fb_member(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Counter
VAR
    c : INT;
END_VAR
    METHOD Inc : INT
    VAR
        c : INT;
    END_VAR
        c := c + 1;
        Inc := c;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "method-shadows-member"), @r"
    [L0116] Warning: method variable named after a member
       ,-[ file:///test0.st:8:9 ]
       |
     4 |     c : INT;
       |     |
       |     `-- member 'c' is declared here
       |
     8 |         c : INT;
       |         |
       |         `-- method variable 'c' hides the member 'c' of its block
       |
       | Note: lint rule: method-shadows-member
    ---'
    ");
}

#[rstest]
fn method_param_shadows_fb_member(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
VAR
    speed : INT;
END_VAR
    METHOD SetSpeed : INT
    VAR_INPUT
        speed : INT;
    END_VAR
        SetSpeed := speed;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "method-shadows-member"), @r"
    [L0116] Warning: method variable named after a member
       ,-[ file:///test0.st:8:9 ]
       |
     4 |     speed : INT;
       |     ^^|^^
       |       `---- member 'speed' is declared here
       |
     8 |         speed : INT;
       |         ^^|^^
       |           `---- method variable 'speed' hides the member 'speed' of its block
       |
       | Note: lint rule: method-shadows-member
    ---'
    ");
}

#[rstest]
fn class_method_local_shadows_member(mut with_db: RootDatabase) {
    let source = r#"
CLASS Widget
VAR
    id : INT;
END_VAR
    METHOD Get : INT
    VAR
        id : INT;
    END_VAR
        Get := id;
    END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "method-shadows-member"), @r"
    [L0116] Warning: method variable named after a member
       ,-[ file:///test0.st:8:9 ]
       |
     4 |     id : INT;
       |     ^|
       |      `-- member 'id' is declared here
       |
     8 |         id : INT;
       |         ^|
       |          `-- method variable 'id' hides the member 'id' of its block
       |
       | Note: lint rule: method-shadows-member
    ---'
    ");
}

#[rstest]
fn no_shadow_distinct_names(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Counter
VAR
    c : INT;
END_VAR
    METHOD Inc : INT
    VAR
        tmp : INT;
    END_VAR
        Inc := c + tmp;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "method-shadows-member"), @r"");
}

/// Each is named as it is declared: the member is `Speed`, the method's
/// variable `speed`.
#[rstest]
fn the_member_is_named_as_declared(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
VAR
    Speed : INT;
END_VAR
    METHOD Set : INT
    VAR
        speed : INT;
    END_VAR
        Set := speed;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "method-shadows-member"), @r"
    [L0116] Warning: method variable named after a member
       ,-[ file:///test0.st:8:9 ]
       |
     4 |     Speed : INT;
       |     ^^|^^
       |       `---- member 'Speed' is declared here
       |
     8 |         speed : INT;
       |         ^^|^^
       |           `---- method variable 'speed' hides the member 'Speed' of its block
       |
       | Note: lint rule: method-shadows-member
    ---'
    ");
}
