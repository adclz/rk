use auto_lsp::{
    default::db::{BaseDatabase, FileManager, file::File},
    lsp_types,
};

use hir::hir_def::{pous::pou::Pou, semantic_index::semantic_index};

use crate::tests::utils::add_sources;
use crate::tests::utils::find_namespace_with_name;
use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;
use db::RootDatabase;
use std::ops::ControlFlow;

use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::semantic_index::HirNode;
use hir::hir_def::semantic_index::SemanticIndex;
use hir::hir_ty::name_res::pou_names_res;
use hir::walk::WalkHir;
use insta::assert_snapshot;
use rstest::rstest;

#[test]
fn global_scope() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
    FUNCTION fn1
    END_FUNCTION    

    FUNCTION_BLOCK fn2
    END_FUNCTION_BLOCK

    CLASS cl
    END_CLASS

    INTERFACE in
    END_INTERFACE
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let sema = semantic_index(&db, file);

    sema.global_pous.iter().for_each(|pou| match pou.pou(&db) {
        Pou::Function(f) => {
            assert_eq!(pou.name(&db).text(&db), "fn1");
            let scope = sema.get_scope(&db, f.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::FunctionBlock(fb) => {
            assert_eq!(pou.name(&db).text(&db), "fn2");
            let scope = sema.get_scope(&db, fb.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::Class(c) => {
            assert_eq!(pou.name(&db).text(&db), "cl");
            let scope = sema.get_scope(&db, c.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::Interface(i) => {
            assert_eq!(pou.name(&db).text(&db), "in");
            let scope = sema.get_scope(&db, i.scope_id(&db));
            assert!(scope.parent.is_some_and(|scope| scope.is_global(&db)));
        }
        Pou::DataType(_) => {}
    });
}

#[test]
fn scoped_pous() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE ns

    FUNCTION fn1
    END_FUNCTION

    FUNCTION_BLOCK fn2
    END_FUNCTION_BLOCK

    CLASS cl
    END_CLASS

    INTERFACE in
    END_INTERFACE

END_NAMESPACE
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();

    let file = db.get_file(&url).unwrap();
    let sema = semantic_index(&db, file);
    let main_ns = sema.namespaces.first().unwrap();

    // Main namespaces should have Global scope as parent
    let main_ns_scope = sema.get_scope(&db, main_ns.scope_id(&db));
    assert!(main_ns_scope.parent.is_some_and(|s| s.is_global(&db)));

    for pou in main_ns.pous(&db) {
        match pou.pou(&db) {
            Pou::Function(f) => {
                assert_eq!(pou.name(&db).text(&db), "fn1");
                let scope = sema.get_scope(&db, f.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::FunctionBlock(fb) => {
                assert_eq!(pou.name(&db).text(&db), "fn2");
                let scope = sema.get_scope(&db, fb.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::Class(c) => {
                assert_eq!(pou.name(&db).text(&db), "cl");
                let scope = sema.get_scope(&db, c.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::Interface(i) => {
                assert_eq!(pou.name(&db).text(&db), "in");
                let scope = sema.get_scope(&db, i.scope_id(&db));
                assert_eq!(scope.parent, Some(main_ns.scope_id(&db)));
            }
            Pou::DataType(_) => {}
        }
    }
}

#[test]
fn using_directives() {
    let mut db = RootDatabase::default();
    let url = lsp_types::Url::parse("file:///test.st").unwrap();
    let source = r#"
NAMESPACE ns
    USING ns2   
    USING ns3.nss
    FUNCTION fn1
    END_FUNCTION    
END_NAMESPACE
"#;
    let file = File::from_string()
        .db(&db)
        .parsers(ast::RK_PARSER.get("structured_text").unwrap())
        .url(&url)
        .source(source.to_string())
        .call()
        .unwrap();

    db.add_file(file).unwrap();
    let file = db.get_file(&url).unwrap();
    let sema = semantic_index(&db, file);

    let main_ns = sema.namespaces.first().unwrap();
    let scope = sema.get_scope(&db, main_ns.scope_id(&db));

    assert_eq!(scope.usings.len(), 2);
}

/// Utility to collect all using in a given source file.
fn collect_usings(db: &dyn BaseDatabase, sema: &SemanticIndex) -> String {
    let mut result = vec![];
    let _ = sema.walk_hir(db, &mut |n| {
        if let HirNode::ResolvedUsing(path) = n {
            result.push(path);
        }
        ControlFlow::Continue(())
    });

    result
        .iter()
        .map(|r| {
            format!(
                "USING {}:  {}\n",
                r.using(db).path(db).to_string(db),
                r.namespaces(db)
                    .iter()
                    .flat_map(|ns| ns
                        .pous(db)
                        .iter()
                        .map(|pou| pou.name(db).text(db).to_string())
                        .collect::<Vec<_>>())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}

#[rstest]
fn child_pous_import_with_using(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    USING ns1.ns2; // bring ns2 into scope

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns2
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1]);

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());

    // brings ns2 into scope
    assert_snapshot!(collect_usings(&with_db, sema), @"USING ns1.ns2:  fb3")
}

#[rstest]
fn sibling_pous_import_with_using(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    USING ns2;
END_NAMESPACE

NAMESPACE ns2
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    // brings ns2 into scope
    assert_snapshot!(collect_usings(&with_db, sema), @"USING ns2:  fb3")
}

#[rstest]
fn fb3_in_scope(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE ns1
    USING ns2;

    FUNCTION_BLOCK fb1
        VAR_INPUT
            i1 : fb2; // should be imported via USING ns2;
        END_VAR

    END_FUNCTION_BLOCK
END_NAMESPACE

NAMESPACE ns2
    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");

    let sema = semantic_index(&with_db, *with_db.get_files().iter().last().unwrap());
    assert_snapshot!(collect_usings(&with_db, sema), @r"
    USING ns2:  fb2
    fb3
    ")
}

// Global pous should be in scope in all namespaces.
#[rstest]
fn inherit_global_pous(mut with_db: RootDatabase) {
    let source1 = r#"
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns1
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE"#;
    add_sources(&mut with_db, &[source1]);

    let ns1 = find_namespace_with_name(
        &with_db,
        *with_db
            .get_files()
            .iter()
            .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
            .unwrap(),
        "ns1",
    )
    .unwrap();

    let pou = Ident::from_slice(&with_db, "fb1");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb2");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb3");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());
}

// Both ns1 namespaces should be allowed to coexist, as they are merged.
// Therefore fb1, fb2, and fb3 should all be in scope in both namespaces.
#[rstest]
fn shared_namespaces(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    let source2 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1, source2]);

    let ns1 = find_namespace_with_name(
        &with_db,
        *with_db
            .get_files()
            .iter()
            .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
            .unwrap(),
        "ns1",
    )
    .unwrap();

    let pou = Ident::from_slice(&with_db, "fb1");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb2");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb3");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());
}

// ns1 and ns2 are separate namespaces, so fb3 should not be in scope in ns1.
#[rstest]
fn unshared_namespaces(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    let source2 = r#"
NAMESPACE ns2
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1, source2]);

    let ns1 = find_namespace_with_name(
        &with_db,
        *with_db
            .get_files()
            .iter()
            .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
            .unwrap(),
        "ns1",
    )
    .unwrap();

    let pou = Ident::from_slice(&with_db, "fb1");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb2");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb3");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_none());
}

// ns1 should have access to ns2's pous because of the USING statement.
#[rstest]
fn shared_namespaces_with_using(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    USING ns2; // bring ns2 into scope

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    let source2 = r#"
NAMESPACE ns2
    FUNCTION_BLOCK fb3

    END_FUNCTION_BLOCK
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1, source2]);

    let ns1 = find_namespace_with_name(
        &with_db,
        *with_db
            .get_files()
            .iter()
            .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
            .unwrap(),
        "ns1",
    )
    .unwrap();

    let pou = Ident::from_slice(&with_db, "fb1");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb2");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb3");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());
}

