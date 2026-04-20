//! FB/Class generic tests: declaration derivation, use-site data flow,
//! call-site resolution, and E032x validation diagnostics.
//!
//! IEC 61131-3 keeps `ANY_*` as specs on VAR_INPUT/etc.; the implicit
//! generic parameter list is the ordered, deduplicated set of those specs
//! on a POU's top-level fields. Use sites (`VAR c : Counter<INT>;`) must
//! supply explicit type arguments — no more silent call-site inference
//! through struct fields or opaque references.

use auto_lsp::default::db::BaseDatabase;
use db::RootDatabase;
use hir::HasName;
use hir::hir_def::expressions::spec::{ElementarySpec, Spec, SpecKind};
use hir::hir_def::pous::function_block::FunctionBlock;
use hir::hir_def::pous::generics::fb_generic_params;
use hir::hir_def::pous::pou::Pou;
use hir::hir_def::semantic_index::semantic_index;
use hir::hir_ty::body::infer_body;
use hir::HirNodeInfo;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, find_pou_with_name, test_diagnostics, with_db};

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Collect fb_any_resolutions from a specific function's body inference.
/// Format: "instance_var.fb_field → ConcreteType".
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

fn find_fb<'db>(db: &'db RootDatabase, name: &str) -> FunctionBlock<'db> {
    let url = auto_lsp::lsp_types::Url::parse("file:///test0.st").unwrap();
    let file = db.get_file(&url).expect("test file not registered");
    let idx = semantic_index(db, file);
    for pou in idx.global_pous.iter() {
        if let Pou::FunctionBlock(fb) = pou
            && fb.name(db).text(db) == name
        {
            return *fb;
        }
    }
    panic!("FB '{}' not found", name);
}

fn elementary_of(db: &RootDatabase, spec: &Spec) -> ElementarySpec {
    match spec.kind(db) {
        SpecKind::Simple(e) => *e,
        other => panic!("expected Simple elementary, got {:?}", other),
    }
}

// ─── Declaration-side: derive_generic_params ────────────────────────────────

#[rstest]
fn derive_non_generic_fb_has_empty_params(mut with_db: RootDatabase) {
    add_sources(
        &mut with_db,
        &["FUNCTION_BLOCK Plain VAR_INPUT x : INT; END_VAR END_FUNCTION_BLOCK"],
    );
    let fb = find_fb(&with_db, "Plain");
    assert!(fb_generic_params(&with_db, fb).is_empty());
}

#[rstest]
fn derive_fb_with_single_any_int(mut with_db: RootDatabase) {
    add_sources(
        &mut with_db,
        &["FUNCTION_BLOCK Counter VAR_INPUT PV : ANY_INT; END_VAR END_FUNCTION_BLOCK"],
    );
    let params = fb_generic_params(&with_db, find_fb(&with_db, "Counter"));
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].bound, ElementarySpec::AnyInt);
}

#[rstest]
fn derive_fb_dedupes_same_any_kind(mut with_db: RootDatabase) {
    add_sources(
        &mut with_db,
        &["FUNCTION_BLOCK Counter VAR_INPUT a : ANY_INT; b : ANY_INT; END_VAR END_FUNCTION_BLOCK"],
    );
    let params = fb_generic_params(&with_db, find_fb(&with_db, "Counter"));
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].bound, ElementarySpec::AnyInt);
}

#[rstest]
fn derive_fb_keeps_distinct_any_kinds_in_order(mut with_db: RootDatabase) {
    add_sources(
        &mut with_db,
        &["FUNCTION_BLOCK Pair VAR_INPUT a : ANY_INT; b : ANY_REAL; END_VAR END_FUNCTION_BLOCK"],
    );
    let params = fb_generic_params(&with_db, find_fb(&with_db, "Pair"));
    assert_eq!(params.len(), 2);
    assert_eq!(params[0].bound, ElementarySpec::AnyInt);
    assert_eq!(params[1].bound, ElementarySpec::AnyReal);
}

// ─── Use-site data flow: `<INT>` lands on the VAR's Spec ───────────────────

