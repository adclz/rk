use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::hir_ty::body::infer_body;
use hir::{HasName, HirNodeInfo};
use insta::assert_snapshot;
use rstest::rstest;

use super::utils::mir_exports;
use crate::tests::utils::{add_sources, find_pou_with_name, test_diagnostics, with_db};

/// Helper: collect fb_any_resolutions from a specific function's body inference.
fn fb_resolutions_in(db: &mut RootDatabase, sources: &[&str], func_name: &str) -> String {
    add_sources(db, sources);
    let url = auto_lsp::lsp_types::Url::parse("file:///test0.st").unwrap();
    let file = db.get_file(&url).expect("test0.st not found");
    let pou = find_pou_with_name(db, file, func_name)
        .unwrap_or_else(|| panic!("POU '{}' not found", func_name));

    let scope = pou.get_scope_id(db);
    let body = infer_body(db, scope);

    let mut results = Vec::new();
    for ((var_decl, field_name), concrete) in &body.fb_any_resolutions {
        let var_name = var_decl.get_name_ident(db).text(db).to_string();
        let field = field_name.text(db).to_string();
        results.push(format!("{}.{} → {:?}", var_name, field, concrete));
    }

    results.sort();
    results.join("\n")
}

// ── HIR: fb_any_resolutions correctly populated ─────────────────────────

#[rstest]
fn fb_any_int_resolved_from_call_site(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT PV: ANY_INT; END_VAR
VAR_OUTPUT CV: INTO(PV); END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR counter : CTU<INT>; END_VAR
    counter(PV := 10);
END_FUNCTION
    "#;
    assert_snapshot!(fb_resolutions_in(&mut with_db, &[source], "test"), @"counter.PV → Int");
}

#[rstest]
fn fb_any_real_resolved_from_call_site(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR_INPUT value: ANY_REAL; END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR fb : MyFB<REAL>; END_VAR
    fb(value := 3.14);
END_FUNCTION
    "#;
    assert_snapshot!(fb_resolutions_in(&mut with_db, &[source], "test"), @"fb.value → Real");
}

#[rstest]
fn fb_any_int_with_lint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT PV: ANY_INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR counter : CTU<LINT>; END_VAR
    counter(PV := LINT#100);
END_FUNCTION
    "#;
    assert_snapshot!(fb_resolutions_in(&mut with_db, &[source], "test"), @"counter.PV → LInt");
}

#[rstest]
fn fb_no_any_no_resolutions(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK SimpleFB
VAR_INPUT x: INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR fb : SimpleFB; END_VAR
    fb(x := 5);
END_FUNCTION
    "#;
    assert_snapshot!(fb_resolutions_in(&mut with_db, &[source], "test"), @"");
}

#[rstest]
fn fb_any_multiple_call_sites_same_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT PV: ANY_INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR counter : CTU<INT>; END_VAR
    counter(PV := 10);
    counter(PV := 20);
END_FUNCTION
    "#;
    assert_snapshot!(fb_resolutions_in(&mut with_db, &[source], "test"), @"counter.PV → Int");
}

#[rstest]
fn fb_any_diagnostics_valid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT PV: ANY_INT; END_VAR
VAR_OUTPUT CV: INTO(PV); END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR counter : CTU<INT>; END_VAR
    counter(PV := 10);
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// ── MIR: FB ANY_INT across multiple files ──────────────────────────────

#[rstest]
fn fb_any_int_cross_file(mut with_db: RootDatabase) {
    let fb_source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT PV: ANY_INT; END_VAR
VAR_OUTPUT CV: INTO(PV); END_VAR
    CV := PV;
END_FUNCTION_BLOCK
    "#;
    let caller_source = r#"
FUNCTION test
VAR counter : CTU<INT>; END_VAR
    counter(PV := 10);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[fb_source, caller_source]), @r"
    export CTU$INT$__body__(*struct(CTU$INT))
    export test()
    ");
}

// ── MIR: FB with ANY_INT lowers to concrete struct ──────────────────────

#[rstest]
fn fb_with_any_int_lowers(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT PV: ANY_INT; END_VAR
VAR_OUTPUT CV: INTO(PV); END_VAR
    CV := PV;
END_FUNCTION_BLOCK

FUNCTION test
VAR counter : CTU<INT>; END_VAR
    counter(PV := 10);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export CTU$INT$__body__(*struct(CTU$INT))
    export test()
    ");
}

#[rstest]
fn fb_with_any_int_field_access(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT PV: ANY_INT; END_VAR
VAR_OUTPUT CV: INTO(PV); END_VAR
    CV := PV;
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR counter : CTU<INT>; END_VAR
    counter(PV := 42);
    test := counter.CV;
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export CTU$INT$__body__(*struct(CTU$INT))
    export test() -> Int
    ");
}