// Pous declared in parent namespace should be in scope in child namespace.
// Pous from ns1 should be in scope in ns2 because ns2 is nested inside ns1.
#[rstest]
fn parent_pous_inherit(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns2
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1]);

    let ns1 = find_namespace_with_name(
        &with_db,
        *with_db
            .get_files()
            .iter()
            .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
            .unwrap(),
        "ns1.ns2",
    )
    .unwrap();

    let pou = Ident::from_slice(&with_db, "fb1");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb2");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb3");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());
}

// However parent namespaces should not have access to child namespace pous.
// ns1 namespace should not have access to ns2's pous.
#[rstest]
fn child_pous_inherit(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns2
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1]);

    let ns1 = find_namespace_with_name(
        &with_db,
        *with_db
            .get_files()
            .iter()
            .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
            .unwrap(),
        "ns1",
    )
    .unwrap();

    let pou = Ident::from_slice(&with_db, "fb1");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb2");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb3");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_none());
}

// A Using directive in a parent namespace should bring pous from the used namespace,
// even in child namespaces.
#[rstest]
fn child_pous_inherit_with_using(mut with_db: RootDatabase) {
    let source1 = r#"
NAMESPACE ns1
    USING ns1.ns2; // bring ns2 into scope

    FUNCTION_BLOCK fb1

    END_FUNCTION_BLOCK

    FUNCTION_BLOCK fb2

    END_FUNCTION_BLOCK

    NAMESPACE ns2
        FUNCTION_BLOCK fb3

        END_FUNCTION_BLOCK
    END_NAMESPACE
END_NAMESPACE"#;

    add_sources(&mut with_db, &[source1]);

    let ns1 = find_namespace_with_name(
        &with_db,
        *with_db
            .get_files()
            .iter()
            .find(|f| f.url(&with_db).as_str() == "file:///test0.st")
            .unwrap(),
        "ns1",
    )
    .unwrap();

    let pou = Ident::from_slice(&with_db, "fb1");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb2");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());

    let pou = Ident::from_slice(&with_db, "fb3");
    assert!(pou_names_res(&with_db, &pou, ns1.scope_id(&with_db)).is_some());
}
