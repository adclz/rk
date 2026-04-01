use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HirNodeInfo;
use ide_proto::handlers::completions_utils::completion_item_builder::{
    CompletionBuilder, build_call_signature,
};
use ide_proto::handlers::completions_utils::QueryMode;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, with_db};

// only INPUT, OUTPUT and IN_OUT variables should be included in the call signature, not VAR or VAR_TEMP
#[rstest]
pub fn call_signature_with_global_vars(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_INPUT
        input_var1: INT;
        input_var2: REAL;
    END_VAR

    VAR_OUTPUT
        output_var1: INT;
        output_var2: REAL;
    END_VAR

    VAR_IN_OUT
        inout_var1: INT;
        inout_var2: REAL;
    END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fn").unwrap();

    let sig = build_call_signature(&with_db, &"fn".to_owned(), pou.get_scope_id(&with_db));
    assert_snapshot!(sig, @r"
    fn(
    	input_var1 := ${1:input_var1},
    	input_var2 := ${2:input_var2},
    	output_var1 => ${3:output_var1},
    	output_var2 => ${4:output_var2},
    	inout_var1 := ${5:inout_var1},
    	inout_var2 := ${6:inout_var2}
    )
    ");
}

// VAR and VAR_TEMP variables should not be included in the call signature
#[rstest]
pub fn call_signature_with_local_vars(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR
        var1: INT;
        var2: REAL;
    END_VAR

    VAR_TEMP
        temp_var1: INT;
        temp_var2: REAL;
    END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou =
        find_pou_with_name(&with_db, *with_db.get_files().iter().last().unwrap(), "fn").unwrap();

    let sig = build_call_signature(&with_db, &"fn".to_owned(), pou.get_scope_id(&with_db));
    assert_snapshot!(sig, @"fn()");
}

// A variable typed as a FUNCTION_BLOCK should produce a call signature
// with the FB's INPUT/OUTPUT/IN_OUT parameters, not the declaring scope's variables.
#[rstest]
pub fn call_signature_for_fb_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK my_fb
    VAR_INPUT
        fb_input: INT;
    END_VAR
    VAR_OUTPUT
        fb_output: REAL;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION caller
    VAR
        inst : my_fb;
    END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "caller",
    )
    .unwrap();

    let builder = CompletionBuilder::default().with_mode(QueryMode::Body);
    let var = pou
        .get_scope_id(&with_db)
        .def_map(&with_db)
        .global_variables
        .values()
        .next()
        .unwrap();

    let item = builder.build_variable(&with_db, var);
    assert_snapshot!(item.insert_text.unwrap(), @"inst(fb_input := ${1:fb_input}, fb_output => ${2:fb_output})");
}

// A variable typed as a FUNCTION_BLOCK with no INPUT/OUTPUT should produce empty parens.
#[rstest]
pub fn call_signature_for_fb_variable_no_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK empty_fb
    VAR
        internal: INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION caller
    VAR
        inst : empty_fb;
    END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source]);
    let pou = find_pou_with_name(
        &with_db,
        *with_db.get_files().iter().last().unwrap(),
        "caller",
    )
    .unwrap();

    let builder = CompletionBuilder::default().with_mode(QueryMode::Body);
    let var = pou
        .get_scope_id(&with_db)
        .def_map(&with_db)
        .global_variables
        .values()
        .next()
        .unwrap();

    let item = builder.build_variable(&with_db, var);
    assert_snapshot!(item.insert_text.unwrap(), @"inst()");
}
