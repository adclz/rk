//! Every function lowers to a symbol of its own: the spellings
//! `mir::lower::naming` promises, and the internal error when two functions
//! would still meet on one.

use db::RootDatabase;
use hir::hir_def::semantic_index::semantic_index;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{
    add_source, add_sources, assert_workspace_is_clean, lower_workspace, with_db,
};

/// Every function's MIR symbol, sorted: what calls find their callee by,
/// which is not always the name a host sees exported.
fn symbols(db: &mut RootDatabase, source: &str) -> String {
    add_sources(db, &[source]);
    assert_workspace_is_clean(db);
    let module = lower_workspace(db);
    let mut names: Vec<String> = module
        .functions
        .iter()
        .map(|f| f.name.text(db).to_string())
        .collect();
    names.sort();
    names.join("\n")
}

// Bug 1: the aliases were stripped before mangling, every array became `T`,
// and the three overloads shared `Which2$T`.
#[rstest]
fn overloads_on_named_arrays_are_named_by_the_alias(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Real5 : ARRAY[0..4] OF REAL; Int5 : ARRAY[0..4] OF INT; Int7 : ARRAY[0..6] OF INT; END_TYPE
        FUNCTION Which2 : INT VAR_INPUT IN : Real5; END_VAR Which2 := 2; END_FUNCTION
        FUNCTION Which2 : INT VAR_INPUT IN : Int5;  END_VAR Which2 := 1; END_FUNCTION
        FUNCTION Which2 : INT VAR_INPUT IN : Int7;  END_VAR Which2 := 7; END_FUNCTION
    "#;
    assert_snapshot!(symbols(&mut with_db, source), @"
    Which2$Int5
    Which2$Int7
    Which2$Real5
    ");
}

#[rstest]
fn overloads_on_named_enums_structs_and_references(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Color : (Red, Green);
            Shape : (Circle, Square);
            Point : STRUCT x : INT; END_STRUCT;
            Size : STRUCT w : INT; END_STRUCT;
            RInt : REF_TO INT;
            RReal : REF_TO REAL;
        END_TYPE
        // Named like the fragment every one of these used to get.
        FUNCTION_BLOCK T VAR x : INT; END_VAR END_FUNCTION_BLOCK

        FUNCTION ByEnum : INT VAR_INPUT c : Color; END_VAR ByEnum := 1; END_FUNCTION
        FUNCTION ByEnum : INT VAR_INPUT s : Shape; END_VAR ByEnum := 2; END_FUNCTION
        FUNCTION ByStruct : INT VAR_INPUT p : Point; END_VAR ByStruct := 1; END_FUNCTION
        FUNCTION ByStruct : INT VAR_INPUT s : Size; END_VAR ByStruct := 2; END_FUNCTION
        FUNCTION ByRef : INT VAR_INPUT r : RInt; END_VAR ByRef := 1; END_FUNCTION
        FUNCTION ByRef : INT VAR_INPUT r : RReal; END_VAR ByRef := 2; END_FUNCTION
        FUNCTION ByFb : INT VAR_IN_OUT v : T; END_VAR ByFb := 1; END_FUNCTION
        FUNCTION ByFb : INT VAR_INPUT p : Point; END_VAR ByFb := 2; END_FUNCTION
    "#;
    assert_snapshot!(symbols(&mut with_db, source), @"
    ByEnum$Color
    ByEnum$Shape
    ByFb$Point
    ByFb$T
    ByRef$RInt
    ByRef$RReal
    ByStruct$Point
    ByStruct$Size
    T$__body__
    ");
}

// A type with no name of its own is spelled by its structure, in brackets
// no identifier can contain.
#[rstest]
fn unnamed_parameter_types_are_spelled_by_structure(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Which : INT VAR_INPUT v : ARRAY[0..4] OF INT; END_VAR Which := 1; END_FUNCTION
        FUNCTION Which : INT VAR_INPUT v : ARRAY[1..2, 0..1] OF REAL; END_VAR Which := 2; END_FUNCTION
        FUNCTION Which : INT VAR_INPUT v : REF_TO INT; END_VAR Which := 3; END_FUNCTION
        FUNCTION Which : INT VAR_INPUT v : INT(0..10); END_VAR Which := 4; END_FUNCTION
    "#;
    assert_snapshot!(symbols(&mut with_db, source), @"
    Which$ARRAY[0..4](INT)
    Which$ARRAY[1..2,0..1](REAL)
    Which$INT(0..10)
    Which$REF_TO(INT)
    ");
}

// `Motion.Axis` used to be folded to `Motion_Axis`, which is another type's
// name.
#[rstest]
fn namespaced_parameter_types_keep_their_dot(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motion_Axis VAR x : INT; END_VAR END_FUNCTION_BLOCK
        NAMESPACE Motion
            FUNCTION_BLOCK Axis VAR x : INT; END_VAR END_FUNCTION_BLOCK
        END_NAMESPACE
        FUNCTION Which : INT VAR_IN_OUT v : Motion_Axis; END_VAR Which := 1; END_FUNCTION
        FUNCTION Which : INT VAR_IN_OUT v : Motion.Axis; END_VAR Which := 2; END_FUNCTION
    "#;
    assert_snapshot!(symbols(&mut with_db, source), @"
    Motion.Axis$__body__
    Motion_Axis$__body__
    Which$Motion.Axis
    Which$Motion_Axis
    ");
}

// What is still left to collide stops lowering rather than lets a call run
// another body. A PROGRAM and a FUNCTION_BLOCK of one name are not refused at
// check yet, and both bodies are `Main$__body__`.
#[rstest]
fn two_functions_under_one_symbol_stop_lowering(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Main VAR x : INT; END_VAR x := x + 1; END_FUNCTION_BLOCK
        PROGRAM Main VAR n : INT; END_VAR n := n + 1; END_PROGRAM
    "#;
    let file = add_source(&mut with_db, source);
    let error = mir::lower::lower_module::lower_module(&with_db, semantic_index(&with_db, file))
        .map(|_| ())
        .expect_err("two bodies under one symbol");
    assert_snapshot!(error, @"two functions lower to the symbol `Main$__body__`");
}
