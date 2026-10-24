// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::tests::utils::{add_library_sources, test_diagnostics, with_db};
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
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:22 ]
       |
     2 |       FUNCTION_BLOCK fb
       |                      ^|
       |                       `-- type 'fb' contains itself
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
    [E1302] Error: types contain each other
        ,-[ file:///test0.st:2:24 ]
        |
      2 |         FUNCTION_BLOCK fb1
        |                        ^|^
        |                         `--- type 'fb1' is recursive
        |
     11 |                   invalid : fb1;
        |                             ^|^
        |                              `--- the cycle passes here
        |
        | Note: cycle goes
        |       -> fb1
        |       -> fb2
        |       ... and back to fb1
    ----'
    ");
}

/// A VAR_IN_OUT member holds the address of the caller's instance, like a
/// REF_TO: two FBs that refer to each other through one contain no copy of
/// each other. It was E1302.
#[rstest]
fn function_blocks_referring_to_each_other_through_in_outs(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motor
        VAR_IN_OUT drive : Drive; END_VAR
        VAR_OUTPUT seen : INT; END_VAR
            seen := drive.speed;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Drive
        VAR_IN_OUT motor : Motor; END_VAR
        VAR_OUTPUT speed : INT := 7; END_VAR
        END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// An array that contains itself is reported where it is declared, and a
/// variable of it is checked like any other: followed element by element
/// the type never ended, and `rk check` hung on the variable (a fuzzer
/// finding).
#[rstest]
fn a_variable_of_a_self_containing_array_is_checked(mut with_db: RootDatabase) {
    let source = r#"
TYPE RowT : ARRAY[0..2] OF RowT; END_TYPE

FUNCTION run : DINT
VAR
    full : ARRAY[0..1] OF RowT;
END_VAR
    run := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:6 ]
       |
     2 | TYPE RowT : ARRAY[0..2] OF RowT; END_TYPE
       |      ^^|^                  ^^|^
       |        `------------------------- type 'RowT' contains itself
       |                              |
       |                              `--- 'RowT' references itself here
    ---'
    ");
}

/// Two arrays that contain themselves are nothing to each other: a copy
/// from one to the other, or a reference to one pointed at the other, is
/// refused as any mismatch is, where comparing them element by element
/// overflowed the stack (a fuzzer finding).
#[rstest]
fn two_self_containing_arrays_are_compared_without_looping(mut with_db: RootDatabase) {
    let source = r#"
TYPE A : ARRAY[0..1] OF A; B : ARRAY[0..1] OF B; END_TYPE

FUNCTION f : DINT
VAR
    a : ARRAY[0..1] OF A;
    b : ARRAY[0..1] OF B;
    r : REF_TO A;
END_VAR
    a := b;
    r := REF(b);
    f := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:6 ]
       |
     2 | TYPE A : ARRAY[0..1] OF A; B : ARRAY[0..1] OF B; END_TYPE
       |      |                  |
       |      `--------------------- type 'A' contains itself
       |                         |
       |                         `-- 'A' references itself here
    ---'
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:28 ]
       |
     2 | TYPE A : ARRAY[0..1] OF A; B : ARRAY[0..1] OF B; END_TYPE
       |                            |                  |
       |                            `--------------------- type 'B' contains itself
       |                                               |
       |                                               `-- 'B' references itself here
    ---'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:10 ]
        |
      6 |     a : ARRAY[0..1] OF A;
        |     |
        |     `-- 'a' is declared here
        |
     10 |     a := b;
        |          |
        |          `-- expected 'ARRAY [0..1] OF A', got 'ARRAY [0..1] OF B'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:10 ]
        |
      8 |     r : REF_TO A;
        |     |
        |     `-- 'r' is declared here
        |
     11 |     r := REF(b);
        |          ^^^|^^
        |             `---- expected 'REF_TO A', got 'REF_TO ARRAY [0..1] OF B'
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
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:14 ]
       |
     2 |         TYPE Engine: STRUCT
       |              ^^^|^^
       |                 `---- type 'Engine' contains itself
     3 |                 sub_engine: Engine;
       |                             ^^^|^^
       |                                `---- 'Engine' references itself here
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
    [E1302] Error: types contain each other
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             A: B;
       |             |
       |             `-- type 'A' is recursive
     4 |             B: A;
       |                |
       |                `-- the cycle passes here
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
    [E1302] Error: types contain each other
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             A: B;
       |             |
       |             `-- type 'A' is recursive
     4 |             B: ARRAY[1..2] OF A;
       |                               |
       |                               `-- the cycle passes here
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
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:11 ]
       |
     2 |     CLASS MyClass EXTENDS MyClass
       |           ^^^|^^^         ^^^|^^^
       |              `--------------------- type 'MyClass' contains itself
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
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:20 ]
       |
     2 |     FUNCTION_BLOCK MyFb EXTENDS MyFb
       |                    ^^|^         ^^|^
       |                      `---------------- type 'MyFb' contains itself
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
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:15 ]
       |
     2 |     INTERFACE MyInterface EXTENDS MyInterface
       |               ^^^^^|^^^^^         ^^^^^|^^^^^
       |                    `--------------------------- type 'MyInterface' contains itself
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
    [E1302] Error: types contain each other
        ,-[ file:///test0.st:11:28 ]
        |
      5 |                   invalid : ns.ns2.fb2;
        |                             ^^^^^|^^^^
        |                                  `------ the cycle passes here
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

/// An alias of itself, and a cycle of aliases, each with a variable of its
/// type. The cycle is reported once, at the declaration, and the variable
/// is checked no further: the checker used to follow the cycle through the
/// variable until the stack ran out, on a four-line file.
#[rstest]
fn a_variable_of_a_cyclic_alias_is_reported_once(mut with_db: RootDatabase) {
    let source = r#"
TYPE A : A; END_TYPE
TYPE B : C; C : D; D : B; END_TYPE

FUNCTION f : INT
VAR a : A; b : B; END_VAR
    a := b;
    f := a;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:6 ]
       |
     2 | TYPE A : A; END_TYPE
       |      |   |
       |      `------ type 'A' contains itself
       |          |
       |          `-- 'A' references itself here
    ---'
    [E1302] Error: types contain each other
       ,-[ file:///test0.st:3:6 ]
       |
     3 | TYPE B : C; C : D; D : B; END_TYPE
       |      |                 |
       |      `-------------------- type 'B' is recursive
       |                        |
       |                        `-- the cycle passes here
       |
       | Note: cycle goes
       |       -> B
       |       -> C
       |       -> D
       |       ... and back to B
    ---'
    ");
}

/// The same cycle reached through an aggregate, an instance or a path: a
/// STRUCT field, an ARRAY element, a variable of a PROGRAM or of a
/// FUNCTION_BLOCK, a member, a subscript, a dereference, a call and an
/// initializer. A FUNCTION's local was already safe, and each of these
/// took `rk check` down with the stack: the layout went from the name to
/// its spec and back, and so did the walk of a path. The cycle is what is
/// reported, once, where it is declared.
#[rstest]
fn a_cyclic_alias_is_reported_once_wherever_it_is_used(mut with_db: RootDatabase) {
    let source = r#"
TYPE A : A; END_TYPE
TYPE B : C; C : B; END_TYPE

TYPE
    Holder : STRUCT
        a : A;
        b : B;
    END_STRUCT;
    Row : ARRAY[1..3] OF A;
END_TYPE

FUNCTION_BLOCK Block
VAR a : A; b : B; END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR a : A; b : B; h : Holder; r : Row; k : Block; i : INT; END_VAR
    i := a.x;
    i := b.x.y;
    i := a[1];
    i := b^;
    i := h.a.x;
    i := r[1].x;
    i := k.a.x;
    a.x := 1;
    a();
END_PROGRAM

FUNCTION f : INT
VAR a : A := (x := 1); b : B := [1, 2]; END_VAR
    f := a.x + b[1];
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:6 ]
       |
     2 | TYPE A : A; END_TYPE
       |      |   |
       |      `------ type 'A' contains itself
       |          |
       |          `-- 'A' references itself here
    ---'
    [E1302] Error: types contain each other
       ,-[ file:///test0.st:3:6 ]
       |
     3 | TYPE B : C; C : B; END_TYPE
       |      |          |
       |      `------------- type 'B' is recursive
       |                 |
       |                 `-- the cycle passes here
       |
       | Note: cycle goes
       |       -> B
       |       -> C
       |       ... and back to B
    ---'
    ");
}

/// The reference a cycle cites is the first one written. The members were
/// visited in the order of a map keyed by interned names, which is the
/// order the names were first met in: this block cited `c`, and the editor
/// disagreed with a fresh check on the same text after an edit.
#[rstest]
fn a_cycle_cites_the_first_reference_written(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Inner
VAR
    n : INT;
    a : Inner;
    b : Inner;
    c : Inner;
    d : Inner;
    e : Inner;
    f : Inner;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1301] Error: type contains itself
       ,-[ file:///test0.st:2:16 ]
       |
     2 | FUNCTION_BLOCK Inner
       |                ^^|^^
       |                  `---- type 'Inner' contains itself
       |
     5 |     a : Inner;
       |         ^^|^^
       |           `---- 'Inner' references itself here
    ---'
    ");
}

/// A type that contains itself is reported once, in its own file: a block
/// of another file that holds it does not report it again, at the other
/// file's lines.
#[rstest]
fn a_self_containing_type_is_reported_in_its_own_file_only(mut with_db: RootDatabase) {
    let holder = r#"
FUNCTION_BLOCK holder
VAR
    inner : fb;
END_VAR
END_FUNCTION_BLOCK
"#;
    let fb = r#"
FUNCTION_BLOCK fb
VAR
    invalid : fb;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[holder, fb]), @r"
    [E1301] Error: type contains itself
       ,-[ file:///test1.st:2:16 ]
       |
     2 | FUNCTION_BLOCK fb
       |                ^|
       |                 `-- type 'fb' contains itself
       |
     4 |     invalid : fb;
       |               ^|
       |                `-- 'fb' references itself here
    ---'
    ");
}

/// A library's type that contains itself is still reported where a block of
/// the workspace holds it: nothing shows the library's own report, and the
/// workspace would check clean.
#[rstest]
fn a_library_s_self_containing_type_is_reported_where_it_is_held(mut with_db: RootDatabase) {
    add_library_sources(
        &mut with_db,
        &["FUNCTION_BLOCK fb\nVAR\n    invalid : fb;\nEND_VAR\nEND_FUNCTION_BLOCK\n"],
    );
    let holder = "FUNCTION_BLOCK holder\nVAR\n    inner : Std.fb;\nEND_VAR\nEND_FUNCTION_BLOCK\n";
    let report = test_diagnostics(&mut with_db, &[holder]);
    assert!(report.contains("[E1301]"), "{report}");
}

/// Two blocks holding each other have no layout, and each lists its own
/// located members: a layout, and a list of them, are queries that reach
/// themselves through the other block, which salsa settles with a value for
/// every POU on the cycle. The layout is none, as neither block has a
/// size; the list is the block's own members, so a VAR_CONFIG locating one
/// of them still finds it.
#[rstest]
fn blocks_holding_each_other_keep_their_own_located_members(mut with_db: RootDatabase) {
    use hir::HasName;
    use hir::hir_ty::head::instances::partly_located_members;
    use hir::hir_ty::layout::instance_layout;
    let source = r#"
FUNCTION_BLOCK a
VAR
    bb : b;
    level AT %I* : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK b
VAR
    aa : a;
END_VAR
END_FUNCTION_BLOCK
"#;
    let file = crate::tests::utils::add_source(&mut with_db, source);
    let located = |name: &str| -> Vec<Vec<String>> {
        let pou = crate::tests::utils::find_pou_with_name(&with_db, file, name).unwrap();
        assert!(
            instance_layout(&with_db, pou).is_none(),
            "{name} has a layout: {:?}",
            instance_layout(&with_db, pou)
        );
        partly_located_members(&with_db, pou)
            .iter()
            .map(|path| {
                path.iter()
                    .map(|var| var.get_name_with_case(&with_db).text(&with_db).to_string())
                    .collect()
            })
            .collect()
    };
    assert_eq!(located("a"), [["level"]]);
    assert_eq!(located("b"), [] as [[&str; 0]; 0]);
    assert_snapshot!(test_diagnostics(&mut with_db, &[]), @r"
    [E1302] Error: types contain each other
        ,-[ file:///test0.st:2:16 ]
        |
      2 | FUNCTION_BLOCK a
        |                |
        |                `-- type 'a' is recursive
        |
     11 |     aa : a;
        |          |
        |          `-- the cycle passes here
        |
        | Note: cycle goes
        |       -> a
        |       -> b
        |       ... and back to a
    ----'
    ");
}