#[rstest]
fn var_without_type_args_has_empty_args(mut with_db: RootDatabase) {
    add_sources(
        &mut with_db,
        &[r#"
FUNCTION_BLOCK Plain VAR_INPUT x : INT; END_VAR END_FUNCTION_BLOCK

FUNCTION_BLOCK Main
VAR c : Plain; END_VAR
END_FUNCTION_BLOCK
"#],
    );
    let fb = find_fb(&with_db, "Main");
    let c = fb
        .variables(&with_db)
        .iter()
        .find(|v| v.get_name_ident(&with_db).text(&with_db) == "c")
        .expect("c var");
    match c.spec(&with_db).kind(&with_db) {
        SpecKind::Target(t) => assert!(t.type_args.is_empty()),
        other => panic!("expected Target, got {:?}", other),
    }
}

#[rstest]
fn var_with_single_type_arg_on_generic_fb(mut with_db: RootDatabase) {
    add_sources(
        &mut with_db,
        &[r#"
FUNCTION_BLOCK Counter VAR_INPUT PV : ANY_INT; END_VAR END_FUNCTION_BLOCK

FUNCTION_BLOCK Main
VAR c : Counter<DINT>; END_VAR
END_FUNCTION_BLOCK
"#],
    );
    let fb = find_fb(&with_db, "Main");
    let c = fb
        .variables(&with_db)
        .iter()
        .find(|v| v.get_name_ident(&with_db).text(&with_db) == "c")
        .expect("c var");
    let target = match c.spec(&with_db).kind(&with_db) {
        SpecKind::Target(t) => t,
        other => panic!("expected Target, got {:?}", other),
    };
    assert_eq!(target.type_args.len(), 1);
    assert_eq!(
        elementary_of(&with_db, &target.type_args[0]),
        ElementarySpec::DInt
    );
}

#[rstest]
fn var_with_multiple_type_args_on_generic_fb(mut with_db: RootDatabase) {
    add_sources(
        &mut with_db,
        &[r#"
FUNCTION_BLOCK Pair VAR_INPUT a : ANY_INT; b : ANY_REAL; END_VAR END_FUNCTION_BLOCK

FUNCTION_BLOCK Main
VAR p : Pair<INT, LREAL>; END_VAR
END_FUNCTION_BLOCK
"#],
    );
    let fb = find_fb(&with_db, "Main");
    let p = fb
        .variables(&with_db)
        .iter()
        .find(|v| v.get_name_ident(&with_db).text(&with_db) == "p")
        .expect("p var");
    let target = match p.spec(&with_db).kind(&with_db) {
        SpecKind::Target(t) => t,
        other => panic!("expected Target, got {:?}", other),
    };
    assert_eq!(target.type_args.len(), 2);
    assert_eq!(
        elementary_of(&with_db, &target.type_args[0]),
        ElementarySpec::Int
    );
    assert_eq!(
        elementary_of(&with_db, &target.type_args[1]),
        ElementarySpec::LReal
    );
}

// ─── Call-site resolution: fb_any_resolutions map ──────────────────────────

#[rstest]
fn fb_any_int_resolved_from_call_site(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT
    PV: ANY_INT;
END_VAR
VAR_OUTPUT
    CV: INTO(PV);
END_VAR
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
VAR_INPUT
    value: ANY_REAL;
END_VAR
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
VAR_INPUT
    PV: ANY_INT;
END_VAR
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
VAR_INPUT
    x: INT;
END_VAR
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
VAR_INPUT
    PV: ANY_INT;
END_VAR
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
VAR_INPUT
    PV: ANY_INT;
END_VAR
VAR_OUTPUT
    CV: INTO(PV);
END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR counter : CTU<INT>; END_VAR
    counter(PV := 10);
END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// ─── Validation diagnostics: E0321–E0324 ───────────────────────────────────

#[rstest]
fn args_on_non_generic_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Plain
VAR_INPUT x : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR p : Plain<INT>; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0321] Error: generic type arguments
       ,-[ file:///test0.st:7:9 ]
       |
     7 | VAR p : Plain<INT>; END_VAR
       |         ^^^^^|^^^^
       |              `------ 'Plain' is not generic and does not take type arguments
    ---'
    ");
}

#[rstest]
fn missing_args_on_generic_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Counter
VAR_INPUT PV : ANY_INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR c : Counter; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0322] Error: generic type arguments
       ,-[ file:///test0.st:7:9 ]
       |
     7 | VAR c : Counter; END_VAR
       |         ^^^|^^^
       |            `----- 'Counter' is generic and requires 1 type argument
    ---'
    ");
}

#[rstest]
fn wrong_number_of_args(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Pair
VAR_INPUT a : ANY_INT; b : ANY_REAL; END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR p : Pair<INT>; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0323] Error: generic type arguments
       ,-[ file:///test0.st:7:9 ]
       |
     7 | VAR p : Pair<INT>; END_VAR
       |         ^^^^|^^^^
       |             `------ 'Pair' expects 2 type arguments, got 1
    ---'
    ");
}

#[rstest]
fn arg_violates_bound(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Counter
VAR_INPUT PV : ANY_INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR c : Counter<REAL>; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0324] Error: generic type arguments
       ,-[ file:///test0.st:7:17 ]
       |
     7 | VAR c : Counter<REAL>; END_VAR
       |                 ^^|^
       |                   `--- type argument 'REAL' does not conform to bound 'ANY_INT'
    ---'
    ");
}
