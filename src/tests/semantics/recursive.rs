use crate::tests::utils::{test_diagnostics, with_db};
use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

#[rstest]
fn self_referential(mut with_db: RootDatabase) {
    let source = r#"
      FUNCTION_BLOCK fb
            VAR_INPUT
                invalid : fb;
            END_VAR

        END_FUNCTION_BLOCK

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: recursion detected
       ,-[ file:///test0.st:2:22 ]
       |
     2 |       FUNCTION_BLOCK fb
       |                      ^|
       |                       `-- type 'fb' is recursive (contains itself)
       |
     4 |                 invalid : fb;
       |                           ^|
       |                            `-- 'fb' references itself here
    ---'
    ");
}

#[rstest]
fn recursive_function_blocks(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
              VAR_INPUT
                  invalid : fb2;
              END_VAR

        END_FUNCTION_BLOCK

        FUNCTION_BLOCK fb2
              VAR_INPUT
                  invalid : fb1;
              END_VAR

        END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1302] Error: recursion detected
        ,-[ file:///test0.st:2:24 ]
        |
      2 |         FUNCTION_BLOCK fb1
        |                        ^|^
        |                         `--- type 'fb1' is recursive
        |
     11 |                   invalid : fb1;
        |                             ^|^
        |                              `--- recurses at this location
        |
        | Note: cycle goes
        |       -> fb1
        |       -> fb2
        |       ... and back to fb1
    ----'
    ");
}

#[rstest]
fn self_referential_struct(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine: STRUCT
                sub_engine: Engine;
            END_STRUCT
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: recursion detected
       ,-[ file:///test0.st:2:14 ]
       |
     2 |         TYPE Engine: STRUCT
       |              ^^^|^^
       |                 `---- type 'Engine' is recursive (contains itself)
     3 |                 sub_engine: Engine;
       |                 ^^^^^^^^^|^^^^^^^^
       |                          `---------- 'Engine' references itself here
    ---'
    ");
}

#[rstest]
fn mutually_referential_types(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            A: B;
            B: A;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1302] Error: recursion detected
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             A: B;
       |             |
       |             `-- type 'A' is recursive
     4 |             B: A;
       |                |
       |                `-- recurses at this location
       |
       | Note: cycle goes
       |       -> A
       |       -> B
       |       ... and back to A
    ---'
    ");
}

#[rstest]
fn mutually_referential_type_and_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            A: B;
            B: ARRAY[1..2] OF A;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1302] Error: recursion detected
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             A: B;
       |             |
       |             `-- type 'A' is recursive
     4 |             B: ARRAY[1..2] OF A;
       |                               |
       |                               `-- recurses at this location
       |
       | Note: cycle goes
       |       -> A
       |       -> B
       |       ... and back to A
    ---'
    ");
}

#[rstest]
fn class_extends_itself(mut with_db: RootDatabase) {
    let source = r#"
    CLASS MyClass EXTENDS MyClass
    END_CLASS
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: recursion detected
       ,-[ file:///test0.st:2:11 ]
       |
     2 |     CLASS MyClass EXTENDS MyClass
       |           ^^^|^^^         ^^^|^^^
       |              `--------------------- type 'MyClass' is recursive (contains itself)
       |                              |
       |                              `----- 'MyClass' references itself here
    ---'
    ");
}

#[rstest]
fn fb_extends_itself(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION_BLOCK MyFb EXTENDS MyFb
    END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: recursion detected
       ,-[ file:///test0.st:2:20 ]
       |
     2 |     FUNCTION_BLOCK MyFb EXTENDS MyFb
       |                    ^^|^         ^^|^
       |                      `---------------- type 'MyFb' is recursive (contains itself)
       |                                   |
       |                                   `--- 'MyFb' references itself here
    ---'
    ");
}

#[rstest]
fn interface_extends_itself(mut with_db: RootDatabase) {
    let source = r#"
    INTERFACE MyInterface EXTENDS MyInterface
    END_INTERFACE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: recursion detected
       ,-[ file:///test0.st:2:15 ]
       |
     2 |     INTERFACE MyInterface EXTENDS MyInterface
       |               ^^^^^|^^^^^         ^^^^^|^^^^^
       |                    `--------------------------- type 'MyInterface' is recursive (contains itself)
       |                                        |
       |                                        `------- 'MyInterface' references itself here
    ---'
    ");
}

#[rstest]
fn recursion_in_namespace(mut with_db: RootDatabase) {
    let source = r#"
    NAMESPACE ns 
        FUNCTION_BLOCK fb1
              VAR_INPUT
                  invalid : ns.ns2.fb2;
              END_VAR

        END_FUNCTION_BLOCK

        NAMESPACE ns2 
            FUNCTION_BLOCK fb2
                  VAR_INPUT
                      invalid : ns.fb1;
                END_VAR

            END_FUNCTION_BLOCK
        END_NAMESPACE
    END_NAMESPACE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1302] Error: recursion detected
        ,-[ file:///test0.st:11:28 ]
        |
      5 |                   invalid : ns.ns2.fb2;
        |                             ^^^^^|^^^^
        |                                  `------ recurses at this location
        |
     11 |             FUNCTION_BLOCK fb2
        |                            ^|^
        |                             `--- type 'fb2' is recursive
        |
        | Note: cycle goes
        |       -> fb2
        |       -> fb1
        |       ... and back to fb2
    ----'
    ");
}
